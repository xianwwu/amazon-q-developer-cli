mod client;
mod error;
mod input_source;
mod parser;
mod tools;
mod types;
use std::collections::HashMap;
use std::io::Write;
use std::process::ExitCode;
use std::sync::Arc;

use client::Client;
use color_eyre::owo_colors::OwoColorize;
use crossterm::{
    execute,
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
use tools::{
    InvokeOutput,
    Tool,
    ToolSpec,
};
use tracing::{
    debug,
    error,
};
use types::{
    ConversationRole,
    ConversationState,
    StopReason,
    ToolResult,
    ToolResultContentBlock,
    ToolResultStatus,
};
use winnow::Partial;
use winnow::stream::Offset;

use crate::cli::chat::parse::{
    ParseState,
    interpret_markdown,
};
use crate::util::region_check;

const MAX_TOOL_USE_RECURSIONS: u32 = 50;

pub async fn chat(mut input: String) -> Result<ExitCode> {
    if !fig_util::system_info::in_cloudshell() && !fig_auth::is_logged_in().await {
        bail!(
            "You are not logged in, please log in with {}",
            format!("{CLI_BINARY_NAME} login",).bold()
        );
    }

    region_check("chat")?;

    let ctx = Context::new();
    let tool_config = load_tools()?;
    debug!(?tool_config, "Using tools");

    let client = Client::new(&ctx, tool_config.clone()).await?;
    let mut stdout = std::io::stdout();

    try_chat(ChatContext {
        output: &mut stdout,
        ctx: Context::new(),
        input_source: InputSource::new()?,
        tool_config,
        client,
        terminal_width_provider: || terminal::window_size().map(|s| s.columns.into()).ok(),
    })
    .await?;

    Ok(ExitCode::SUCCESS)
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

fn ask_for_consent() -> Result<(), String> {
    Ok(())
}

#[async_trait::async_trait]
impl Tool for ToolUse {
    async fn invoke(&self) -> Result<InvokeOutput, Error> {
        debug!(?self, "invoking tool");
        self.tool.invoke().await
    }
}

impl std::fmt::Display for ToolUse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.tool)
    }
}

#[derive(Debug)]
struct ChatContext<'w, W> {
    /// The [Write] destination for printing conversation text.
    output: &'w mut W,
    ctx: Arc<Context>,
    input_source: InputSource,
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
        mut input_source,
        tool_config,
        client,
        terminal_width_provider,
    } = chat_ctx;

    // todo: what should we set this to?
    execute!(
        output,
        style::Print(color_print::cstr! {"
Hi, I'm <g>Amazon Q</g>. I can answer questions about your shell and CLI tools, and even perform actions on your behalf!

"
        })
    )?;

    let mut conversation_id = None;
    let mut conversation_state = ConversationState::new(tool_config.clone());
    let mut stop_reason = None; // StopReason associated with each model response.
    let mut tool_uses = Vec::new();
    let mut tool_use_recursions = 0;
    #[allow(unused_assignments)] // not sure why this is triggering a lint warning
    let mut response = None;

    loop {
        match stop_reason {
            // None -> first loop recursion
            // Some(EndTurn) -> assistant has finished responding/requesting tool uses.
            // In both cases, send the next user's prompt.
            Some(StopReason::EndTurn) | None => {
                tool_use_recursions = 0;
                let user_input = match input_source.read_line(Some("> "))? {
                    Some(line) => line,
                    None => break,
                };

                match user_input.trim() {
                    "exit" | "quit" => {
                        if let Some(conversation_id) = conversation_id {
                            // fig_telemetry::send_end_chat(conversation_id.clone()).await;
                        }
                        return Ok(());
                    },
                    _ => (),
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
                let tool_results = handle_tool_use(uses).await?;
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
                            conversation_id = Some(id);
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
                            buf.push_str("\n\n");
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
                    output.flush()?;
                    break;
                }
            }
        }
    }

    Ok(())
}

/// Executes the list of tools and returns their results as messages.
// async fn handle_tool_use(tool_uses: Vec<ToolUse>) -> Result<Vec<Message>> {
async fn handle_tool_use(tool_uses: Vec<ToolUse>) -> Result<Vec<ToolResult>> {
    debug!(?tool_uses, "processing tools");
    let mut results = Vec::new();
    for tool_use in tool_uses {
        match ask_for_consent() {
            Ok(_) => (),
            Err(reason) => {
                results.push(ToolResult {
                    tool_use_id: tool_use.tool_use_id,
                    content: vec![ToolResultContentBlock::Text(format!(
                        "The user denied permission to execute this tool. Reason: {}",
                        &reason
                    ))],
                    status: ToolResultStatus::Error,
                });
                break;
            },
        }

        match tool_use.invoke().await {
            Ok(result) => {
                results.push(ToolResult {
                    tool_use_id: tool_use.tool_use_id,
                    content: vec![result.into()],
                    status: ToolResultStatus::Success,
                });
            },
            Err(err) => {
                error!(?err, "An error occurred processing the tool");
                results.push(ToolResult {
                    tool_use_id: tool_use.tool_use_id,
                    content: vec![ToolResultContentBlock::Text(format!(
                        "An error occurred processing the tool: {}",
                        err
                    ))],
                    status: ToolResultStatus::Error,
                });
            },
        }
    }
    Ok(results)
}
