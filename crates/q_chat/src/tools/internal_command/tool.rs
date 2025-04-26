use std::io::Write;

use crossterm::queue;
use crossterm::style::{
    self,
    Color,
};
use eyre::Result;
use fig_os_shim::Context;
use tracing::{
    debug,
    info,
};

use crate::tools::InvokeOutput;
use crate::tools::internal_command::schema::InternalCommand;
use crate::command::{
    Command,
    ContextSubcommand,
    ProfileSubcommand,
    ToolsSubcommand,
};
use crate::ChatState;

impl InternalCommand {
    /// Validate that the command exists
    pub fn validate_simple(&self) -> Result<()> {
        // Validate that the command is one of the known commands
        let cmd = self.command.trim_start_matches('/');

        // Check if the command is one of the known commands
        match cmd {
            "quit" | "clear" | "help" | "context" | "profile" | "tools" | "issue" | "compact" | "editor" | "usage" => {
                Ok(())
            },
            _ => Err(eyre::eyre!("Unknown command: {}", self.command)),
        }
    }

    /// Check if the command requires user acceptance
    pub fn requires_acceptance_simple(&self) -> bool {
        // For read-only commands, don't require confirmation
        let cmd = self.command.trim_start_matches('/');
        match cmd {
            "help" | "usage" => return false,
            _ => {},
        }

        // For context show and profile list, don't require confirmation
        if cmd == "context" && self.subcommand.as_deref() == Some("show") {
            return false;
        }
        if cmd == "profile" && self.subcommand.as_deref() == Some("list") {
            return false;
        }

        // For all other commands, require acceptance
        true
    }

    /// Format the command string with subcommand and arguments
    pub fn format_command_string(&self) -> String {
        // Start with the base command
        let mut cmd_str = if !self.command.starts_with('/') {
            format!("/{}", self.command)
        } else {
            self.command.clone()
        };

        // Add subcommand if present
        if let Some(subcommand) = &self.subcommand {
            cmd_str.push_str(&format!(" {}", subcommand));
        }

        // Add arguments if present
        if let Some(args) = &self.args {
            for arg in args {
                cmd_str.push_str(&format!(" {}", arg));
            }
        }

        // Add flags if present
        if let Some(flags) = &self.flags {
            for (flag, value) in flags {
                if value.is_empty() {
                    cmd_str.push_str(&format!(" --{}", flag));
                } else {
                    cmd_str.push_str(&format!(" --{}={}", flag, value));
                }
            }
        }

        cmd_str
    }

    /// Get a description for the command
    pub fn get_command_description(&self) -> String {
        let cmd = self.command.trim_start_matches('/');

        match cmd {
            "quit" => "Exit the chat session".to_string(),
            "clear" => "Clear the current conversation history".to_string(),
            "help" => "Show help information about available commands".to_string(),
            "context" => match self.subcommand.as_deref() {
                Some("add") => "Add a file to the conversation context".to_string(),
                Some("rm" | "remove") => "Remove a file from the conversation context".to_string(),
                Some("clear") => "Clear all files from the conversation context".to_string(),
                Some("show") => "Show all files in the conversation context".to_string(),
                _ => "Manage conversation context files".to_string(),
            },
            "profile" => match self.subcommand.as_deref() {
                Some("list") => "List all available profiles".to_string(),
                Some("create") => "Create a new profile".to_string(),
                Some("delete") => "Delete an existing profile".to_string(),
                Some("set") => "Switch to a different profile".to_string(),
                Some("rename") => "Rename an existing profile".to_string(),
                _ => "Manage conversation profiles".to_string(),
            },
            "tools" => match self.subcommand.as_deref() {
                Some("list") => "List all available tools".to_string(),
                Some("enable") => "Enable a specific tool".to_string(),
                Some("disable") => "Disable a specific tool".to_string(),
                Some("trust") => "Trust a specific tool for this session".to_string(),
                Some("untrust") => "Remove trust for a specific tool".to_string(),
                _ => "Manage tool permissions and settings".to_string(),
            },
            "issue" => "Create a GitHub issue for reporting bugs or feature requests".to_string(),
            "compact" => "Summarize and compact the conversation history".to_string(),
            "editor" => "Open an external editor to compose a prompt".to_string(),
            "usage" => "Show current session's context window usage".to_string(),
            _ => "Execute a command in the Q chat system".to_string(),
        }
    }

