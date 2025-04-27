use std::future::Future;
use std::pin::Pin;

use eyre::Result;

use super::{
    CommandContextAdapter,
    CommandHandler,
};
use crate::command::{
    Command,
    ProfileSubcommand,
};
use crate::{
    ChatState,
    QueuedTool,
};

/// Profile command handler
pub struct ProfileCommand;

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
        r#"The profile command manages Amazon Q profiles.

Subcommands:
- list: List all available profiles
- create <name>: Create a new profile
- delete <name>: Delete an existing profile
- set <name>: Switch to a different profile
- rename <old_name> <new_name>: Rename an existing profile

Examples:
- "/profile list" - Lists all available profiles
- "/profile create work" - Creates a new profile named "work"
- "/profile set personal" - Switches to the "personal" profile
- "/profile delete test" - Deletes the "test" profile

To get the current profiles, use the command "/profile list" which will display all available profiles with the current one marked."#.to_string()
    }

    fn execute<'a>(
        &'a self,
        args: Vec<&'a str>,
        _ctx: &'a mut CommandContextAdapter<'a>,
        tool_uses: Option<Vec<QueuedTool>>,
        pending_tool_index: Option<usize>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatState>> + Send + 'a>> {
        Box::pin(async move {
            // Parse arguments to determine the subcommand
            let subcommand = if args.is_empty() {
                ProfileSubcommand::List
            } else if let Some(first_arg) = args.first() {
                match *first_arg {
                    "list" => ProfileSubcommand::List,
                    "set" => {
                        if args.len() < 2 {
                            return Err(eyre::eyre!("Missing profile name for set command"));
                        }
                        ProfileSubcommand::Set {
                            name: args[1].to_string(),
                        }
                    },
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
                    "rename" => {
                        if args.len() < 3 {
                            return Err(eyre::eyre!("Missing old or new profile name for rename command"));
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
                ProfileSubcommand::List // Fallback, should not happen
            };

            Ok(ChatState::ExecuteCommand {
                command: Command::Profile { subcommand },
                tool_uses,
                pending_tool_index,
            })
        })
    }

    fn requires_confirmation(&self, args: &[&str]) -> bool {
        if args.is_empty() {
            return false; // Default list doesn't require confirmation
        }

        match args[0] {
            "list" | "help" => false, // Read-only commands don't require confirmation
            "delete" => true,         // Delete always requires confirmation
            _ => false,               // Other commands don't require confirmation
        }
    }
}
