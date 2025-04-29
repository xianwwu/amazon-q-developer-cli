use eyre::Result;

use super::CommandHandler;
use crate::command::{
    Command,
    ContextSubcommand,
};

/// Context command handler
pub struct ContextCommand;

impl ContextCommand {
    /// Create a new context command handler
    pub fn new() -> Self {
        Self
    }
}

impl Default for ContextCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandHandler for ContextCommand {
    fn name(&self) -> &'static str {
        "context"
    }

    fn description(&self) -> &'static str {
        "Manage context files and hooks for the chat session"
    }

    fn usage(&self) -> &'static str {
        "/context [subcommand]"
    }

    fn help(&self) -> String {
        "Manage context files and hooks for the chat session.\n\n\
        Subcommands:\n\
        help        Show context help\n\
        show        Display current context rules configuration [--expand]\n\
        add         Add file(s) to context [--global] [--force]\n\
        rm          Remove file(s) from context [--global]\n\
        clear       Clear all files from current context [--global]\n\
        hooks       View and manage context hooks"
            .to_string()
    }

    fn llm_description(&self) -> String {
        r#"The context command manages files added as context to the conversation.

Subcommands:
- add <file_path>: Add a file as context
- rm <index_or_path>: Remove a context file by index or path
- clear: Remove all context files
- show: Display all current context files
- hooks: Manage context hooks

Examples:
- "/context add README.md" - Adds README.md as context
- "/context rm 2" - Removes the second context file
- "/context show" - Shows all current context files
- "/context clear" - Removes all context files

To get the current context files, use the command "/context show" which will display all current context files.
To see the full content of context files, use "/context show --expand"."#
            .to_string()
    }

    fn to_command(&self, args: Vec<&str>) -> Result<Command> {
        // Parse arguments to determine the subcommand
        let subcommand = if args.is_empty() {
            ContextSubcommand::Show { expand: false }
        } else if let Some(first_arg) = args.first() {
            match *first_arg {
                "show" => {
                    let expand = args.len() > 1 && args[1] == "--expand";
                    ContextSubcommand::Show { expand }
                },
                "add" => {
                    let mut global = false;
                    let mut force = false;
                    let mut paths = Vec::new();

                    for arg in &args[1..] {
                        match *arg {
                            "--global" => global = true,
                            "--force" => force = true,
                            _ => paths.push((*arg).to_string()),
                        }
                    }

                    ContextSubcommand::Add { global, force, paths }
                },
                "rm" | "remove" => {
                    let mut global = false;
                    let mut paths = Vec::new();

                    for arg in &args[1..] {
                        match *arg {
                            "--global" => global = true,
                            _ => paths.push((*arg).to_string()),
                        }
                    }

                    ContextSubcommand::Remove { global, paths }
                },
                "clear" => {
                    let global = args.len() > 1 && args[1] == "--global";
                    ContextSubcommand::Clear { global }
                },
                "help" => ContextSubcommand::Help,
                "hooks" => {
                    // Use the Command::parse_hooks function to parse hooks subcommands
                    // This ensures consistent behavior with the Command::parse method
                    let hook_parts: Vec<&str> = std::iter::once("hooks").chain(args.iter().copied()).collect();

                    match crate::command::Command::parse_hooks(&hook_parts) {
                        Ok(crate::command::Command::Context { subcommand }) => subcommand,
                        _ => ContextSubcommand::Hooks { subcommand: None },
                    }
                },
                _ => ContextSubcommand::Help,
            }
        } else {
            ContextSubcommand::Show { expand: false } // Fallback, should not happen
        };

        Ok(Command::Context { subcommand })
    }

    fn requires_confirmation(&self, args: &[&str]) -> bool {
        if args.is_empty() {
            return false; // Default show doesn't require confirmation
        }

        match args[0] {
            "show" | "help" | "hooks" => false, // Read-only commands don't require confirmation
            _ => true,                          // All other subcommands require confirmation
        }
    }
}
