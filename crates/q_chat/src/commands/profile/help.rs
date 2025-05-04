use eyre::Result;

use crate::command::{
    Command,
    ProfileSubcommand,
};
use crate::commands::handler::CommandHandler;

/// Static instance of the profile help command handler
pub static HELP_PROFILE_HANDLER: HelpProfileCommand = HelpProfileCommand;

/// Handler for the profile help command
pub struct HelpProfileCommand;

impl Default for HelpProfileCommand {
    fn default() -> Self {
        Self
    }
}

impl CommandHandler for HelpProfileCommand {
    fn name(&self) -> &'static str {
        "profile help"
    }

    fn description(&self) -> &'static str {
        "Display help information for the profile command"
    }

    fn usage(&self) -> &'static str {
        "/profile help"
    }

    fn help(&self) -> String {
        "Displays help information for the profile command and its subcommands.".to_string()
    }

    fn to_command(&self, _args: Vec<&str>) -> Result<Command> {
        Ok(Command::Profile {
            subcommand: ProfileSubcommand::Help,
        })
    }

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        false
    }
}
