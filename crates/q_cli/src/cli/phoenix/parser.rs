use std::borrow::Cow;
use std::sync::Arc;

use aws_sdk_bedrockruntime::error::SdkError;
use aws_sdk_bedrockruntime::operation::converse_stream::{
    ConverseStreamError,
    // ConverseStreamOutput as ConverseStreamResponse,
};
use aws_sdk_bedrockruntime::types::builders::MessageBuilder;
use aws_sdk_bedrockruntime::types::error::ConverseStreamOutputError;
use aws_sdk_bedrockruntime::types::{
    ContentBlock as BedrockContentBlock,
    ContentBlockDelta,
    ContentBlockDeltaEvent,
    ContentBlockStart,
    ConversationRole as BedrockConversationRole,
    ConverseStreamMetadataEvent,
    ConverseStreamOutput,
    Message as BedrockMessage,
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

use super::client::ConverseStreamResponse;
use super::error::Error;
use super::tools::{
    Tool,
    ToolConfig,
};
use super::types::StopReason;
use super::{
    ConversationRole,
    Message,
};
use crate::cli::phoenix::tools::{
    new_tool,
    serde_value_to_document,
};

/// State associated with parsing a [ConverseStreamResponse] into a [Message].
///
/// # Usage
///
/// You should repeatedly call [Self::recv] to receive [ResponseEvent]'s until a
/// [ResponseEvent::EndStream] value is returned.
#[derive(Debug)]
pub struct ResponseParser {
    ctx: Arc<Context>,
    /// The response to consume and parse into a sequence of [Ev].
    response: ConverseStreamResponse,
    /// The [ToolConfig] used to generate the response.
    tool_config: ToolConfig,
    /// The list of [ContentBlock] items to be used in the final parsed message.
    content: Vec<BedrockContentBlock>,
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

    /// Consumes the associated [ConverseStreamResponse] until a valid [ResponseEvent] is parsed.
    pub async fn recv(&mut self) -> Result<ResponseEvent, Error> {
        loop {
            match self.response.recv().await {
                Ok(Some(output)) => {
                    trace!(?output, "Received output");
                    match output {
                        ConverseStreamOutput::ContentBlockDelta(event) => match event.delta {
                            Some(ContentBlockDelta::Text(text)) => {
                                self.assistant_text.push_str(&text);
                                return Ok(ResponseEvent::AssistantText(text));
                            },
                            _ => {
                                return Err(Error::Custom(
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
                            self.content.push(BedrockContentBlock::Text(assistant_text));
                        },
                        ConverseStreamOutput::MessageStart(event) => {
                            assert!(event.role == BedrockConversationRole::Assistant);
                        },
                        ConverseStreamOutput::MessageStop(event) => {
                            match event.stop_reason {
                                StopReason::EndTurn | StopReason::ToolUse => {
                                    assert!(self.stop_reason.is_none());
                                    self.stop_reason = Some(event.stop_reason);
                                },
                                StopReason::MaxTokens => {
                                    // todo - how to handle max tokens?
                                    return Err(Error::MaxTokensReached("Max tokens reached".into()));
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
                            return Err(Error::Custom(
                                "Unexpected end of stream before receiving a stop reason".into(),
                            ));
                        },
                    };
                    let content = std::mem::take(&mut self.content);
                    let message = Message::new(
                        ConversationRole::Assistant,
                        content.into_iter().map(Into::into).collect(),
                    );
                    return Ok(ResponseEvent::EndStream {
                        stop_reason,
                        message,
                    });
                },
                Err(err) => return Err(Error::SdkError(err)),
            }
        }
    }

    async fn parse_tool_use(&mut self, start: ToolUseBlockStart) -> Result<ToolUse, Error> {
        let mut tool_args = String::new();
        let tool_name = &start.name;
        loop {
            match self.response.recv().await {
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
                    return Err(Error::Custom(
                        format!("Received unexpected event while parsing a tool use: {:?}", event).into(),
                    ));
                },
                Err(err) => return Err(Error::SdkError(err)),
            }
        }
        let value: serde_json::Value = serde_json::from_str(&tool_args)?;
        self.content.push(BedrockContentBlock::ToolUse(
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
            None => Err(Error::UnknownToolUse {
                tool_name: tool_name.clone(),
            }),
        }
    }
}

#[derive(Debug)]
pub enum ResponseEvent {
    /// Conversation identifier returned by the backend.
    ConversationId(String),
    /// Text returned by the assistant. This should be displayed to the user as it is received.
    AssistantText(String),
    /// A tool use requested by the assistant. This should be displayed to the user as it is
    /// received.
    ToolUse(ToolUse),
    /// Represents the end of the response. No more events will be returned.
    EndStream {
        /// Indicates the response ended.
        stop_reason: StopReason,
        /// The completed message containing all of the assistant text and tool use events
        /// previously emitted. This should be stored in the conversation history and sent in
        /// future conversation messages.
        message: Message,
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

/// Represents a tool use requested by the assistant.
#[derive(Debug)]
pub struct ToolUse {
    /// Corresponds to the `"toolUseId"` returned by the model.
    pub tool_use_id: String,
    pub tool: Box<dyn Tool + Sync>,
}
