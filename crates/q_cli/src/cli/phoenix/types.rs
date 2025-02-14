//! Wrappers and re-exports around the Bedrock SDK types.
//!
//! This is intended to make refactoring to the actual client a bit more straightforward - we
//! should ideally only need to make updates to this file rather than all of the tool/parsing/CLI
//! files in tandem.

use aws_sdk_bedrockruntime::types::{
    ContentBlock as BedrockContentBlock,
    ConversationRole as BedrockConversationRole,
    DocumentBlock,
    Message as BedrockMessage,
    StopReason as BedrockStopReason,
    ToolResultBlock,
    ToolResultContentBlock,
    ToolResultStatus,
    ToolUseBlock as BedrockToolUseBlock,
};

#[derive(Debug, Clone)]
pub enum ConversationRole {
    User,
    Assistant,
}

impl From<ConversationRole> for BedrockConversationRole {
    fn from(value: ConversationRole) -> Self {
        match value {
            ConversationRole::User => Self::User,
            ConversationRole::Assistant => Self::Assistant,
        }
    }
}

pub type StopReason = BedrockStopReason;

pub type ChatMessage = fig_api_client::model::ChatMessage;

#[derive(Debug, Clone)]
pub struct Message {
    role: ConversationRole,
    content: Vec<ContentBlock>,
}

impl Message {
    pub fn new(role: ConversationRole, content: Vec<ContentBlock>) -> Self {
        Self { role, content }
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

#[derive(Debug, Clone)]
pub enum ContentBlock {
    Document(DocumentBlock),
    Text(String),
    ToolUse(BedrockToolUseBlock),
    ToolResult(ToolResultBlock),
}

impl From<ContentBlock> for BedrockContentBlock {
    fn from(value: ContentBlock) -> Self {
        match value {
            ContentBlock::Document(document_block) => Self::Document(document_block),
            ContentBlock::Text(text) => Self::Text(text),
            ContentBlock::ToolUse(tool_use_block) => Self::ToolUse(tool_use_block),
            ContentBlock::ToolResult(tool_result_block) => Self::ToolResult(tool_result_block),
        }
    }
}

impl From<BedrockContentBlock> for ContentBlock {
    fn from(value: BedrockContentBlock) -> Self {
        match value {
            BedrockContentBlock::Document(document_block) => Self::Document(document_block),
            BedrockContentBlock::Text(text) => Self::Text(text),
            BedrockContentBlock::ToolResult(tool_result_block) => Self::ToolResult(tool_result_block),
            BedrockContentBlock::ToolUse(tool_use_block) => Self::ToolUse(tool_use_block),
            other => panic!("Unsupported content: {:?}", other),
        }
    }
}
