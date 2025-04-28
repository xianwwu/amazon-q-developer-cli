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
use crate::tools::Tool;
use crate::{
    ChatState,
    QueuedTool,
};

/// Handler for the tools reset single command
pub struct ResetSingleToolCommand {
    tool_name: String,
}

impl ResetSingleToolCommand {
    pub fn new(tool_name: String) -> Self {
        Self { tool_name }
    }
}

impl CommandHandler for ResetSingleToolCommand {
    fn name(&self) -> &'static str {
        "reset"
    }

    fn description(&self) -> &'static str {
        "Reset a specific tool to default permission level"
    }

    fn usage(&self) -> &'static str {
        "/tools reset <tool_name>"
    }

    fn help(&self) -> String {
        "Reset a specific tool to its default permission level.".to_string()
    }

    fn execute<'a>(
        &'a self,
        _args: Vec<&'a str>,
        ctx: &'a mut CommandContextAdapter<'a>,
        tool_uses: Option<Vec<QueuedTool>>,
        pending_tool_index: Option<usize>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatState>> + Send + 'a>> {
        Box::pin(async move {
            // Check if the tool exists
            if !Tool::all_tool_names().contains(&self.tool_name.as_str()) {
                queue!(
                    ctx.output,
                    style::SetForegroundColor(Color::Red),
                    style::Print(format!("\nUnknown tool: '{}'\n\n", self.tool_name)),
                    style::ResetColor
                )?;
            } else {
                // Reset the tool permission
                ctx.tool_permissions.reset_tool(&self.tool_name);

                queue!(
                    ctx.output,
                    style::SetForegroundColor(Color::Green),
                    style::Print(format!(
                        "\nReset tool '{}' to default permission level.\n\n",
                        self.tool_name
                    )),
                    style::ResetColor
                )?;
            }
            ctx.output.flush()?;

            Ok(ChatState::PromptUser {
                tool_uses,
                pending_tool_index,
                skip_printing_tools: false,
            })
        })
    }

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        false // Reset single command doesn't require confirmation
    }
}
