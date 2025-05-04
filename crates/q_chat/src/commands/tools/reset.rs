use eyre::Result;

use crate::command::{
    Command,
    ToolsSubcommand,
};
use crate::commands::handler::CommandHandler;

/// Static instance of the tools reset command handler
pub static RESET_TOOLS_HANDLER: ResetToolsCommand = ResetToolsCommand;

/// Handler for the tools reset command
pub struct ResetToolsCommand;

impl Default for ResetToolsCommand {
    fn default() -> Self {
        Self
    }
}

impl CommandHandler for ResetToolsCommand {
    fn name(&self) -> &'static str {
        "tools reset"
    }

    fn description(&self) -> &'static str {
        "Reset all tool permissions to their default state"
    }

    fn usage(&self) -> &'static str {
        "/tools reset"
    }

    fn help(&self) -> String {
        "Resets all tool permissions to their default state. This will clear any previously granted permissions."
            .to_string()
    }

    fn to_command(&self, _args: Vec<&str>) -> Result<Command> {
        Ok(Command::Tools {
            subcommand: Some(ToolsSubcommand::Reset),
        })
    }

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        true // Reset is destructive, so require confirmation
    }
}
