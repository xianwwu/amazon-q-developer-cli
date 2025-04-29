use eyre::Result;

use super::CommandHandler;
use crate::command::Command;

/// Clear command handler
pub struct ClearCommand;

impl ClearCommand {
    /// Create a new clear command handler
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClearCommand {
    fn default() -> Self {
        Self::new()
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

    fn llm_description(&self) -> String {
        r#"The clear command erases the conversation history and context from hooks for the current session.

Usage:
- /clear                     Clear the conversation history

This command will prompt for confirmation before clearing the history.

Examples of statements that may trigger this command:
- "Clear the conversation"
- "Start fresh"
- "Reset our chat"
- "Clear the chat history"
- "I want to start over"
- "Erase our conversation"
- "Let's start with a clean slate"
- "Clear everything"
- "Reset the context"
- "Wipe the conversation history""#
            .to_string()
    }

    fn to_command(&self, _args: Vec<&str>) -> Result<Command> {
        Ok(Command::Clear)
    }

    // Using the default implementation from the trait that calls to_command
    // No need to override execute anymore

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        true // Clear command requires confirmation
    }
}
