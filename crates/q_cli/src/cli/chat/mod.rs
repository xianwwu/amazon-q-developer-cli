mod conversation_state;
mod input_source;
mod parse;
mod parser;
mod prompt;
mod tools;
use std::collections::HashMap;
use std::io::{
    IsTerminal,
    Read,
    Stdout,
    Write,
};
use std::process::ExitCode;
use std::sync::Arc;

use color_eyre::owo_colors::OwoColorize;
use conversation_state::ConversationState;
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
use eyre::{
    Result,
    bail,
};
use fig_api_client::StreamingClient;
use fig_api_client::clients::SendMessageOutput;
use fig_api_client::model::{
    ToolResult,
    ToolResultContentBlock,
    ToolResultStatus,
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
use tools::{
    Tool,
    ToolSpec,
    parse_tool,
};
use tracing::{
    debug,
    error,
};
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

    let tool_config = load_tools()?;
    debug!(?tool_config, "Using tools");

    let client = StreamingClient::new().await?;
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
struct ChatContext<'o> {
    /// The [Write] destination for printing conversation text.
    output: &'o mut Stdout,
    ctx: Arc<Context>,
    initial_input: Option<String>,
    input_source: InputSource,
    is_interactive: bool,
    tool_config: ToolConfiguration,
    /// The client to use to interact with the model.
    client: StreamingClient,
    /// Width of the terminal, required for [ParseState].
    terminal_width_provider: fn() -> Option<usize>,
}

async fn try_chat(chat_ctx: ChatContext<'_>) -> Result<()> {
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
    let mut tool_uses = vec![];
    let mut tool_use_recursions = 0;
    let mut spinner = None;

    loop {
        let terminal_width = terminal_width_provider().unwrap_or(80);

        let mut response = response(
            &ctx,
            &client,
            output,
            &mut conversation_state,
            &mut tool_uses,
            &mut tool_use_recursions,
            &mut input_source,
            initial_input.take(),
            is_interactive,
            terminal_width,
            &mut spinner,
        )
        .await?;
        let response = match response.take() {
            Some(response) => response,
            None => break,
        };

        // Handle the response
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
                        buf.push_str(&format!("\n\n# Tool Use: {}", tool_use.args));
                        tool_uses.push(tool_use);
                    },
                    parser::ResponseEvent::EndStream { message } => {
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

        if !is_interactive {
            break;
        }
    }

    Ok(())
}

async fn response(
    ctx: &Context,
    client: &StreamingClient,
    output: &mut Stdout,
    mut conversation_state: &mut ConversationState,
    mut tool_uses: &mut Vec<ToolUse>,
    mut tool_use_recursions: &mut u32,
    mut input_source: &mut InputSource,
    mut initial_input: Option<String>,
    is_interactive: bool,
    terminal_width: usize,
    mut spinner: &mut Option<Spinner>,
) -> Result<Option<SendMessageOutput>> {
    let mut queued_tools: Vec<(String, Box<dyn Tool>)> = vec![];
    if !tool_uses.is_empty() {
        // Parse the requested tools then validate them initializing needed fields
        let mut tool_results = Vec::with_capacity(tool_uses.len());
        for tool_use in tool_uses.drain(..) {
            let tool_use_id = tool_use.id.clone();
            match parse_tool(tool_use) {
                Ok(mut tool) => {
                    if let Err(err) = tool.validate(&ctx).await {
                        tool_results.push(ToolResult {
                            tool_use_id,
                            content: vec![ToolResultContentBlock::Text(format!(
                                "Failed to validate tool parameters: {err}"
                            ))],
                            status: ToolResultStatus::Error,
                        });
                    }
                },
                Err(err) => tool_results.push(err),
            }
        }

        if !tool_results.is_empty() {
            conversation_state.add_tool_results(tool_results);
            return Ok(Some(client.send_message(conversation_state.clone().into()).await?));
        }
    }

    let user_input = match initial_input.take() {
        Some(input) => input,
        None => match input_source.read_line(Some("> "))? {
            Some(line) => line,
            None => return Ok(None),
        },
    };

    match user_input.trim() {
        "exit" | "quit" => {
            if let Some(_id) = conversation_state.conversation_id.as_ref() {
                // TODO: telemetry
                // fig_telemetry::send_end_chat(id.clone()).await;
            }

            return Ok(None);
        },
        c if c == "c" && !queued_tools.is_empty() => {
            *tool_use_recursions += 1;
            if *tool_use_recursions > MAX_TOOL_USE_RECURSIONS {
                bail!("Exceeded max tool use recursion limit: {}", MAX_TOOL_USE_RECURSIONS);
            }

            // Prompt for consent of the requested tools.
            for tool_use in &queued_tools {
                queue!(output, style::Print("-".repeat(terminal_width)));
                queue!(output, cursor::MoveToColumn(2))?;
                queue!(output, style::SetAttribute(Attribute::Bold))?;
                queue!(output, style::Print(format!(" {} ", tool_use.1.display_name())));
                queue!(output, cursor::MoveDown(1));
                queue!(output, style::SetAttribute(Attribute::NormalIntensity));
                tool_use.1.show_readable_intention(output);
                output.flush()?;
            }

            // Execute the requested tools.
            let mut tool_results = vec![];
            for tool in queued_tools {
                match tool.1.invoke(&ctx, output).await {
                    Ok(result) => {
                        tool_results.push(ToolResult {
                            tool_use_id: tool.0,
                            content: vec![result.into()],
                            status: ToolResultStatus::Success,
                        });
                    },
                    Err(err) => {
                        error!(?err, "An error occurred processing the tool");
                        tool_results.push(ToolResult {
                            tool_use_id: tool.0,
                            content: vec![ToolResultContentBlock::Text(format!(
                                "An error occurred processing the tool: {}",
                                err
                            ))],
                            status: ToolResultStatus::Error,
                        });
                    },
                }
            }

            conversation_state.add_tool_results(tool_results);
            Ok(Some(client.send_message(conversation_state.clone().into()).await?))
        },
        _ => {
            *tool_use_recursions = 0;

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
                *spinner = Some(Spinner::new(Spinners::Dots, "Generating your answer...".to_owned()));
                tokio::spawn(async {
                    tokio::signal::ctrl_c().await.unwrap();
                    execute!(std::io::stdout(), cursor::Show).unwrap();
                    #[allow(clippy::exit)]
                    std::process::exit(0);
                });
                execute!(output, style::Print("\n"))?;
            }

            conversation_state.append_new_user_message(user_input);
            Ok(Some(client.send_message(conversation_state.clone().into()).await?))
        },
    }
}
