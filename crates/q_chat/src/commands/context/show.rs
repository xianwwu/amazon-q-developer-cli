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

/// Static instance of the show context command handler
pub static SHOW_CONTEXT_HANDLER: ShowContextCommand = ShowContextCommand;

/// Handler for the context show command
pub struct ShowContextCommand;

impl CommandHandler for ShowContextCommand {
    fn name(&self) -> &'static str {
        "show"
    }

    fn description(&self) -> &'static str {
        "Display current context configuration"
    }

    fn usage(&self) -> &'static str {
        "/context show [--expand]"
    }

    fn help(&self) -> String {
        "Display the current context configuration. Use --expand to show expanded file contents.".to_string()
    }

    fn to_command(&self, args: Vec<&str>) -> Result<crate::command::Command, ChatError> {
        let expand = args.contains(&"--expand");

        Ok(crate::command::Command::Context {
            subcommand: crate::command::ContextSubcommand::Show { expand },
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
            // Parse the command to get the expand parameter
            let command = self.to_command(args)?;

            // Extract the expand parameter from the command
            let expand = match command {
                crate::command::Command::Context {
                    subcommand: crate::command::ContextSubcommand::Show { expand },
                } => expand,
                _ => return Err(ChatError::Custom("Invalid command".into())),
            };

            // Get the context manager
            let Some(context_manager) = &ctx.conversation_state.context_manager else {
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

            // Display current profile
            queue!(
                ctx.output,
                style::SetForegroundColor(Color::Blue),
                style::Print(format!("Current profile: {}\n", context_manager.current_profile)),
                style::ResetColor
            )?;

            // Show global context paths
            queue!(
                ctx.output,
                style::SetForegroundColor(Color::Yellow),
                style::Print("\nGlobal context paths:\n"),
                style::ResetColor
            )?;

            if context_manager.global_config.paths.is_empty() {
                queue!(ctx.output, style::Print("  (none)\n"))?;
            } else {
                for path in &context_manager.global_config.paths {
                    queue!(ctx.output, style::Print(format!("  {}\n", path)))?;
                }

                // If expand is requested, show the expanded files
                if expand {
                    let expanded_files = context_manager.get_global_context_files(true).await?;
                    queue!(
                        ctx.output,
                        style::SetForegroundColor(Color::Yellow),
                        style::Print("\nExpanded global context files:\n"),
                        style::ResetColor
                    )?;

                    if expanded_files.is_empty() {
                        queue!(ctx.output, style::Print("  (none)\n"))?;
                    } else {
                        for (path, _) in expanded_files {
                            queue!(ctx.output, style::Print(format!("  {}\n", path)))?;
                        }
                    }
                }
            }

            // Display profile-specific context paths
            queue!(
                ctx.output,
                style::SetForegroundColor(Color::Yellow),
                style::Print(format!(
                    "\nProfile '{}' context paths:\n",
                    context_manager.current_profile
                )),
                style::ResetColor
            )?;

            if context_manager.profile_config.paths.is_empty() {
                queue!(ctx.output, style::Print("  (none)\n"))?;
            } else {
                for path in &context_manager.profile_config.paths {
                    queue!(ctx.output, style::Print(format!("  {}\n", path)))?;
                }

                // If expand is requested, show the expanded files
                if expand {
                    let expanded_files = context_manager.get_current_profile_context_files(true).await?;
                    queue!(
                        ctx.output,
                        style::SetForegroundColor(Color::Yellow),
                        style::Print(format!(
                            "\nExpanded profile '{}' context files:\n",
                            context_manager.current_profile
                        )),
                        style::ResetColor
                    )?;

                    if expanded_files.is_empty() {
                        queue!(ctx.output, style::Print("  (none)\n"))?;
                    } else {
                        for (path, _) in expanded_files {
                            queue!(ctx.output, style::Print(format!("  {}\n", path)))?;
                        }
                    }
                }
            }

            ctx.output.flush()?;

            Ok(ChatState::PromptUser {
                tool_uses,
                pending_tool_index,
                skip_printing_tools: true,
            })
        })
    }

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        false // Showing context doesn't require confirmation
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
    fn test_to_command_with_expand() {
        let handler = ShowContextCommand;
        let args = vec!["--expand"];

        let command = handler.to_command(args).unwrap();

        match command {
            Command::Context {
                subcommand: ContextSubcommand::Show { expand },
            } => {
                assert!(expand);
            },
            _ => panic!("Expected Context Show command"),
        }
    }

    #[test]
    fn test_to_command_without_expand() {
        let handler = ShowContextCommand;
        let args = vec![];

        let command = handler.to_command(args).unwrap();

        match command {
            Command::Context {
                subcommand: ContextSubcommand::Show { expand },
            } => {
                assert!(!expand);
            },
            _ => panic!("Expected Context Show command"),
        }
    }

    #[test]
    fn test_requires_confirmation() {
        let handler = ShowContextCommand;
        assert!(!handler.requires_confirmation(&[]));
    }
}
