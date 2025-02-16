use aws_sdk_bedrockruntime::types::{
    ContentBlock as BedrockContentBlock,
    ConversationRole as BedrockConversationRole,
    DocumentBlock,
    Message as BedrockMessage,
    StopReason as BedrockStopReason,
    ToolResultBlock as BedrockToolResult,
    ToolUseBlock as BedrockToolUseBlock,
};
pub use aws_sdk_bedrockruntime::types::{
    ToolResultContentBlock,
    ToolResultStatus,
};
use tracing::error;

use super::{
    ConversationRole,
    StopReason,
};
use crate::cli::phoenix::ToolConfiguration;

impl From<ConversationRole> for BedrockConversationRole {
    fn from(value: ConversationRole) -> Self {
        match value {
            ConversationRole::User => Self::User,
            ConversationRole::Assistant => Self::Assistant,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: Vec<ToolResultContentBlock>,
    pub status: ToolResultStatus,
}

impl From<ToolResult> for BedrockToolResult {
    fn from(value: ToolResult) -> Self {
        Self::builder()
            .tool_use_id(value.tool_use_id)
            .set_content(Some(value.content))
            .status(value.status)
            .build()
            .expect("building ToolResult should not fail")
    }
}

impl From<BedrockToolResult> for ToolResult {
    fn from(value: BedrockToolResult) -> Self {
        Self {
            tool_use_id: value.tool_use_id,
            content: value.content,
            status: value.status.unwrap_or(ToolResultStatus::Success),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConversationState {
    pub conversation_id: Option<String>,
    pub messages: Vec<Message>,
    pub next_message: Option<Message>,
    /// If not empty, contains the tool results to be sent back to the model.
    pub tool_results: Vec<ToolResult>,
    _tool_config: ToolConfiguration,
}

impl ConversationState {
    pub fn new(_tool_config: ToolConfiguration) -> Self {
        Self {
            conversation_id: None,
            messages: vec![],
            next_message: None,
            tool_results: vec![],
            _tool_config,
        }
    }

    pub fn append_new_user_message(&mut self, prompt: String) {
        if self.next_message.is_some() {
            error!("Replacing the next_message with a new message with input: {}", prompt);
        }
        let msg = Message::new(ConversationRole::User, vec![ContentBlock::Text(prompt)]);
        self.next_message = Some(msg);
    }

    pub fn push_assistant_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    pub fn add_tool_results(&mut self, mut results: Vec<ToolResult>) {
        self.tool_results.append(&mut results);
    }
}

#[derive(Debug, Clone)]
pub struct Message {
    role: ConversationRole,
    content: Vec<ContentBlock>,
}

impl Message {
    pub fn new(role: ConversationRole, content: Vec<ContentBlock>) -> Self {
        Self { role, content }
    }

    pub fn new_user_prompt(prompt: String) -> Self {
        Self::new(ConversationRole::User, vec![ContentBlock::Text(prompt)])
    }
}

impl From<Message> for BedrockMessage {
    fn from(value: Message) -> Self {
        Self::builder()
            .role(value.role.into())
            .set_content(Some(value.content.into_iter().map(Into::into).collect()))
            .build()
            .expect("building witih role and content set should not fail")
    }
}

impl From<BedrockStopReason> for StopReason {
    fn from(value: BedrockStopReason) -> Self {
        match value {
            BedrockStopReason::EndTurn => Self::EndTurn,
            BedrockStopReason::ToolUse => Self::ToolUse,
            other => panic!("unsupported bedrock stop reason: {}", other),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ContentBlock {
    Document(DocumentBlock),
    Text(String),
    ToolUse(BedrockToolUseBlock),
    ToolResult(ToolResult),
}

impl From<ContentBlock> for BedrockContentBlock {
    fn from(value: ContentBlock) -> Self {
        match value {
            ContentBlock::Document(document_block) => Self::Document(document_block),
            ContentBlock::Text(text) => Self::Text(text),
            ContentBlock::ToolUse(tool_use_block) => Self::ToolUse(tool_use_block),
            ContentBlock::ToolResult(tool_result_block) => Self::ToolResult(tool_result_block.into()),
        }
    }
}

impl From<BedrockContentBlock> for ContentBlock {
    fn from(value: BedrockContentBlock) -> Self {
        match value {
            BedrockContentBlock::Document(document_block) => Self::Document(document_block),
            BedrockContentBlock::Text(text) => Self::Text(text),
            BedrockContentBlock::ToolResult(tool_result_block) => Self::ToolResult(tool_result_block.into()),
            BedrockContentBlock::ToolUse(tool_use_block) => Self::ToolUse(tool_use_block),
            other => panic!("Unsupported content: {:?}", other),
        }
    }
}
