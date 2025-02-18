mod client;
mod conversation_state;
mod error;
mod input_source;
mod parse;
mod parser;
mod prompt;
mod tools;
use std::collections::HashMap;
use std::io::{
    IsTerminal,
    Read,
    Write,
};
use std::process::ExitCode;
use std::sync::Arc;

use client::Client;
use color_eyre::owo_colors::OwoColorize;
use conversation_state::{
    ConversationRole,
    ConversationState,
    StopReason,
    ToolResult,
};
use crossterm::style::{
    Attribute,
    Color,
};
use crossterm::{
    cursor,
    execute,
    queue,
    style,
    terminal,
};
pub use error::Error;
use eyre::{
    Result,
    bail,
};
use fig_os_shim::Context;
use fig_util::CLI_BINARY_NAME;
use input_source::InputSource;
use parser::{
    ResponseParser,
    ToolUse,
};
use spinners::{
    Spinner,
    Spinners,
};
use tools::ToolSpec;
use tracing::debug;
use winnow::Partial;
use winnow::stream::Offset;

use crate::cli::chat::parse::{
    ParseState,
    interpret_markdown,
};
use crate::util::region_check;

const MAX_TOOL_USE_RECURSIONS: u32 = 50;

pub async fn chat(initial_input: Option<String>) -> Result<ExitCode> {
    if !fig_util::system_info::in_cloudshell() && !fig_auth::is_logged_in().await {
        bail!(
            "You are not logged in, please log in with {}",
            format!("{CLI_BINARY_NAME} login",).bold()
        );
    }

    region_check("chat")?;

    let stdin = std::io::stdin();
    let is_interactive = stdin.is_terminal();
    let initial_input = if !is_interactive {
        // append to input string any extra info that was provided.
        let mut input = initial_input.unwrap_or_default();
        stdin.lock().read_to_string(&mut input)?;
        Some(input)
    } else {
        initial_input
    };

    let ctx = Context::new();
    let tool_config = load_tools()?;
    debug!(?tool_config, "Using tools");

    let client = Client::new(&ctx, tool_config.clone()).await?;
    let mut output = std::io::stdout();

    let result = try_chat(ChatContext {
        output: &mut output,
        ctx: Context::new(),
        initial_input,
        input_source: InputSource::new()?,
        is_interactive,
        tool_config,
        client,
        terminal_width_provider: || terminal::window_size().map(|s| s.columns.into()).ok(),
    })
    .await;

    if is_interactive {
        queue!(output, style::SetAttribute(Attribute::Reset), style::ResetColor).ok();
    }
    output.flush().ok();

    result.map(|_| ExitCode::SUCCESS)
}

/// The tools that can be used by the model.
#[derive(Debug, Clone)]
pub struct ToolConfiguration {
    tools: HashMap<String, ToolSpec>,
}

fn load_tools() -> Result<ToolConfiguration> {
    let tools: Vec<ToolSpec> = serde_json::from_str(include_str!("tools/tool_index.json"))?;
    Ok(ToolConfiguration {
        tools: tools.into_iter().map(|spec| (spec.name.clone(), spec)).collect(),
    })
}

#[derive(Debug)]
struct ChatContext<'w, W> {
    /// The [Write] destination for printing conversation text.
    output: &'w mut W,
    ctx: Arc<Context>,
    initial_input: Option<String>,
    input_source: InputSource,
    is_interactive: bool,
    tool_config: ToolConfiguration,
    /// The client to use to interact with the model.
    client: Client,
    /// Width of the terminal, required for [ParseState].
    terminal_width_provider: fn() -> Option<usize>,
}