    /// Queue description for the command execution
    pub fn queue_description(&self, updates: &mut impl Write) -> Result<()> {
        let command_str = self.format_command_string();

        queue!(
            updates,
            style::SetForegroundColor(Color::Blue),
            style::Print("Suggested command: "),
            style::SetForegroundColor(Color::Yellow),
            style::Print(&command_str),
            style::ResetColor,
            style::Print("\n"),
        )?;

        Ok(())
    }

    /// Invoke the internal command tool
    ///
    /// This method executes the internal command and returns an InvokeOutput with the result.
    /// It parses the command into a Command enum and returns a ChatState::ExecuteParsedCommand
    /// state that will be handled by the chat loop.
    ///
    /// # Arguments
    ///
    /// * `_context` - The context for the command execution
    /// * `_updates` - A writer for outputting status updates
    ///
    /// # Returns
    ///
    /// * `Result<InvokeOutput>` - The result of the command execution
    pub async fn invoke(&self, _context: &Context, _updates: &mut impl Write) -> Result<InvokeOutput> {
        // Format the command string for execution
        let command_str = self.format_command_string();
        let description = self.get_command_description();

        // Log the command being executed
        info!("internal_command tool executing command: {}", command_str);
        debug!(
            "Command details - command: {}, subcommand: {:?}, args: {:?}, flags: {:?}",
            self.command, self.subcommand, self.args, self.flags
        );

        // Create a response with the suggested command and description
        let response = format!(
            "I suggest using the command: `{}` - {}\n\nExecuting this command for you.",
            command_str, description
        );

        // Parse the command into a Command enum
        use std::collections::HashSet;

        // Convert the command to a Command enum
        let parsed_command = match self.command.trim_start_matches('/') {
            "quit" => Command::Quit,
            "clear" => Command::Clear,
            "help" => Command::Help,
            "context" => {
                // Handle context subcommands
                match self.subcommand.as_deref() {
                    Some("add") => {
                        if let Some(args) = &self.args {
                            if !args.is_empty() {
                                let mut global = false;
                                let mut force = false;

                                // Check for flags
                                if let Some(flags) = &self.flags {
                                    if flags.contains_key("global") {
                                        global = true;
                                    }
                                    if flags.contains_key("force") {
                                        force = true;
                                    }
                                }

                                Command::Context {
                                    subcommand: ContextSubcommand::Add {
                                        global,
                                        force,
                                        paths: args.clone(),
                                    },
                                }
                            } else {
                                return Err(eyre::eyre!("Missing file path for context add command"));
                            }
                        } else {
                            return Err(eyre::eyre!("Missing file path for context add command"));
                        }
                    },
                    Some("rm" | "remove") => {
                        if let Some(args) = &self.args {
                            if !args.is_empty() {
                                let mut global = false;

                                // Check for flags
                                if let Some(flags) = &self.flags {
                                    if flags.contains_key("global") {
                                        global = true;
                                    }
                                }

                                Command::Context {
                                    subcommand: ContextSubcommand::Remove {
                                        global,
                                        paths: args.clone(),
                                    },
                                }
                            } else {
                                return Err(eyre::eyre!("Missing file path or index for context remove command"));
                            }
                        } else {
                            return Err(eyre::eyre!("Missing file path or index for context remove command"));
                        }
                    },
                    Some("clear") => {
                        let mut global = false;

                        // Check for flags
                        if let Some(flags) = &self.flags {
                            if flags.contains_key("global") {
                                global = true;
                            }
                        }

                        Command::Context {
                            subcommand: ContextSubcommand::Clear { global },
                        }
                    },
                    Some("show") => {
                        let mut expand = false;

                        // Check for flags
                        if let Some(flags) = &self.flags {
                            if flags.contains_key("expand") {
                                expand = true;
                            }
                        }

                        Command::Context {
                            subcommand: ContextSubcommand::Show { expand },
                        }
                    },
                    _ => return Err(eyre::eyre!("Unknown context subcommand: {:?}", self.subcommand)),
                }
            },
            "profile" => {
                // Handle profile subcommands
                match self.subcommand.as_deref() {
                    Some("list") => Command::Profile {
                        subcommand: ProfileSubcommand::List,
                    },
                    Some("create") => {
                        if let Some(args) = &self.args {
                            if !args.is_empty() {
                                Command::Profile {
                                    subcommand: ProfileSubcommand::Create { name: args[0].clone() },
                                }
                            } else {
                                return Err(eyre::eyre!("Missing profile name for profile create command"));
                            }
                        } else {
                            return Err(eyre::eyre!("Missing profile name for profile create command"));
                        }
                    },
                    Some("delete") => {
                        if let Some(args) = &self.args {
                            if !args.is_empty() {
                                Command::Profile {
                                    subcommand: ProfileSubcommand::Delete { name: args[0].clone() },
                                }
                            } else {
                                return Err(eyre::eyre!("Missing profile name for profile delete command"));
                            }
                        } else {
                            return Err(eyre::eyre!("Missing profile name for profile delete command"));
                        }
                    },
                    Some("set") => {
                        if let Some(args) = &self.args {
                            if !args.is_empty() {
                                Command::Profile {
                                    subcommand: ProfileSubcommand::Set { name: args[0].clone() },
                                }
                            } else {
                                return Err(eyre::eyre!("Missing profile name for profile set command"));
                            }
                        } else {
                            return Err(eyre::eyre!("Missing profile name for profile set command"));
                        }
                    },
                    Some("rename") => {
                        if let Some(args) = &self.args {
                            if args.len() >= 2 {
                                Command::Profile {
                                    subcommand: ProfileSubcommand::Rename {
                                        old_name: args[0].clone(),
                                        new_name: args[1].clone(),
                                    },
                                }
                            } else {
                                return Err(eyre::eyre!(
                                    "Missing old or new profile name for profile rename command"
                                ));
                            }
                        } else {
                            return Err(eyre::eyre!("Missing profile names for profile rename command"));
                        }
                    },
                    _ => return Err(eyre::eyre!("Unknown profile subcommand: {:?}", self.subcommand)),
                }
            },
            "tools" => {
                // Handle tools subcommands
                match self.subcommand.as_deref() {
                    Some("list") => Command::Tools {
                        subcommand: Some(ToolsSubcommand::Help),
                    },
                    Some("trust") => {
                        if let Some(args) = &self.args {
                            if !args.is_empty() {
                                let mut tool_names = HashSet::new();
                                tool_names.insert(args[0].clone());
                                Command::Tools {
                                    subcommand: Some(ToolsSubcommand::Trust { tool_names }),
                                }
                            } else {
                                return Err(eyre::eyre!("Missing tool name for tools trust command"));
                            }
                        } else {
                            return Err(eyre::eyre!("Missing tool name for tools trust command"));
                        }
                    },
                    Some("untrust") => {
                        if let Some(args) = &self.args {
                            if !args.is_empty() {
                                let mut tool_names = HashSet::new();
                                tool_names.insert(args[0].clone());
                                Command::Tools {
                                    subcommand: Some(ToolsSubcommand::Untrust { tool_names }),
                                }
                            } else {
                                return Err(eyre::eyre!("Missing tool name for tools untrust command"));
                            }
                        } else {
                            return Err(eyre::eyre!("Missing tool name for tools untrust command"));
                        }
                    },
                    Some("reset") => Command::Tools {
                        subcommand: Some(ToolsSubcommand::Reset),
                    },
                    _ => return Err(eyre::eyre!("Unknown tools subcommand: {:?}", self.subcommand)),
                }
            },
            "issue" => {
                let prompt = if let Some(args) = &self.args {
                    if !args.is_empty() { Some(args.join(" ")) } else { None }
                } else {
                    None
                };
                Command::Issue { prompt }
            },
            "compact" => {
                let mut show_summary = false;
                let mut help = false;
                let mut prompt = None;

                // Check for flags
                if let Some(flags) = &self.flags {
                    if flags.contains_key("summary") {
                        show_summary = true;
                    }
                    if flags.contains_key("help") {
                        help = true;
                    }
                }

                // Check for prompt
                if let Some(args) = &self.args {
                    if !args.is_empty() {
                        prompt = Some(args.join(" "));
                    }
                }

                Command::Compact {
                    prompt,
                    show_summary,
                    help,
                }
            },
            "editor" => {
                let initial_text = if let Some(args) = &self.args {
                    if !args.is_empty() { Some(args.join(" ")) } else { None }
                } else {
                    None
                };
                Command::PromptEditor { initial_text }
            },
            "usage" => Command::Usage,
            _ => return Err(eyre::eyre!("Unknown command: {}", self.command)),
        };

        // Log the parsed command
        debug!("Parsed command: {:?}", parsed_command);

        // Log the next state being returned
        debug!(
            "internal_command tool returning ChatState::ExecuteParsedCommand with command: {:?}",
            parsed_command
        );

        // Return an InvokeOutput with the response and next state
        Ok(InvokeOutput {
            output: crate::tools::OutputKind::Text(response),
            next_state: Some(ChatState::ExecuteParsedCommand(parsed_command)),
        })
    }
}
