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

/// Help command handler
pub struct HelpCommand {
    help_text: String,
}

impl HelpCommand {
    /// Create a new help command handler
    pub fn new() -> Self {
        Self {
            help_text: crate::HELP_TEXT.to_string(),
        }
    }
}

impl Default for HelpCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandHandler for HelpCommand {
    fn name(&self) -> &'static str {
        "help"
    }

    fn description(&self) -> &'static str {
        "Show help information"
    }

    fn usage(&self) -> &'static str {
        "/help"
    }

    fn help(&self) -> String {
        "Show help information for all commands".to_string()
    }

    fn llm_description(&self) -> String {
        r#"The help command displays information about available commands.

Usage:
- /help                      Show general help information

Examples:
- "/help" - Shows general help information with a list of all available commands"#
            .to_string()
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
                command: crate::command::Command::Help {
                    help_text: Some(self.help_text.clone()),
                },
                tool_uses,
                pending_tool_index,
            })
        })
    }

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        false // Help command doesn't require confirmation
    }
}
