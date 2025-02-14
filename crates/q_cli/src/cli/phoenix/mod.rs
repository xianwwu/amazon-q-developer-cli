mod client;
mod error;
mod input_source;
mod parser;
mod parser2;
mod tools;
mod types;
use std::io::Write;
use std::process::ExitCode;
use std::sync::Arc;

use aws_sdk_bedrockruntime::types::{
    ToolResultBlock,
    ToolResultContentBlock,
    ToolResultStatus,
};
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
    ToolConfig,
    load_tool_config,
};
use tracing::{
    debug,
    error,
    info,
};
use types::{
    ContentBlock,
    ConversationRole,
    Message,
    StopReason,
};
use winnow::Partial;
use winnow::stream::Offset;

use crate::cli::chat::parse::{
    ParseState,
    interpret_markdown,
};
use crate::util::region_check;

const MODEL_ID: &str = "anthropic.claude-3-5-sonnet-20241022-v2:0";

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
    let tool_config = load_tool_config();
    debug!(?tool_config, "Using tool configuration");

    let system_prompt = create_system_prompt(&ctx)?;
    let client = Client::new(MODEL_ID.to_string(), system_prompt, tool_config.clone()).await;
    let mut stdout = std::io::stdout();

    try_chat(ChatContext {
        output: &mut stdout,
        session_id: None,
        ctx: Context::new(),
        input_source: InputSource::new()?,
        tool_config,
        client,
        terminal_width_provider: || terminal::window_size().map(|s| s.columns.into()).ok(),
    })
    .await?;

    Ok(ExitCode::SUCCESS)
}

/// Creates a system prompt with context about the user's environment.
fn create_system_prompt(ctx: &Context) -> Result<String> {
    let cwd = ctx.env().current_dir()?;
    let cwd = cwd.to_string_lossy();
    let os = ctx.platform().os();
    let system_prompt = format!(
        r#"You are an expert programmer and CLI chat assistant. You are given a list of tools to use to answer a given prompt.

You should only respond to tasks related to coding. You must never make assumptions about the user's environment. If you need more information,
you MUST make a tool use request.

When you execute a tool, do not assume that the user can see the output directly. You should either show the command output, or explain what the
output contains in a friendly format.

Context about the user's environment is provided below:
- Current working directory: {}
- Operating system: {}
"#,
        cwd, os
    );

    Ok(system_prompt)
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
    session_id: Option<String>,
    ctx: Arc<Context>,
    input_source: InputSource,
    tool_config: ToolConfig,
    /// The client to use to interact with the model.
    client: Client,
    /// Width of the terminal, required for [ParseState].
    terminal_width_provider: fn() -> Option<usize>,
}

async fn try_chat<W: Write>(chat_ctx: ChatContext<'_, W>) -> Result<()> {
    let ChatContext {
        output,
        session_id: conversation_id,
        ctx,
        mut input_source,
        client,
        tool_config,
        terminal_width_provider,
    } = chat_ctx;

    // todo: what should we set this to?
    execute!(
        output,
        style::Print(color_print::cstr! {"
Hi, I'm <g>Amazon Q</g>. I can answer questions about your shell and CLI tools, and even perform actions on your behalf! 🐦

"
        })
    )?;

    let mut messages = Vec::new(); // Holds the entire conversation message history.
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

                messages.push(Message::new(ConversationRole::User, vec![ContentBlock::Text(
                    user_input,
                )]));

                response = Some(client.send_messages(messages.clone()).await?);
            },
            Some(StopReason::ToolUse) => {
                tool_use_recursions += 1;
                if tool_use_recursions > MAX_TOOL_USE_RECURSIONS {
                    bail!("Exceeded max tool use recursion limit: {}", MAX_TOOL_USE_RECURSIONS);
                }

                let uses = std::mem::take(&mut tool_uses);
                let mut tool_results = handle_tool_use(uses).await?;
                messages.append(&mut tool_results);

                response = Some(client.send_messages(messages.clone()).await?);
            },
            Some(other) => {
                bail!("Unknown stop reason: {:?}", other);
            },
        }

        // Handle the response
        if let Some(response) = response.take() {
            let mut buf = String::new();
            let mut offset = 0;
            let mut ended = false;
            let mut parser = ResponseParser::new(Arc::clone(&ctx), response, tool_config.clone());
            let mut state = ParseState::new(terminal_width_provider());

            loop {
                match parser.recv().await {
                    Ok(msg_event) => match msg_event {
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
                            metadata,
                        } => {
                            debug!(?metadata, "Metadata on last response");
                            buf.push_str("\n\n");
                            stop_reason = Some(sr);
                            messages.push(message);
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
async fn handle_tool_use(tool_uses: Vec<ToolUse>) -> Result<Vec<Message>> {
    debug!(?tool_uses, "processing tools");
    let mut messages = Vec::new();
    for tool_use in tool_uses {
        if tool_use.requires_consent() {
            // prompt user first, if required, return if denied
            match ask_for_consent() {
                Ok(_) => (),
                Err(reason) => {
                    messages.push(Message::new(ConversationRole::User, vec![ContentBlock::ToolResult(
                        ToolResultBlock::builder()
                            .tool_use_id(tool_use.tool_use_id)
                            .content(ToolResultContentBlock::Text(format!(
                                "The user denied permission to execute this tool. Reason: {}",
                                &reason
                            )))
                            .status(ToolResultStatus::Error)
                            .build()
                            .unwrap(),
                    )]));
                    break;
                },
            }
        }
        match tool_use.invoke().await {
            Ok(result) => {
                messages.push(Message::new(ConversationRole::User, vec![ContentBlock::ToolResult(
                    ToolResultBlock::builder()
                        .tool_use_id(tool_use.tool_use_id)
                        .content(result.into())
                        .status(ToolResultStatus::Success)
                        .build()
                        .unwrap(),
                )]));
            },
            Err(err) => {
                error!(?err, "An error occurred processing the tool");
                messages.push(Message::new(ConversationRole::User, vec![ContentBlock::ToolResult(
                    ToolResultBlock::builder()
                        .tool_use_id(tool_use.tool_use_id)
                        .content(ToolResultContentBlock::Text(format!(
                            "An error occurred processing the tool: {}",
                            err
                        )))
                        .status(ToolResultStatus::Error)
                        .build()
                        .unwrap(),
                )]));
            },
        }
    }
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_flow() {
        let user_input = vec!["test input", "y"];
        let mut output = String::new();

        let cc = ChatContext {
            output: &mut output,
            session_id: None,
            ctx: Context::new_fake(),
            input_source: todo!(),
            tool_config: todo!(),
            client: todo!(),
            terminal_width_provider: || Some(50),
        };
    }
}
