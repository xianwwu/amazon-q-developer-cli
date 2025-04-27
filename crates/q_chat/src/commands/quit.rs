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

/// Quit command handler
pub struct QuitCommand;

impl QuitCommand {
    /// Create a new quit command handler
    pub fn new() -> Self {
        Self
    }
}

impl CommandHandler for QuitCommand {
    fn name(&self) -> &'static str {
        "quit"
    }

    fn description(&self) -> &'static str {
        "Quit the application"
    }

    fn usage(&self) -> &'static str {
        "/quit"
    }

    fn help(&self) -> String {
        "Exit the Amazon Q chat application".to_string()
    }

    fn execute<'a>(
        &'a self,
        _args: Vec<&'a str>,
        _ctx: &'a mut CommandContextAdapter<'a>,
        _tool_uses: Option<Vec<QueuedTool>>,
        _pending_tool_index: Option<usize>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatState>> + Send + 'a>> {
        Box::pin(async move { Ok(ChatState::Exit) })
    }

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        true // Quit command requires confirmation
    }
}
