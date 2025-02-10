use std::borrow::Cow;
use std::sync::Arc;

use aws_sdk_bedrockruntime::error::SdkError;
use aws_sdk_bedrockruntime::operation::converse_stream::{
    ConverseStreamError,
    ConverseStreamOutput as ConverseStreamResponse,
};
use aws_sdk_bedrockruntime::types::builders::MessageBuilder;
use aws_sdk_bedrockruntime::types::error::ConverseStreamOutputError;
use aws_sdk_bedrockruntime::types::{
    ContentBlock,
    ContentBlockDelta,
    ContentBlockDeltaEvent,
    ContentBlockStart,
    ConversationRole,
    ConverseStreamMetadataEvent,
    ConverseStreamOutput,
    Message,
    StopReason,
    ToolUseBlock,
    ToolUseBlockStart,
};
use aws_smithy_types::event_stream::RawMessage;
use fig_os_shim::Context;
use thiserror::Error;
use tracing::{
    error,
    trace,
    warn,
};

use super::tools::{
    Tool,
    ToolConfig,
    ToolError,
};
use crate::cli::achat::tools::{
    new_tool,
    serde_value_to_document,
};

/// State associated with parsing a [ConverseStreamResponse] into a [Message].
#[derive(Debug)]
pub struct ResponseParser {
    ctx: Arc<Context>,
    /// The response to consume and parse into a sequence of [Ev].
    response: ConverseStreamResponse,
    /// The [ToolConfig] used to generate the response.
    tool_config: ToolConfig,
    /// The list of [ContentBlock] items to be used in the final parsed message.
    content: Vec<ContentBlock>,
    /// The [StopReason] for the associated [ConverseStreamResponse].
    stop_reason: Option<StopReason>,
    assistant_text: String,
    metadata_event: Option<ConverseStreamMetadataEvent>,
}

impl ResponseParser {
    pub fn new(ctx: Arc<Context>, response: ConverseStreamResponse, tool_config: ToolConfig) -> Self {
        Self {
            ctx,
            response,
            content: Vec::new(),
            stop_reason: None,
            tool_config,
            assistant_text: String::new(),
            metadata_event: None,
        }
    }

    pub async fn recv(&mut self) -> Result<ResponseEvent, RecvError> {
        loop {
            match self.response.stream.recv().await {
                Ok(Some(output)) => {
                    trace!(?output, "Received output");
                    match output {
                        ConverseStreamOutput::ContentBlockDelta(event) => match event.delta {
                            Some(ContentBlockDelta::Text(text)) => {
                                self.assistant_text.push_str(&text);
                                return Ok(ResponseEvent::AssistantText(text));
                            },
                            _ => {
                                return Err(RecvError::Custom(
                                    format!("Unexpected event while reading the model response: {:?}", event).into(),
                                ));
                            },
                        },
                        ConverseStreamOutput::ContentBlockStart(event) => match event.start {
                            Some(ContentBlockStart::ToolUse(start)) => {
                                let tool_use = self.parse_tool_use(start).await?;
                                return Ok(ResponseEvent::ToolUse(tool_use));
                            },
                            ref other => {
                                warn!(?other, "Unexpected ContentBlockStart event that isn't a tool use");
                            },
                        },
                        ConverseStreamOutput::ContentBlockStop(event) => {
                            // This should only match for the AI response.
                            assert!(event.content_block_index == 0);
                            let assistant_text = std::mem::take(&mut self.assistant_text);
                            self.content.push(ContentBlock::Text(assistant_text));
                        },
                        ConverseStreamOutput::MessageStart(event) => {
                            assert!(event.role == ConversationRole::Assistant);
                        },
                        ConverseStreamOutput::MessageStop(event) => {
                            match event.stop_reason {
                                StopReason::EndTurn | StopReason::ToolUse => {
                                    assert!(self.stop_reason.is_none());
                                    self.stop_reason = Some(event.stop_reason);
                                },
                                StopReason::MaxTokens => {
                                    // todo - how to handle max tokens?
                                    return Err(RecvError::MaxTokensReached("uh oh".into()));
                                },
                                other => {
                                    warn!("Unhandled message stop reason: {}", other);
                                },
                            }
                        },
                        ConverseStreamOutput::Metadata(event) => {
                            if self.stop_reason.is_none() {
                                warn!(?event, "Unexpected Metadata event before MessageStop");
                            }
                            self.metadata_event = Some(event);
                        },
                        _ => (),
                    }
                },
                Ok(None) => {
                    let stop_reason = match self.stop_reason.take() {
                        Some(v) => v,
                        None => {
                            return Err(RecvError::Custom(
                                "Unexpected end of stream before receiving a stop reason".into(),
                            ));
                        },
                    };
                    let content = std::mem::take(&mut self.content);
                    let message = Message::builder()
                        .role(ConversationRole::Assistant)
                        .set_content(Some(content))
                        .build()
                        .expect("building the AI message should not fail");
                    return Ok(ResponseEvent::EndStream {
                        stop_reason,
                        message,
                        metadata: self.metadata_event.take().map(|ev| ev.into()),
                    });
                },
                Err(err) => return Err(RecvError::SdkError(err)),
            }
        }
    }

