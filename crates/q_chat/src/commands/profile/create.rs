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

/// Handler for the profile create command
pub struct CreateProfileCommand {
    name: String,
}

impl CreateProfileCommand {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

impl CommandHandler for CreateProfileCommand {
    fn name(&self) -> &'static str {
        "create"
    }

    fn description(&self) -> &'static str {
        "Create a new profile"
    }

    fn usage(&self) -> &'static str {
        "/profile create <name>"
    }

    fn help(&self) -> String {
        "Create a new profile with the specified name.".to_string()
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
            if let Some(context_manager) = &ctx.conversation_state.context_manager {
                // Create the profile
                match context_manager.create_profile(&self.name).await {
                    Ok(_) => {
                        queue!(
                            ctx.output,
                            style::Print("\nProfile '"),
                            style::SetForegroundColor(Color::Green),
                            style::Print(&self.name),
                            style::ResetColor,
                            style::Print("' created successfully.\n\n")
                        )?;
                    },
                    Err(e) => {
                        queue!(
                            ctx.output,
                            style::SetForegroundColor(Color::Red),
                            style::Print(format!("\nError creating profile: {}\n\n", e)),
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
        false // Create command doesn't require confirmation
    }
}
