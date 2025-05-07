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

/// Static instance of the remove context command handler
pub static REMOVE_CONTEXT_HANDLER: RemoveContextCommand = RemoveContextCommand;

/// Handler for the context remove command
pub struct RemoveContextCommand;

impl CommandHandler for RemoveContextCommand {
    fn name(&self) -> &'static str {
        "remove"
    }

    fn description(&self) -> &'static str {
        "Remove file(s) from context"
    }

    fn usage(&self) -> &'static str {
        "/context rm [--global] <path1> [path2...]"
    }

    fn help(&self) -> String {
        "Remove files from the context. Use --global to remove from global context.".to_string()
    }

    fn to_command(&self, args: Vec<&str>) -> Result<crate::command::Command, ChatError> {
        let mut global = false;
        let mut paths = Vec::new();

        for arg in args {
            match arg {
                "--global" => global = true,
                _ => paths.push(arg.to_string()),
            }
        }

        Ok(crate::command::Command::Context {
            subcommand: crate::command::ContextSubcommand::Remove { global, paths },
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
            let (global, paths) = match command {
                crate::command::Command::Context {
                    subcommand: crate::command::ContextSubcommand::Remove { global, paths },
                } => (global, paths),
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

            // Remove the paths from the context
            match context_manager.remove_paths(paths, global).await {
                Ok(_) => {
                    // Success message
                    let scope = if global { "global" } else { "profile" };
                    queue!(
                        ctx.output,
                        style::SetForegroundColor(Color::Green),
                        style::Print(format!("Removed path(s) from {} context\n", scope)),
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

            Ok(ChatState::PromptUser {
                tool_uses,
                pending_tool_index,
                skip_printing_tools: true,
            })
        })
    }

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        true // Removing context files requires confirmation as it's a destructive operation
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
    fn test_to_command_with_global() {
        let handler = RemoveContextCommand;
        let args = vec!["--global", "path1", "path2"];

        let command = handler.to_command(args).unwrap();

        match command {
            Command::Context {
                subcommand: ContextSubcommand::Remove { global, paths },
            } => {
                assert!(global);
                assert_eq!(paths, vec!["path1".to_string(), "path2".to_string()]);
            },
            _ => panic!("Expected Context Remove command"),
        }
    }

    #[test]
    fn test_to_command_without_global() {
        let handler = RemoveContextCommand;
        let args = vec!["path1", "path2"];

        let command = handler.to_command(args).unwrap();

        match command {
            Command::Context {
                subcommand: ContextSubcommand::Remove { global, paths },
            } => {
                assert!(!global);
                assert_eq!(paths, vec!["path1".to_string(), "path2".to_string()]);
            },
            _ => panic!("Expected Context Remove command"),
        }
    }

    #[test]
    fn test_to_command_no_paths() {
        let handler = RemoveContextCommand;
        let args = vec!["--global"];

        let command = handler.to_command(args).unwrap();

        match command {
            Command::Context {
                subcommand: ContextSubcommand::Remove { global, paths },
            } => {
                assert!(global);
                assert!(paths.is_empty());
            },
            _ => panic!("Expected Context Remove command"),
        }
    }

    #[test]
    fn test_requires_confirmation() {
        let handler = RemoveContextCommand;
        assert!(handler.requires_confirmation(&[]));
    }
}
