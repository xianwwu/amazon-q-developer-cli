mod tools;
use std::borrow::Cow;
use std::process::ExitCode;

use aws_sdk_bedrockruntime::Client as BedrockClient;
// use aws_sdk_bedrockruntime::Error
use aws_sdk_bedrockruntime::types::{
    ContentBlock,
    ContentBlockDelta,
    ContentBlockDeltaEvent,
    ContentBlockStart,
    ConversationRole,
    ConverseStreamOutput,
    Message as BedrockMessage,
    StopReason,
    ToolConfiguration as BedrockToolConfiguration,
    ToolResultBlock,
    ToolResultContentBlock,
    ToolResultStatus,
    ToolUseBlock,
};
use color_eyre::owo_colors::OwoColorize;
use eyre::{
    Result,
    bail,
};
use fig_os_shim::Context;
use fig_util::CLI_BINARY_NAME;
use tools::{
    Tool,
    serde_value_to_document,
};
use tracing::{
    debug,
    error,
    info,
    trace,
    warn,
};

use crate::util::region_check;

const CLAUDE_REGION: &str = "us-west-2";
const MODEL_ID: &str = "anthropic.claude-3-haiku-20240307-v1:0";

const SYSTEM_PROMPT: &str = r#"You are a CLI chat assistant. You are given a list of tools to use to answer a given prompt.

You MUST:
1. Never make assumptions about the user's environment. If you need more information, you MUST make a tool use request.
"#;

const MAX_TOOL_USE_RECURSIONS: u32 = 5;