    async fn parse_tool_use(&mut self, start: ToolUseBlockStart) -> Result<ToolUse, RecvError> {
        let mut tool_args = String::new();
        let tool_name = &start.name;
        loop {
            match self.response.stream.recv().await {
                Ok(
                    ref l @ Some(ConverseStreamOutput::ContentBlockDelta(ContentBlockDeltaEvent {
                        delta: Some(ContentBlockDelta::ToolUse(ref tool)),
                        ..
                    })),
                ) => {
                    trace!(?l, "Received output");
                    tool_args.push_str(&tool.input);
                },
                Ok(ref l @ Some(ConverseStreamOutput::ContentBlockStop(_))) => {
                    trace!(?l, "Received output");
                    break;
                },
                Ok(event) => {
                    return Err(RecvError::Custom(
                        format!("Received unexpected event while parsing a tool use: {:?}", event).into(),
                    ));
                },
                Err(err) => return Err(RecvError::SdkError(err)),
            }
        }
        let value: serde_json::Value = serde_json::from_str(&tool_args)?;
        self.content.push(ContentBlock::ToolUse(
            ToolUseBlock::builder()
                .tool_use_id(start.tool_use_id.clone())
                .name(tool_name)
                .input(serde_value_to_document(value.clone()))
                .build()
                .unwrap(),
        ));
        match self.tool_config.get_by_name(tool_name) {
            Some(spec) => Ok(ToolUse {
                tool_use_id: start.tool_use_id,
                tool: new_tool(Arc::clone(&self.ctx), &spec.name, value)?,
            }),
            None => Err(RecvError::UnknownToolUse {
                tool_name: tool_name.clone(),
            }),
        }
    }
}

#[derive(Debug)]
pub enum ResponseEvent {
    AssistantText(String),
    ToolUse(ToolUse),
    EndStream {
        stop_reason: StopReason,
        message: Message,
        metadata: Option<Metadata>,
    },
}

/// Metadata associated with an assistant response, e.g. token usage.
#[derive(Debug)]
pub struct Metadata(ConverseStreamMetadataEvent);

impl From<ConverseStreamMetadataEvent> for Metadata {
    fn from(value: ConverseStreamMetadataEvent) -> Self {
        Self(value)
    }
}

#[derive(Debug, Error)]
pub enum RecvError {
    #[error(transparent)]
    SdkError(#[from] SdkError<ConverseStreamOutputError, RawMessage>),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    ToolError(#[from] ToolError),
    #[error("Model requested the use of an unknown tool: {tool_name}")]
    UnknownToolUse { tool_name: String },
    #[error("{0}")]
    MaxTokensReached(String),
    #[error("{0}")]
    Custom(Cow<'static, str>),
}

/// Represents a tool use requested by the assistant.
#[derive(Debug)]
pub struct ToolUse {
    /// Corresponds to the `"toolUseId"` returned by the model.
    pub tool_use_id: String,
    pub tool: Box<dyn Tool + Sync>,
}
