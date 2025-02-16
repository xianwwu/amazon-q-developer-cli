//! Wrappers and re-exports around the Bedrock SDK types.
//!
//! This is intended to make refactoring to the actual client a bit more straightforward - we
//! should ideally only need to make updates to this file rather than all of the tool/parsing/CLI
//! files in tandem.

pub mod bedrock;
pub use bedrock::*;

// pub mod q;
// pub use q::*;

#[derive(Debug, Clone)]
pub enum ConversationRole {
    User,
    Assistant,
}

#[derive(Debug, Clone)]
pub enum StopReason {
    EndTurn,
    ToolUse,
}