pub async fn chat(mut input: String) -> Result<ExitCode> {
    if !fig_util::system_info::in_cloudshell() && !fig_auth::is_logged_in().await {
        bail!(
            "You are not logged in, please log in with {}",
            format!("{CLI_BINARY_NAME} login",).bold()
        );
    }

    region_check("chat")?;

    info!("Running achat");

    let client = Client::new().await.client;

    let tool_specs = tools::load_tools();
    let tool_config = BedrockToolConfiguration::builder()
        .set_tools(Some(
            tool_specs.values().cloned().map(Into::into).collect::<_>(),
        ))
        .build()
        .unwrap();

    info!(?tool_config, "Using tool configuration");

    let mut messages = Vec::new();
    let mut stop_reason = None;
    let mut tool_use_recursions = 0;

    loop {
        match stop_reason {
            Some(StopReason::ToolUse) => {
                // We need to process a tool, don't ask for input
                tool_use_recursions += 1;
                if tool_use_recursions >= MAX_TOOL_USE_RECURSIONS {
                    bail!("Exceeded max tool use recursion limit: {}", MAX_TOOL_USE_RECURSIONS);
                }
            },
            Some(StopReason::EndTurn) => {
                #[allow(unused_assignments)]
                {
                    tool_use_recursions = 0;
                }
                break;
            },
            None => {
                // First loop iteration
                messages.push(
                    BedrockMessage::builder()
                        .role(ConversationRole::User)
                        .content(ContentBlock::Text(
                            "What is in my project's readme? The path is /Volumes/workplace/q-cli/README.md".into(),
                        ))
                        .build()
                        .unwrap(),
                );
            },
            _ => break,
        }

        let response = client
            .converse_stream()
            .model_id(MODEL_ID)
            .system(aws_sdk_bedrockruntime::types::SystemContentBlock::Text(
                SYSTEM_PROMPT.into(),
            ))
            .set_messages(Some(messages.clone()))
            .tool_config(tool_config.clone())
            .send()
            .await?;

        let mut stream = response.stream;
        let mut ai_text = String::new(); // Assistant's text response
        let mut tool_uses = Vec::new(); // tool uses requested by the Assistant
        let mut message = BedrockMessage::builder(); // message to include in the history for the
        // Assistant
        stop_reason = None;
        loop {
            match stream.recv().await {
                Ok(Some(val)) => {
                    trace!(?val, "Received output");
                    match val {
                        ConverseStreamOutput::ContentBlockDelta(event) => match event.delta {
                            Some(ContentBlockDelta::Text(text)) => {
                                ai_text.push_str(&text);
                            },
                            ref other => {
                                warn!(?event, "Unexpected event while reading the model response");
                            },
                        },
                        ConverseStreamOutput::ContentBlockStart(event) => {
                            match event.start {
                                Some(ContentBlockStart::ToolUse(start)) => {
                                    // consume tool use until blockstop
                                    let mut tool_args = String::new();
                                    let tool_name = &start.name;
                                    loop {
                                        match stream.recv().await {
                                            Ok(
                                                ref l @ Some(ConverseStreamOutput::ContentBlockDelta(
                                                    ContentBlockDeltaEvent {
                                                        delta: Some(ContentBlockDelta::ToolUse(ref tool)),
                                                        ..
                                                    },
                                                )),
                                            ) => {
                                                trace!(?l, "Received output");
                                                tool_args.push_str(&tool.input);
                                            },
                                            Ok(ref l @ Some(ConverseStreamOutput::ContentBlockStop(_))) => {
                                                trace!(?l, "Received output");
                                                break;
                                            },
                                            Ok(event) => {
                                                bail!(
                                                    "Received unexpected event while parsing a tool use: {:?}",
                                                    event
                                                );
                                            },
                                            Err(err) => bail!(err),
                                        }
                                    }
                                    let value: serde_json::Value = serde_json::from_str(&tool_args)?;
                                    message = message.content(ContentBlock::ToolUse(
                                        ToolUseBlock::builder()
                                            .tool_use_id(start.tool_use_id.clone())
                                            .name(tool_name)
                                            .input(serde_value_to_document(value.clone()))
                                            .build()
                                            .unwrap(),
                                    ));
                                    match tool_specs.get(tool_name) {
                                        Some(spec) => {
                                            tool_uses.push(ToolUse {
                                                tool_use_id: start.tool_use_id,
                                                tool: tools::new_tool(Context::new(), &spec.name, value)?,
                                            });
                                        },
                                        None => {
                                            error!(tool_name, "Unknown tool use");
                                        },
                                    }
                                },
                                ref other => {
                                    warn!(?other, "Unexpected ContentBlockStart event that isn't a tool use");
                                },
                            }
                        },
                        ConverseStreamOutput::ContentBlockStop(event) => {
                            // This should only match for the AI response.
                            assert!(event.content_block_index == 0);
                            message = message.content(ContentBlock::Text(ai_text.clone()));
                        },
                        ConverseStreamOutput::MessageStart(event) => {
                            assert!(event.role == ConversationRole::Assistant);
                            message = message.role(event.role);
                        },
                        ConverseStreamOutput::MessageStop(event) => {
                            match event.stop_reason {
                                StopReason::EndTurn | StopReason::ToolUse => {
                                    assert!(stop_reason.is_none());
                                    stop_reason = Some(event.stop_reason);
                                },
                                StopReason::MaxTokens => {
                                    // todo - how to handle max tokens?
                                },
                                other => {
                                    warn!("Unhandled message stop reason: {}", other);
                                },
                            }
                        },
                        ConverseStreamOutput::Metadata(event) => {
                            if stop_reason.is_none() {
                                warn!(?event, "Unexpected Metadata event before MessageStop");
                            }
                            if let Some(usage) = event.usage() {
                                debug!(?usage, "usage data");
                            }
                        },
                        _ => (),
                    }
                },
                Ok(None) => {
                    if stop_reason.is_none() {
                        warn!("Did not receive MessageStop before end of stream");
                    }
                    // Return message
                    break;
                },
                Err(err) => bail!("Error occurred receiving the stream: {:?}", err),
            }
        }

        let message = message.build().expect("Building the AI message should not fail");
        debug!(?message, "constructed AI message from stream");
        messages.push(message);

        // handle tool use
        if matches!(stop_reason, Some(StopReason::ToolUse)) {
            for tool_use in tool_uses {
                if tool_use.requires_consent() {
                    // prompt user first, if required, return if denied
                    match ask_for_consent() {
                        Ok(_) => (),
                        Err(reason) => {
                            messages.push(
                                BedrockMessage::builder()
                                    .role(ConversationRole::User)
                                    .content(ContentBlock::ToolResult(
                                        ToolResultBlock::builder()
                                            .tool_use_id(tool_use.tool_use_id)
                                            .content(ToolResultContentBlock::Text(format!(
                                                "The user denied permission to execute this tool. Reason: {}",
                                                &reason
                                            )))
                                            .status(ToolResultStatus::Error)
                                            .build()
                                            .unwrap(),
                                    ))
                                    .build()
                                    .unwrap(),
                            );
                            break;
                        },
                    }
                }
                match tool_use.invoke().await {
                    Ok(result) => {
                        messages.push(
                            BedrockMessage::builder()
                                .role(ConversationRole::User)
                                .content(ContentBlock::ToolResult(
                                    ToolResultBlock::builder()
                                        .tool_use_id(tool_use.tool_use_id)
                                        .content(result.into())
                                        .status(ToolResultStatus::Success)
                                        .build()
                                        .unwrap(),
                                ))
                                .build()
                                .unwrap(),
                        );
                    },
                    Err(err) => {
                        error!(?err, "An error occurred processing the tool");
                        messages.push(
                            BedrockMessage::builder()
                                .role(ConversationRole::User)
                                .content(ContentBlock::ToolResult(
                                    ToolResultBlock::builder()
                                        .tool_use_id(tool_use.tool_use_id)
                                        .content(ToolResultContentBlock::Text(format!(
                                            "An error occurred processing the tool: {}",
                                            err
                                        )))
                                        .status(ToolResultStatus::Error)
                                        .build()
                                        .unwrap(),
                                ))
                                .build()
                                .unwrap(),
                        );
                    },
                }
            }
        }
    }

    Ok(ExitCode::SUCCESS)
}

fn ask_for_consent() -> Result<(), String> {
    Ok(())
}

// #[derive(Debug, thiserror::Error)]
// pub enum RecvError {
//     // #[error(transparent)]
//     // SdkError(#[from] SdkError),
//     #[error("Model requested the use of an unknown tool: {0}")]
//     UnknownToolUse(String),
//     #[error("{0}")]
//     Custom(Cow<'static, str>),
// }

#[derive(Debug)]
pub struct ToolUse {
    /// Corresponds to the `"toolUseId"` returned by the model.
    pub tool_use_id: String,
    pub tool: Box<dyn tools::Tool + Sync>,
}

#[async_trait::async_trait]
impl tools::Tool for ToolUse {
    async fn invoke(&self) -> Result<tools::InvokeOutput, tools::ToolError> {
        debug!(?self, "invoking tool");
        self.tool.invoke().await
    }
}

pub async fn try_chat(ctx: &Context) {}

/// A client for calling the Bedrock ConverseStream API.
#[derive(Debug)]
pub struct Client {
    client: BedrockClient,
}

impl Client {
    pub async fn new() -> Self {
        let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(CLAUDE_REGION)
            .load()
            .await;
        let client = BedrockClient::new(&sdk_config);
        Self { client }
    }
}
