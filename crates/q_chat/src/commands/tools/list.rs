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

/// Handler for the tools list command
pub struct ListToolsCommand;

impl ListToolsCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ListToolsCommand {
    fn default() -> Self {
        Self::new()
    }
}

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
        "List all available tools and their trust status.".to_string()
    }

    fn to_command(&self, _args: Vec<&str>) -> Result<Command> {
        Ok(Command::Tools { subcommand: None })
    }

    fn execute<'a>(
        &'a self,
        _args: Vec<&'a str>,
        ctx: &'a mut CommandContextAdapter<'a>,
        tool_uses: Option<Vec<QueuedTool>>,
        pending_tool_index: Option<usize>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatState>> + Send + 'a>> {
        Box::pin(async move {
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
        })
    }

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        false // List command doesn't require confirmation
    }
}
