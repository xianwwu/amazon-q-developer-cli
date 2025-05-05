use std::future::Future;
use std::io::Write;
use std::pin::Pin;

use crossterm::queue;
use crossterm::style::{
    self,
    Color,
};
use eyre::Result;

use crate::command::{
    Command,
    ProfileSubcommand,
};
use crate::commands::context_adapter::CommandContextAdapter;
use crate::commands::handler::CommandHandler;
use crate::{
    ChatState,
    QueuedTool,
};

/// Static instance of the profile set command handler
pub static SET_PROFILE_HANDLER: SetProfileCommand = SetProfileCommand;

/// Handler for the profile set command
pub struct SetProfileCommand;

impl CommandHandler for SetProfileCommand {
    fn name(&self) -> &'static str {
        "set"
    }

    fn description(&self) -> &'static str {
        "Set the current profile"
    }

    fn usage(&self) -> &'static str {
        "/profile set <name>"
    }

    fn help(&self) -> String {
        "Switch to the specified profile.".to_string()
    }

    fn to_command(&self, args: Vec<&str>) -> Result<Command> {
        if args.len() != 1 {
            return Err(eyre::eyre!("Expected profile name argument"));
        }

        Ok(Command::Profile {
            subcommand: ProfileSubcommand::Set {
                name: args[0].to_string(),
            },
        })
    }

    fn execute<'a>(
        &'a self,
        args: Vec<&'a str>,
        ctx: &'a mut CommandContextAdapter<'a>,
        tool_uses: Option<Vec<QueuedTool>>,
        pending_tool_index: Option<usize>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatState>> + Send + 'a>> {
        Box::pin(async move {
            // Parse the command to get the profile name
            let command = self.to_command(args)?;

            // Extract the profile name from the command
            let name = match command {
                Command::Profile {
                    subcommand: ProfileSubcommand::Set { name },
                } => name,
                _ => return Err(eyre::eyre!("Invalid command")),
            };

            // Get the context manager
            if let Some(context_manager) = &mut ctx.conversation_state.context_manager {
                // Switch to the profile
                match context_manager.switch_profile(&name).await {
                    Ok(_) => {
                        queue!(
                            ctx.output,
                            style::Print("\nSwitched to profile '"),
                            style::SetForegroundColor(Color::Green),
                            style::Print(&name),
                            style::ResetColor,
                            style::Print("'.\n\n")
                        )?;
                    },
                    Err(e) => {
                        queue!(
                            ctx.output,
                            style::SetForegroundColor(Color::Red),
                            style::Print(format!("\nError switching profile: {}\n\n", e)),
                            style::ResetColor
                        )?;
                    },
                }
                ctx.output.flush()?;
            } else {
                queue!(
                    ctx.output,
                    style::SetForegroundColor(Color::Red),
                    style::Print("\nContext manager is not available.\n\n"),
                    style::ResetColor
                )?;
                ctx.output.flush()?;
            }

            Ok(ChatState::PromptUser {
                tool_uses,
                pending_tool_index,
                skip_printing_tools: false,
            })
        })
    }

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        true // Set command requires confirmation as it's a mutative operation
    }
}
