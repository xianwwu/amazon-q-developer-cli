use eyre::Result;

use super::CommandHandler;
use crate::command::{
    Command,
    ProfileSubcommand,
};

mod create;
mod delete;
mod help;
mod list;
mod rename;
mod set;

// Static handlers for profile subcommands
pub use create::{
    CREATE_PROFILE_HANDLER,
    CreateProfileCommand,
};
pub use delete::{
    DELETE_PROFILE_HANDLER,
    DeleteProfileCommand,
};
pub use help::{
    HELP_PROFILE_HANDLER,
    HelpProfileCommand,
};
pub use list::{
    LIST_PROFILE_HANDLER,
    ListProfileCommand,
};
pub use rename::{
    RENAME_PROFILE_HANDLER,
    RenameProfileCommand,
};
pub use set::{
    SET_PROFILE_HANDLER,
    SetProfileCommand,
};

/// Profile command handler
pub struct ProfileCommand;

/// Static instance of the profile command handler
pub static PROFILE_HANDLER: ProfileCommand = ProfileCommand;

impl ProfileCommand {
    /// Create a new profile command handler
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProfileCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandHandler for ProfileCommand {
    fn name(&self) -> &'static str {
        "profile"
    }

    fn description(&self) -> &'static str {
        "Manage profiles"
    }

    fn usage(&self) -> &'static str {
        "/profile [subcommand]"
    }

    fn help(&self) -> String {
        "Manage profiles for the chat session.\n\n\
        Subcommands:\n\
        help        Show profile help\n\
        list        List profiles\n\
        set         Set the current profile\n\
        create      Create a new profile\n\
        delete      Delete a profile\n\
        rename      Rename a profile"
            .to_string()
    }

    fn llm_description(&self) -> String {
        r#"The profile command manages different profiles for organizing context files.

Subcommands:
- list: List all available profiles
- create <name>: Create a new profile
- delete <name>: Delete a profile
- set <name>: Switch to a different profile
- rename <old_name> <new_name>: Rename a profile

Examples:
- "/profile list" - Lists all available profiles
- "/profile create work" - Creates a new profile named "work"
- "/profile set work" - Switches to the "work" profile
- "/profile delete old_profile" - Deletes the profile named "old_profile"
- "/profile rename work work_new" - Renames the "work" profile to "work_new"

Profiles allow you to organize context files for different projects or tasks. The "global" profile contains context files that are available in all profiles."#
            .to_string()
    }

    fn to_command(&self, args: Vec<&str>) -> Result<Command> {
        // Parse arguments to determine the subcommand
        let subcommand = if args.is_empty() {
            ProfileSubcommand::Help
        } else if let Some(first_arg) = args.first() {
            match *first_arg {
                "list" => ProfileSubcommand::List,
                "create" => {
                    if args.len() < 2 {
                        return Err(eyre::eyre!("Missing profile name for create command"));
                    }
                    ProfileSubcommand::Create {
                        name: args[1].to_string(),
                    }
                },
                "delete" => {
                    if args.len() < 2 {
                        return Err(eyre::eyre!("Missing profile name for delete command"));
                    }
                    ProfileSubcommand::Delete {
                        name: args[1].to_string(),
                    }
                },
                "set" => {
                    if args.len() < 2 {
                        return Err(eyre::eyre!("Missing profile name for set command"));
                    }
                    ProfileSubcommand::Set {
                        name: args[1].to_string(),
                    }
                },
                "rename" => {
                    if args.len() < 3 {
                        return Err(eyre::eyre!("Missing profile names for rename command"));
                    }
                    ProfileSubcommand::Rename {
                        old_name: args[1].to_string(),
                        new_name: args[2].to_string(),
                    }
                },
                "help" => ProfileSubcommand::Help,
                _ => ProfileSubcommand::Help,
            }
        } else {
            ProfileSubcommand::Help // Fallback, should not happen
        };

        Ok(Command::Profile { subcommand })
    }

    fn requires_confirmation(&self, args: &[&str]) -> bool {
        if args.is_empty() {
            return false; // Default help doesn't require confirmation
        }

        match args[0] {
            "list" | "help" => false, // Read-only commands don't require confirmation
            "delete" => true,         // Delete requires confirmation
            _ => false,               // Other commands don't require confirmation
        }
    }
}
