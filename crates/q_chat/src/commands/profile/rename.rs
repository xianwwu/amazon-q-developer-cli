use eyre::Result;

use crate::command::{
    Command,
    ProfileSubcommand,
};
use crate::commands::handler::CommandHandler;

/// Static instance of the profile rename command handler
pub static RENAME_PROFILE_HANDLER: RenameProfileCommand = RenameProfileCommand;

/// Handler for the profile rename command
pub struct RenameProfileCommand;

impl Default for RenameProfileCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl RenameProfileCommand {
    pub fn new() -> Self {
        Self
    }
}

impl CommandHandler for RenameProfileCommand {
    fn name(&self) -> &'static str {
        "rename"
    }

    fn description(&self) -> &'static str {
        "Rename a profile"
    }

    fn usage(&self) -> &'static str {
        "/profile rename <old_name> <new_name>"
    }

    fn help(&self) -> String {
        "Rename a profile from <old_name> to <new_name>.".to_string()
    }

    fn to_command(&self, args: Vec<&str>) -> Result<Command> {
        if args.len() != 2 {
            return Err(eyre::eyre!("Expected old_name and new_name arguments"));
        }

        let old_name = args[0].to_string();
        let new_name = args[1].to_string();

        Ok(Command::Profile {
            subcommand: ProfileSubcommand::Rename { old_name, new_name },
        })
    }

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        true // Rename command requires confirmation as it's a mutative operation
    }
}
