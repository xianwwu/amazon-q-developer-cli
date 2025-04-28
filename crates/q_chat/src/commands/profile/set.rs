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

/// Handler for the profile set command
pub struct SetProfileCommand {
    name: String,
}

impl SetProfileCommand {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

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
                // Switch to the profile
                match context_manager.switch_profile(&self.name).await {
                    Ok(_) => {
                        queue!(
                            ctx.output,
                            style::Print("\nSwitched to profile '"),
                            style::SetForegroundColor(Color::Green),
                            style::Print(&self.name),
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
        false // Set command doesn't require confirmation
    }
}
