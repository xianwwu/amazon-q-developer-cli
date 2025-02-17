use std::sync::Arc;

use aws_sdk_bedrockruntime::types::{
    ContentBlock as BedrockContentBlock,
    ContentBlockDelta,
    ContentBlockDeltaEvent,
    ContentBlockStart,
    ConversationRole as BedrockConversationRole,
    ConverseStreamMetadataEvent,
    ConverseStreamOutput,
    StopReason as BedrockStopReason,
    ToolUseBlock,
    ToolUseBlockStart,
};
use fig_os_shim::Context;
use tracing::{
    trace,
    warn,
};

use super::ToolUse;
use crate::cli::chat::ConversationRole;
use crate::cli::chat::client::bedrock::SendMessageOutput;
use crate::cli::chat::error::Error;
use crate::cli::chat::tools::{
    parse_tool,
    serde_value_to_document,
};
use crate::cli::chat::types::{
    Message,
    StopReason,
};

/// State associated with parsing a [SendMessageOutput] into a [Message].
///
/// # Usage
///
/// You should repeatedly call [Self::recv] to receive [ResponseEvent]'s until a
/// [ResponseEvent::EndStream] value is returned.
#[derive(Debug)]
pub struct ResponseParser {
    _ctx: Arc<Context>,
    /// The response to consume and parse into a sequence of [Ev].
    response: SendMessageOutput,
    /// The list of [ContentBlock] items to be used in the final parsed message.
    content: Vec<BedrockContentBlock>,
    /// The [StopReason] for the associated [SendMessageOutput].
    stop_reason: Option<StopReason>,
    assistant_text: String,
    metadata_event: Option<ConverseStreamMetadataEvent>,
}

impl ResponseParser {
    pub fn new(ctx: Arc<Context>, response: SendMessageOutput) -> Self {
        Self {
            _ctx: ctx,
            response,
            content: Vec::new(),
            stop_reason: None,
            assistant_text: String::new(),
            metadata_event: None,
        }
    }

    /// Consumes the associated [SendMessageOutput] until a valid [ResponseEvent] is parsed.
    pub async fn recv(&mut self) -> Result<ResponseEvent, Error> {
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
                        ConverseStreamOutput::MessageStop(event) => match event.stop_reason {
                            BedrockStopReason::EndTurn | BedrockStopReason::ToolUse => {
                                assert!(self.stop_reason.is_none());
                                self.stop_reason = Some(event.stop_reason.into());
                            },
                            other => {
                                warn!("Unhandled message stop reason: {:?}", other);
                            },
                        },
                        ConverseStreamOutput::Metadata(event) => {
                            if self.stop_reason.is_none() {
                                warn!(?event, "Unexpected Metadata event before MessageStop");
                            }
                            self.metadata_event = Some(event);

                            // Conversation id's are defined by the Q model. Just doing a random
                            // one here for consistency sake.
                            return Ok(ResponseEvent::ConversationId(rand::random::<u32>().to_string()));
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
                    return Ok(ResponseEvent::EndStream { stop_reason, message });
                },
                Err(err) => return Err(Error::Sdk(err)),
            }
        }
    }

    async fn parse_tool_use(&mut self, start: ToolUseBlockStart) -> Result<ToolUse, Error> {
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
                    return Err(Error::Custom(
                        format!("Received unexpected event while parsing a tool use: {:?}", event).into(),
                    ));
                },
                Err(err) => return Err(Error::Sdk(err)),
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

        parse_tool(tool_name, value).map(|tool| ToolUse {
            tool_use_id: start.tool_use_id,
            tool,
        })
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
