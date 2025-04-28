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

/// Handler for the tools reset command
pub struct ResetToolsCommand;

impl ResetToolsCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ResetToolsCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandHandler for ResetToolsCommand {
    fn name(&self) -> &'static str {
        "reset"
    }

    fn description(&self) -> &'static str {
        "Reset all tools to default permission levels"
    }

    fn usage(&self) -> &'static str {
        "/tools reset"
    }

    fn help(&self) -> String {
        "Reset all tools to their default permission levels.".to_string()
    }

    fn execute<'a>(
        &'a self,
        _args: Vec<&'a str>,
        ctx: &'a mut CommandContextAdapter<'a>,
        tool_uses: Option<Vec<QueuedTool>>,
        pending_tool_index: Option<usize>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatState>> + Send + 'a>> {
        Box::pin(async move {
            // Reset all tool permissions
            ctx.tool_permissions.reset();

            queue!(
                ctx.output,
                style::SetForegroundColor(Color::Green),
                style::Print("\nReset all tools to the default permission levels.\n"),
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
        false // Reset command doesn't require confirmation
    }
}
