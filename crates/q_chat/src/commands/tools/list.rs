use std::future::Future;
use std::io::Write;
use std::pin::Pin;

use crossterm::queue;
use crossterm::style::{
    self,
    Color,
};
use eyre::Result;

use crate::command::Command;
use crate::commands::context_adapter::CommandContextAdapter;
use crate::commands::handler::CommandHandler;
use crate::tools::Tool;
use crate::{
    ChatState,
    QueuedTool,
};

/// Static instance of the tools list command handler
pub static LIST_TOOLS_HANDLER: ListToolsCommand = ListToolsCommand;

/// Handler for the tools list command
pub struct ListToolsCommand;

impl CommandHandler for ListToolsCommand {
    fn name(&self) -> &'static str {
        "list"
    }

    fn description(&self) -> &'static str {
        "List all available tools and their status"
    }

    fn usage(&self) -> &'static str {
        "/tools list"
    }

    fn help(&self) -> String {
        "List all available tools and their current permission status.".to_string()
    }

    fn to_command(&self, _args: Vec<&str>) -> Result<Command> {
        Ok(Command::Tools { subcommand: None })
    }

    fn execute_command<'a>(
        &'a self,
        command: &'a Command,
        ctx: &'a mut CommandContextAdapter<'a>,
        tool_uses: Option<Vec<QueuedTool>>,
        pending_tool_index: Option<usize>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatState>> + Send + 'a>> {
        Box::pin(async move {
            if let Command::Tools { subcommand: None } = command {
                // List all tools and their status
                queue!(
                    ctx.output,
                    style::Print("\nTrusted tools can be run without confirmation\n\n")
                )?;

                // Get all tool names
                let tool_names = Tool::all_tool_names();

                // Display each tool with its permission status
                for tool_name in tool_names {
                    let permission_label = ctx.tool_permissions.display_label(tool_name);

                    queue!(
                        ctx.output,
                        style::Print("- "),
                        style::Print(format!("{:<20} ", tool_name)),
                        style::Print(permission_label),
                        style::Print("\n")
                    )?;
                }

                // Add a note about default settings
                queue!(
                    ctx.output,
                    style::SetForegroundColor(Color::DarkGrey),
                    style::Print("\n* Default settings\n\n"),
                    style::Print("💡 Use "),
                    style::SetForegroundColor(Color::Green),
                    style::Print("/tools help"),
                    style::SetForegroundColor(Color::DarkGrey),
                    style::Print(" to edit permissions.\n"),
                    style::ResetColor,
                    style::Print("\n")
                )?;
                ctx.output.flush()?;

                Ok(ChatState::PromptUser {
                    tool_uses,
                    pending_tool_index,
                    skip_printing_tools: false,
                })
            } else {
                Err(eyre::anyhow!(
                    "ListToolsCommand can only execute Tools commands with no subcommand"
                ))
            }
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
    use crate::tools::ToolPermissions;
    use crate::util::shared_writer::SharedWriter;

    #[tokio::test]
    async fn test_tools_list_command() {
        let handler = ListToolsCommand;

        // Create a minimal context
        let context = Arc::new(Context::new_fake());
        let output = SharedWriter::null();
        let mut conversation_state = ConversationState::new(
            Arc::clone(&context),
            "test-conversation",
            HashMap::new(),
            None,
            Some(SharedWriter::null()),
        )
        .await;
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

        // Execute the list subcommand
        let args = vec![];
        let result = handler.execute(args, &mut ctx, None, None).await;

        assert!(result.is_ok());
    }
}