async fn try_chat<W: Write>(chat_ctx: ChatContext<'_, W>) -> Result<()> {
    let ChatContext {
        output,
        ctx,
        mut initial_input,
        mut input_source,
        is_interactive,
        tool_config,
        client,
        terminal_width_provider,
    } = chat_ctx;

    // todo: what should we set this to?
    if is_interactive {
        execute!(
            output,
            style::Print(color_print::cstr! {"
Hi, I'm <g>Amazon Q</g>. I can answer questions about your workspace and tooling, and help execute ops related tasks on your behalf!

"
            })
        )?;
    }

    let mut conversation_state = ConversationState::new(tool_config.clone());
    let mut stop_reason = None; // StopReason associated with each model response.
    let mut tool_uses = Vec::new();
    let mut tool_use_recursions = 0;
    let mut response;
    let mut spinner = None;

    loop {
        match stop_reason {
            // None -> first loop recursion
            // Some(EndTurn) -> assistant has finished responding/requesting tool uses.
            // In both cases, send the next user's prompt.
            Some(StopReason::EndTurn) | None => {
                tool_use_recursions = 0;
                let user_input = match initial_input.take() {
                    Some(input) => input,
                    None => match input_source.read_line(Some("> "))? {
                        Some(line) => line,
                        None => break,
                    },
                };

                match user_input.trim() {
                    "exit" | "quit" => {
                        if let Some(_id) = conversation_state.conversation_id {
                            // TODO: telemetry
                            // fig_telemetry::send_end_chat(id.clone()).await;
                        }
                        return Ok(());
                    },
                    _ => (),
                }

                if is_interactive {
                    queue!(output, style::SetForegroundColor(Color::Magenta))?;
                    if user_input.contains("@history") {
                        queue!(output, style::Print("Using shell history\n"))?;
                    }
                    if user_input.contains("@git") {
                        queue!(output, style::Print("Using git context\n"))?;
                    }
                    if user_input.contains("@env") {
                        queue!(output, style::Print("Using environment\n"))?;
                    }
                    queue!(output, style::SetForegroundColor(Color::Reset))?;
                    queue!(output, cursor::Hide)?;
                    spinner = Some(Spinner::new(Spinners::Dots, "Generating your answer...".to_owned()));
                    tokio::spawn(async {
                        tokio::signal::ctrl_c().await.unwrap();
                        execute!(std::io::stdout(), cursor::Show).unwrap();
                        #[allow(clippy::exit)]
                        std::process::exit(0);
                    });
                    execute!(output, style::Print("\n"))?;
                }
                conversation_state.append_new_user_message(user_input);

                response = Some(client.send_messages(&mut conversation_state).await?);
            },
            Some(StopReason::ToolUse) => {
                tool_use_recursions += 1;
                if tool_use_recursions > MAX_TOOL_USE_RECURSIONS {
                    bail!("Exceeded max tool use recursion limit: {}", MAX_TOOL_USE_RECURSIONS);
                }

                let uses = std::mem::take(&mut tool_uses);
                let tool_results = handle_tool_uses(output, &ctx, uses).await?;
                conversation_state.add_tool_results(tool_results);

                response = Some(client.send_messages(&mut conversation_state).await?);
            },
        }

        // Handle the response
        if let Some(response) = response.take() {
            let mut buf = String::new();
            let mut offset = 0;
            let mut ended = false;
            let mut parser = ResponseParser::new(Arc::clone(&ctx), response);
            let mut state = ParseState::new(terminal_width_provider());

            loop {
                match parser.recv().await {
                    Ok(msg_event) => match msg_event {
                        parser::ResponseEvent::ConversationId(id) => {
                            conversation_state.conversation_id = Some(id);
                        },
                        parser::ResponseEvent::AssistantText(text) => {
                            buf.push_str(&text);
                        },
                        parser::ResponseEvent::ToolUse(tool_use) => {
                            buf.push_str(&format!("\n\n# Tool Use: {}", tool_use.tool));
                            tool_uses.push(tool_use);
                        },
                        parser::ResponseEvent::EndStream {
                            stop_reason: sr,
                            message,
                        } => {
                            // buf.push_str("\n\n");
                            stop_reason = Some(sr);
                            conversation_state.push_assistant_message(message);
                            ended = true;
                        },
                    },
                    Err(err) => {
                        bail!("An error occurred reading the model's response: {:?}", err);
                    },
                }

                // Fix for the markdown parser copied over from q chat:
                // this is a hack since otherwise the parser might report Incomplete with useful data
                // still left in the buffer. I'm not sure how this is intended to be handled.
                if ended {
                    buf.push('\n');
                }

                if !buf.is_empty() && is_interactive && spinner.is_some() {
                    drop(spinner.take());
                    queue!(
                        output,
                        terminal::Clear(terminal::ClearType::CurrentLine),
                        cursor::MoveToColumn(0),
                        cursor::Show
                    )?;
                }

                // Print the response
                loop {
                    let input = Partial::new(&buf[offset..]);
                    match interpret_markdown(input, &mut *output, &mut state) {
                        Ok(parsed) => {
                            offset += parsed.offset_from(&input);
                            output.flush()?;
                            state.newline = state.set_newline;
                            state.set_newline = false;
                        },
                        Err(err) => match err.into_inner() {
                            Some(err) => bail!(err.to_string()),
                            None => break, // Data was incomplete
                        },
                    }
                }

                if ended {
                    if is_interactive {
                        queue!(output, style::ResetColor, style::SetAttribute(Attribute::Reset))?;
                        execute!(output, style::Print("\n"))?;

                        for (i, citation) in &state.citations {
                            queue!(
                                output,
                                style::Print("\n"),
                                style::SetForegroundColor(Color::Blue),
                                style::Print(format!("[^{i}]: ")),
                                style::SetForegroundColor(Color::DarkGrey),
                                style::Print(format!("{citation}\n")),
                                style::SetForegroundColor(Color::Reset)
                            )?;
                        }
                    }
                    break;
                }
            }
        }
    }

    Ok(())
}

