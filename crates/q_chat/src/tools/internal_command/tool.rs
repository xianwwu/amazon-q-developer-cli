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
use crate::tools::internal_command::schema::InternalCommand;
use crate::tools::{
    InvokeOutput,
    OutputKind,
};

impl InternalCommand {
    /// Validate that the command exists
    pub fn validate_simple(&self) -> Result<()> {
        // Format a simple command string
        let cmd_str = if !self.command.starts_with('/') {
            format!("/{}", self.command)
        } else {
            self.command.clone()
        };

        // Try to parse the command using the Command::parse method
        match Command::parse(&cmd_str) {
            Ok(_) => Ok(()),
            Err(e) => Err(eyre::eyre!("Unknown command: {} - {}", self.command, e)),
        }
    }

    /// Check if the command requires user acceptance
    pub fn requires_acceptance_simple(&self) -> bool {
        // Format a simple command string
        let cmd_str = if !self.command.starts_with('/') {
            format!("/{}", self.command)
        } else {
            self.command.clone()
        };

        // Try to parse the command
        if let Ok(command) = Command::parse(&cmd_str) {
            // Get the handler for this command using to_handler()
            let handler = command.to_handler();

            // Convert args to string slices for the handler
            let args: Vec<&str> = match &self.subcommand {
                Some(subcommand) => vec![subcommand.as_str()],
                None => vec![],
            };

            return handler.requires_confirmation(&args);
        }

        // For commands not recognized, default to requiring confirmation
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
        // Format a simple command string
        let cmd_str = if !self.command.starts_with('/') {
            format!("/{}", self.command)
        } else {
            self.command.clone()
        };

        // Try to parse the command
        if let Ok(command) = Command::parse(&cmd_str) {
            // Get the handler for this command using to_handler()
            let handler = command.to_handler();
            return handler.description().to_string();
        }

        // For commands not recognized, return a generic description
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
    /// It uses Command::parse_from_components to get the Command enum and then uses execute
    /// to execute the command.
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

        // Parse the command using Command::parse_from_components
        match Command::parse_from_components(
            &self.command,
            self.subcommand.as_ref(),
            self.args.as_ref(),
            self.flags.as_ref(),
        ) {
            Ok(command) => {
                // Return an InvokeOutput with the response and next state
                Ok(InvokeOutput {
                    output: OutputKind::Text(response),
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
                    output: OutputKind::Text(e.to_string()),
                    next_state: None,
                })
            },
        }
    }
}
