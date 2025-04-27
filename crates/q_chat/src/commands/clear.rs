use std::future::Future;
use std::pin::Pin;

use eyre::Result;

use super::{
    CommandContextAdapter,
    CommandHandler,
};
use crate::{
    ChatState,
    QueuedTool,
};

/// Clear command handler
pub struct ClearCommand;

impl ClearCommand {
    /// Create a new clear command handler
    pub fn new() -> Self {
        Self
    }
}

impl CommandHandler for ClearCommand {
    fn name(&self) -> &'static str {
        "clear"
    }

    fn description(&self) -> &'static str {
        "Clear the conversation history"
    }

    fn usage(&self) -> &'static str {
        "/clear"
    }

    fn help(&self) -> String {
        "Clear the conversation history and context from hooks for the current session".to_string()
    }

    fn execute<'a>(
        &'a self,
        _args: Vec<&'a str>,
        _ctx: &'a mut CommandContextAdapter<'a>,
        tool_uses: Option<Vec<QueuedTool>>,
        pending_tool_index: Option<usize>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatState>> + Send + 'a>> {
        Box::pin(async move {
            Ok(ChatState::ExecuteCommand {
                command: crate::command::Command::Clear,
                tool_uses,
                pending_tool_index,
            })
        })
    }

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        true // Clear command requires confirmation
    }
}