async fn handle_tool_uses(
    _output: &mut impl Write,
    _ctx: &Context,
    _tool_uses: Vec<ToolUse>,
) -> Result<Vec<ToolResult>> {
    Ok(vec![])
}

// async fn handle_tool_use(tool_uses: Vec<ToolUse>) -> Result<Vec<Message>> {
// async fn handle_tool_uses(output: &mut impl Write, ctx: &Context, tool_uses: Vec<ToolUse>) ->
// Result<Vec<ToolResult>> {     debug!(?tool_uses, "processing tools");
//     let mut results = Vec::new();
//
//     for tool_use in tool_uses {
//         queue!(output, style::SetAttribute(Attribute::Bold))?;
//         // queue!(output, style::Print())
//         queue!(output, style::Print(tool_use.tool.display_name()))?;
//         queue!(output, cur)?;
//     }
//
//     for tool_use in tool_uses {
//         match ask_for_consent() {
//             Ok(_) => (),
//             Err(reason) => {
//                 results.push(ToolResult {
//                     tool_use_id: tool_use.tool_use_id,
//                     content: vec![ToolResultContentBlock::Text(format!(
//                         "The user denied permission to execute this tool. Reason: {}",
//                         &reason
//                     ))],
//                     status: ToolResultStatus::Error,
//                 });
//                 break;
//             },
//         }
//
//         match tool_use.tool.invoke(&ctx).await {
//             Ok(result) => {
//                 results.push(ToolResult {
//                     tool_use_id: tool_use.tool_use_id,
//                     content: vec![result.into()],
//                     status: ToolResultStatus::Success,
//                 });
//             },
//             Err(err) => {
//                 error!(?err, "An error occurred processing the tool");
//                 results.push(ToolResult {
//                     tool_use_id: tool_use.tool_use_id,
//                     content: vec![ToolResultContentBlock::Text(format!(
//                         "An error occurred processing the tool: {}",
//                         err
//                     ))],
//                     status: ToolResultStatus::Error,
//                 });
//             },
//         }
//     }
//     Ok(results)
// }
