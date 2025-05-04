use eyre::Result;

use crate::command::{
    Command,
    ToolsSubcommand,
};
use crate::commands::handler::CommandHandler;

/// Static instance of the tools help command handler
pub static HELP_TOOLS_HANDLER: HelpToolsCommand = HelpToolsCommand;

/// Handler for the tools help command
pub struct HelpToolsCommand;

impl Default for HelpToolsCommand {
    fn default() -> Self {
        Self
    }
}

impl CommandHandler for HelpToolsCommand {
    fn name(&self) -> &'static str {
        "tools help"
    }

    fn description(&self) -> &'static str {
        "Display help information for the tools command"
    }

    fn usage(&self) -> &'static str {
        "/tools help"
    }

    fn help(&self) -> String {
        "Displays help information for the tools command and its subcommands.".to_string()
    }

    fn to_command(&self, _args: Vec<&str>) -> Result<Command> {
        Ok(Command::Tools {
            subcommand: Some(ToolsSubcommand::Help),
        })
    }

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        false
    }
}
