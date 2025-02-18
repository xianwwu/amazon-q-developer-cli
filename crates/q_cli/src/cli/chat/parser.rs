use std::sync::Arc;

use fig_api_client::clients::SendMessageOutput;
use fig_api_client::model::{
    AssistantResponseMessage,
    ChatMessage,
    ChatResponseStream,
};
use fig_os_shim::Context;
use tracing::{
    error,
    trace,
};

use crate::cli::chat::conversation_state::{
    Message,
    StopReason,
};
use crate::cli::chat::error::Error;
use crate::cli::chat::tools::{
    Tool,
    parse_tool,
};

/// Represents a tool use requested by the assistant.
#[derive(Debug)]
pub struct ToolUse {
    /// Corresponds to the `"toolUseId"` returned by the model.
    pub tool_use_id: String,
    pub tool_name: String,
    /// The tool arguments encoded as JSON.
    pub tool: String,
}

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
    response: SendMessageOutput,
    /// Buffer to hold the next event in [SendMessageOutput].
    peek: Option<ChatResponseStream>,
    /// Message identifier for the assistant's response.
    message_id: Option<String>,
    /// Buffer for holding the accumulated assistant response.
    assistant_text: String,
    /// Whether or not a tool use was received. Used to derive the [StopReason].
    received_tool_use: bool,
}

impl ResponseParser {
    pub fn new(ctx: Arc<Context>, response: SendMessageOutput) -> Self {
        Self {
            ctx,
            response,
            peek: None,
            message_id: None,
            assistant_text: String::new(),
            received_tool_use: false,
        }
    }

    /// Consumes the associated [ConverseStreamResponse] until a valid [ResponseEvent] is parsed.
    pub async fn recv(&mut self) -> Result<ResponseEvent, Error> {
        loop {
            match self.next().await {
                Ok(Some(output)) => {
                    trace!(?output, "Received output");
                    match output {
                        ChatResponseStream::AssistantResponseEvent { content } => {
                            self.assistant_text.push_str(&content);
                            return Ok(ResponseEvent::AssistantText(content));
                        },
                        ChatResponseStream::InvalidStateEvent { reason, message } => {
                            error!(%reason, %message, "invalid state event");
                        },
                        ChatResponseStream::MessageMetadataEvent {
                            conversation_id,
                            utterance_id,
                        } => {
                            if let Some(id) = utterance_id {
                                self.message_id = Some(id);
                            }
                            if let Some(id) = conversation_id {
                                return Ok(ResponseEvent::ConversationId(id));
                            }
                        },
                        ChatResponseStream::ToolUseEvent {
                            tool_use_id,
                            name,
                            input,
                            stop,
                        } => {
                            let tool_use = self.parse_tool_use(tool_use_id, name, input, stop).await?;
                            return Ok(ResponseEvent::ToolUse(tool_use));
                        },
                        _ => {},
                    }
                },
                Ok(None) => {
                    let stop_reason = if self.received_tool_use {
                        StopReason::ToolUse
                    } else {
                        StopReason::EndTurn
                    };
                    let message = Message(ChatMessage::AssistantResponseMessage(AssistantResponseMessage {
                        message_id: self.message_id.take(),
                        content: std::mem::take(&mut self.assistant_text),
                    }));
                    return Ok(ResponseEvent::EndStream { stop_reason, message });
                },
                Err(err) => return Err(err.into()),
            }
        }
    }

    /// Consumes the response stream until a valid [ToolUse] is parsed.
    ///
    /// The arguments are the fields from the first [ChatResponseStream::ToolUseEvent] consumed.
    async fn parse_tool_use(
        &mut self,
        tool_use_id: String,
        tool_name: String,
        mut input: Option<String>,
        stop: Option<bool>,
    ) -> Result<ToolUse, Error> {
        assert!(input.is_some());
        assert!(stop.is_none_or(|v| !v));
        let mut tool_string = input.take().unwrap_or_default();
        while let Some(ChatResponseStream::ToolUseEvent { .. }) = self.peek().await? {
            if let Some(ChatResponseStream::ToolUseEvent { input, stop, .. }) = self.next().await? {
                if let Some(i) = input {
                    tool_string.push_str(&i);
                }
                if let Some(true) = stop {
                    break;
                }
            }
        }
        self.assistant_text.push_str(&tool_string);
        Ok(ToolUse {
            tool_use_id,
            tool_name,
            tool: tool_string,
        })
    }

    /// Returns the next event in the [SendMessageOutput] without consuming it.
    async fn peek(&mut self) -> Result<Option<&ChatResponseStream>, fig_api_client::Error> {
        if self.peek.is_some() {
            return Ok(self.peek.as_ref());
        }
        match self.next().await? {
            Some(v) => {
                self.peek = Some(v);
                Ok(self.peek.as_ref())
            },
            None => Ok(None),
        }
    }

    /// Consumes the next [SendMessageOutput] event.
    async fn next(&mut self) -> Result<Option<ChatResponseStream>, fig_api_client::Error> {
        if let Some(ev) = self.peek.take() {
            return Ok(Some(ev));
        }
        self.response.recv().await
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
        /// subsequent requests.
        message: Message,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse() {
        let tool_use_id = "TEST_ID".to_string();
        let tool_name = "execute_bash".to_string();
        let tool_use = serde_json::json!({
            "command": "echo hello"
        })
        .to_string();
        let tool_use_split_at = 5;
        let mut events = vec![
            ChatResponseStream::AssistantResponseEvent {
                content: "hi".to_string(),
            },
            ChatResponseStream::AssistantResponseEvent {
                content: " there".to_string(),
            },
            ChatResponseStream::ToolUseEvent {
                tool_use_id: tool_use_id.clone(),
                name: tool_name.clone(),
                input: Some(tool_use.as_str().split_at(tool_use_split_at).0.to_string()),
                stop: None,
            },
            ChatResponseStream::ToolUseEvent {
                tool_use_id: tool_use_id.clone(),
                name: tool_name.clone(),
                input: Some(tool_use.as_str().split_at(tool_use_split_at).1.to_string()),
                stop: None,
            },
            ChatResponseStream::ToolUseEvent {
                tool_use_id: tool_use_id.clone(),
                name: tool_name.clone(),
                input: None,
                stop: Some(true),
            },
        ];
        events.reverse();
        let mock = SendMessageOutput::Mock(events);
        let mut parser = ResponseParser::new(Context::new_fake(), mock);

        for _ in 0..5 {
            println!("{:?}", parser.recv().await.unwrap());
        }
    }
}
