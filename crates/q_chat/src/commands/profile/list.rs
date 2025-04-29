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

/// Handler for the profile list command
pub struct ListProfileCommand;

impl ListProfileCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ListProfileCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandHandler for ListProfileCommand {
    fn name(&self) -> &'static str {
        "list"
    }

    fn description(&self) -> &'static str {
        "List available profiles"
    }

    fn usage(&self) -> &'static str {
        "/profile list"
    }

    fn help(&self) -> String {
        "List all available profiles and show which one is currently active.".to_string()
    }

    fn to_command(&self, _args: Vec<&str>) -> Result<Command> {
        Ok(Command::Profile {
            subcommand: ProfileSubcommand::List,
        })
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
                // Get the list of profiles
                let profiles = context_manager.list_profiles().await?;
                let current_profile = &context_manager.current_profile;

                // Display the profiles
                queue!(ctx.output, style::Print("\nAvailable profiles:\n"))?;

                for profile in profiles {
                    if &profile == current_profile {
                        queue!(
                            ctx.output,
                            style::Print("* "),
                            style::SetForegroundColor(Color::Green),
                            style::Print(profile),
                            style::ResetColor,
                            style::Print("\n")
                        )?;
                    } else {
                        queue!(
                            ctx.output,
                            style::Print("  "),
                            style::Print(profile),
                            style::Print("\n")
                        )?;
                    }
                }

                queue!(ctx.output, style::Print("\n"))?;
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
        false // List command doesn't require confirmation
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use fig_os_shim::Context;

    use super::*;
    use crate::Settings;
    use crate::conversation_state::ConversationState;
    use crate::input_source::InputSource;
    use crate::shared_writer::SharedWriter;
    use crate::tools::ToolPermissions;

    #[tokio::test]
    async fn test_list_profile_command() {
        let handler = ListProfileCommand::new();

        // Create a minimal context
        let context = Arc::new(Context::new_fake());
        let output = SharedWriter::null();
        let mut conversation_state =
            ConversationState::new(Arc::clone(&context), HashMap::new(), None, Some(SharedWriter::null())).await;
        let mut tool_permissions = ToolPermissions::new(0);
        let mut input_source = InputSource::new_mock(vec![]);
        let settings = Settings::new_fake();

        let mut ctx = CommandContextAdapter {
            context: &context,
            output: &mut output.clone(),
            conversation_state: &mut conversation_state,
            tool_permissions: &mut tool_permissions,
            interactive: true,
            input_source: &mut input_source,
            settings: &settings,
        };

        // Execute the list command
        let result = handler.execute(vec![], &mut ctx, None, None).await;

        assert!(result.is_ok());
    }
}
