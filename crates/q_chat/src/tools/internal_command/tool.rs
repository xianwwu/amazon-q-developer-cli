use std::io::Write;

use crossterm::queue;
use crossterm::style::{
    self,
    Color,
};
use eyre::Result;
use fig_os_shim::Context;
use tracing::debug;

use crate::ChatState;
use crate::command::Command;
use crate::commands::registry::CommandRegistry;
use crate::tools::InvokeOutput;
use crate::tools::internal_command::schema::InternalCommand;

impl InternalCommand {
    /// Validate that the command exists
    pub fn validate_simple(&self) -> Result<()> {
        // Validate that the command is one of the known commands
        let cmd = self.command.trim_start_matches('/');

        // Check if the command exists in the command registry
        if CommandRegistry::global().command_exists(cmd) {
            return Ok(());
        }

        // For commands not in the registry, return an error
        Err(eyre::eyre!("Unknown command: {}", self.command))
    }

    /// Check if the command requires user acceptance
    pub fn requires_acceptance_simple(&self) -> bool {
        let cmd = self.command.trim_start_matches('/');

        // Try to get the handler from the registry
        if let Some(handler) = CommandRegistry::global().get(cmd) {
            // Convert args to string slices for the handler
            let args: Vec<&str> = match &self.subcommand {
                Some(subcommand) => vec![subcommand.as_str()],
                None => vec![],
            };

            return handler.requires_confirmation(&args);
        }

        // For commands not in the registry, default to requiring confirmation
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

        // Try to get the description from the command registry
        if let Some(handler) = CommandRegistry::global().get(cmd) {
            return handler.description().to_string();
        }

        // For commands not in the registry, return a generic description
        "Execute a command in the Q chat system".to_string()
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
    /// It formats the command string and returns a ChatState::ExecuteCommand state that will
    /// be handled by the chat loop.
    ///
    /// # Arguments
    ///
    /// * `_context` - The context for the command execution
    /// * `updates` - A writer for outputting status updates
    ///
    /// # Returns
    ///
    /// * `Result<InvokeOutput>` - The result of the command execution
    pub async fn invoke(&self, _context: &Context, updates: &mut impl Write) -> Result<InvokeOutput> {
        // Format the command string for execution
        let command_str = self.format_command_string();
        let description = self.get_command_description();

        // Write the command to the output
        writeln!(updates, "{}", command_str)?;

        // Create a response with the command and description
        let response = format!("Executing command for you: `{}` - {}", command_str, description);

        // Log the command string
        debug!("Executing command: {}", command_str);

        // Get the command handler from the registry
        let cmd = self.command.trim_start_matches('/');
        let command_registry = CommandRegistry::global();

        if let Some(handler) = command_registry.get(cmd) {
            // Convert args to a Vec<&str>
            let args = self
                .args
                .as_ref()
                .map(|args| args.iter().map(|s| s.as_str()).collect())
                .unwrap_or_default();

            // Use to_command to get the Command enum
            match handler.to_command(args) {
                Ok(command) => {
                    // Return an InvokeOutput with the response and next state
                    Ok(InvokeOutput {
                        output: crate::tools::OutputKind::Text(response),
                        next_state: Some(ChatState::ExecuteCommand {
                            command,
                            tool_uses: None,
                            pending_tool_index: None,
                        }),
                    })
                },
                Err(e) => {
                    // Return an InvokeOutput with the error message and no next state
                    Ok(InvokeOutput {
                        output: crate::tools::OutputKind::Text(format!("Error parsing command: {}", e)),
                        next_state: None,
                    })
                },
            }
        } else {
            // Try to parse the command using the old method as fallback
            match Command::parse(&command_str, &mut std::io::stdout()) {
                Ok(command) => {
                    // Return an InvokeOutput with the response and next state
                    Ok(InvokeOutput {
                        output: crate::tools::OutputKind::Text(response),
                        next_state: Some(ChatState::ExecuteCommand {
                            command,
                            tool_uses: None,
                            pending_tool_index: None,
                        }),
                    })
                },
                Err(e) => {
                    // Return an InvokeOutput with the error message and no next state
                    Ok(InvokeOutput {
                        output: crate::tools::OutputKind::Text(e),
                        next_state: None,
                    })
                },
            }
        }
    }
}
