use std::io::Write;

use crossterm::queue;
use crossterm::style::{
    self,
    Color,
};

use crate::commands::CommandHandler;
use crate::{
    ChatError,
    ChatState,
    QueuedTool,
};

/// Static instance of the add context command handler
pub static ADD_CONTEXT_HANDLER: AddContextCommand = AddContextCommand;

/// Handler for the context add command
pub struct AddContextCommand;

impl CommandHandler for AddContextCommand {
    fn name(&self) -> &'static str {
        "add"
    }

    fn description(&self) -> &'static str {
        "Add file(s) to context"
    }

    fn usage(&self) -> &'static str {
        "/context add [--global] [--force] <path1> [path2...]"
    }

    fn help(&self) -> String {
        "Add files to the context. Use --global to add to global context (available in all profiles). Use --force to add files even if they exceed size limits.".to_string()
    }

    fn to_command(&self, args: Vec<&str>) -> Result<crate::command::Command, ChatError> {
        let mut global = false;
        let mut force = false;
        let mut paths = Vec::new();

        for arg in args {
            match arg {
                "--global" => global = true,
                "--force" => force = true,
                _ => paths.push(arg.to_string()),
            }
        }

        Ok(crate::command::Command::Context {
            subcommand: crate::command::ContextSubcommand::Add { global, force, paths },
        })
    }

    fn execute<'a>(
        &'a self,
        args: Vec<&'a str>,
        ctx: &'a mut crate::commands::context_adapter::CommandContextAdapter<'a>,
        tool_uses: Option<Vec<QueuedTool>>,
        pending_tool_index: Option<usize>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ChatState, ChatError>> + Send + 'a>> {
        Box::pin(async move {
            // Parse the command to get the parameters
            let command = self.to_command(args)?;

            // Extract the parameters from the command
            let (global, force, paths) = match command {
                crate::command::Command::Context {
                    subcommand: crate::command::ContextSubcommand::Add { global, force, paths },
                } => (global, force, paths),
                _ => return Err(ChatError::Custom("Invalid command".into())),
            };

            // Check if paths are provided
            if paths.is_empty() {
                return Err(ChatError::Custom(
                    format!("No paths specified. Usage: {}", self.usage()).into(),
                ));
            }

            // Get the context manager
            let Some(context_manager) = &mut ctx.conversation_state.context_manager else {
                queue!(
                    ctx.output,
                    style::SetForegroundColor(Color::Red),
                    style::Print("Error: Context manager not initialized\n"),
                    style::ResetColor
                )?;
                ctx.output.flush()?;
                return Ok(ChatState::PromptUser {
                    tool_uses,
                    pending_tool_index,
                    skip_printing_tools: true,
                });
            };

            // Add the paths to the context
            match context_manager.add_paths(paths.clone(), global, force).await {
                Ok(_) => {
                    // Success message
                    let scope = if global { "global" } else { "profile" };
                    queue!(
                        ctx.output,
                        style::SetForegroundColor(Color::Green),
                        style::Print(format!("Added {} file(s) to {} context\n", paths.len(), scope)),
                        style::ResetColor
                    )?;
                    ctx.output.flush()?;
                },
                Err(e) => {
                    // Error message
                    queue!(
                        ctx.output,
                        style::SetForegroundColor(Color::Red),
                        style::Print(format!("Error: {}\n", e)),
                        style::ResetColor
                    )?;
                    ctx.output.flush()?;
                },
            }

            // Return to prompt
            Ok(ChatState::PromptUser {
                tool_uses,
                pending_tool_index,
                skip_printing_tools: true,
            })
        })
    }

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        true // Adding context files requires confirmation as it's a mutative operation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{
        Command,
        ContextSubcommand,
    };

    #[test]
    fn test_to_command_with_global_and_force() {
        let handler = AddContextCommand;
        let args = vec!["--global", "--force", "path1", "path2"];

        let command = handler.to_command(args).unwrap();

        match command {
            Command::Context {
                subcommand: ContextSubcommand::Add { global, force, paths },
            } => {
                assert!(global);
                assert!(force);
                assert_eq!(paths, vec!["path1".to_string(), "path2".to_string()]);
            },
            _ => panic!("Expected Context Add command"),
        }
    }

    #[test]
    fn test_to_command_with_global_only() {
        let handler = AddContextCommand;
        let args = vec!["--global", "path1", "path2"];

        let command = handler.to_command(args).unwrap();

        match command {
            Command::Context {
                subcommand: ContextSubcommand::Add { global, force, paths },
            } => {
                assert!(global);
                assert!(!force);
                assert_eq!(paths, vec!["path1".to_string(), "path2".to_string()]);
            },
            _ => panic!("Expected Context Add command"),
        }
    }

    #[test]
    fn test_to_command_with_force_only() {
        let handler = AddContextCommand;
        let args = vec!["--force", "path1", "path2"];

        let command = handler.to_command(args).unwrap();

        match command {
            Command::Context {
                subcommand: ContextSubcommand::Add { global, force, paths },
            } => {
                assert!(!global);
                assert!(force);
                assert_eq!(paths, vec!["path1".to_string(), "path2".to_string()]);
            },
            _ => panic!("Expected Context Add command"),
        }
    }

    #[test]
    fn test_to_command_no_flags() {
        let handler = AddContextCommand;
        let args = vec!["path1", "path2"];

        let command = handler.to_command(args).unwrap();

        match command {
            Command::Context {
                subcommand: ContextSubcommand::Add { global, force, paths },
            } => {
                assert!(!global);
                assert!(!force);
                assert_eq!(paths, vec!["path1".to_string(), "path2".to_string()]);
            },
            _ => panic!("Expected Context Add command"),
        }
    }

    #[test]
    fn test_to_command_no_paths() {
        let handler = AddContextCommand;
        let args = vec!["--global", "--force"];

        let command = handler.to_command(args).unwrap();

        match command {
            Command::Context {
                subcommand: ContextSubcommand::Add { global, force, paths },
            } => {
                assert!(global);
                assert!(force);
                assert!(paths.is_empty());
            },
            _ => panic!("Expected Context Add command"),
        }
    }

    #[test]
    fn test_requires_confirmation() {
        let handler = AddContextCommand;
        assert!(handler.requires_confirmation(&[]));
    }
}
