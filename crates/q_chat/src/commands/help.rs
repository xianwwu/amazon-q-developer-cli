use eyre::Result;

use super::CommandHandler;
use crate::command::Command;

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

    fn to_command(&self, _args: Vec<&str>) -> Result<Command> {
        Ok(Command::Help {
            help_text: Some(self.help_text.clone()),
        })
    }

    // Using the default implementation from the trait that calls to_command
    // No need to override execute anymore

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        false // Help command doesn't require confirmation
    }
}
