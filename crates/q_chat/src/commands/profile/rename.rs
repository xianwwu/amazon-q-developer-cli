use std::future::Future;
use std::io::Write;
use std::pin::Pin;

use crossterm::queue;
use crossterm::style::{
    self,
    Color,
};
use eyre::Result;

use crate::commands::context_adapter::CommandContextAdapter;
use crate::commands::handler::CommandHandler;
use crate::{
    ChatState,
    QueuedTool,
};

/// Handler for the profile rename command
pub struct RenameProfileCommand {
    old_name: String,
    new_name: String,
}

impl RenameProfileCommand {
    pub fn new(old_name: String, new_name: String) -> Self {
        Self { old_name, new_name }
    }
}

impl CommandHandler for RenameProfileCommand {
    fn name(&self) -> &'static str {
        "rename"
    }

    fn description(&self) -> &'static str {
        "Rename a profile"
    }

    fn usage(&self) -> &'static str {
        "/profile rename <old_name> <new_name>"
    }

    fn help(&self) -> String {
        "Rename a profile from <old_name> to <new_name>.".to_string()
    }

    fn execute<'a>(
        &'a self,
        _args: Vec<&'a str>,
        ctx: &'a mut CommandContextAdapter<'a>,
        tool_uses: Option<Vec<QueuedTool>>,
        pending_tool_index: Option<usize>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatState>> + Send + 'a>> {
        Box::pin(async move {
            // Get the context manager
            if let Some(context_manager) = &mut ctx.conversation_state.context_manager {
                // Rename the profile
                match context_manager.rename_profile(&self.old_name, &self.new_name).await {
                    Ok(_) => {
                        queue!(
                            ctx.output,
                            style::Print("\nProfile '"),
                            style::SetForegroundColor(Color::Green),
                            style::Print(&self.old_name),
                            style::ResetColor,
                            style::Print("' renamed to '"),
                            style::SetForegroundColor(Color::Green),
                            style::Print(&self.new_name),
                            style::ResetColor,
                            style::Print("'.\n\n")
                        )?;
                    },
                    Err(e) => {
                        queue!(
                            ctx.output,
                            style::SetForegroundColor(Color::Red),
                            style::Print(format!("\nError renaming profile: {}\n\n", e)),
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
        false // Rename command doesn't require confirmation
    }
}
