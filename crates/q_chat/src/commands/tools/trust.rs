use std::collections::HashSet;
use std::future::Future;
use std::io::Write;
use std::pin::Pin;

use crossterm::queue;
use crossterm::style::{
    self,
    Attribute,
    Color,
};
use eyre::Result;

use crate::command::{
    Command,
    ToolsSubcommand,
};
use crate::commands::context_adapter::CommandContextAdapter;
use crate::commands::handler::CommandHandler;
use crate::tools::Tool;
use crate::{
    ChatState,
    QueuedTool,
};

/// Handler for the tools trust command
pub struct TrustToolsCommand {
    tool_names: Vec<String>,
}

impl TrustToolsCommand {
    pub fn new(tool_names: Vec<String>) -> Self {
        Self { tool_names }
    }
}

impl CommandHandler for TrustToolsCommand {
    fn name(&self) -> &'static str {
        "trust"
    }

    fn description(&self) -> &'static str {
        "Trust a specific tool for the session"
    }

    fn usage(&self) -> &'static str {
        "/tools trust <tool_name> [tool_name...]"
    }

    fn help(&self) -> String {
        "Trust specific tools for the session. Trusted tools will not require confirmation before running.".to_string()
    }

    fn to_command(&self, _args: Vec<&str>) -> Result<Command> {
        let tool_names: HashSet<String> = self.tool_names.iter().cloned().collect();
        Ok(Command::Tools {
            subcommand: Some(ToolsSubcommand::Trust { tool_names }),
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
            // Trust the specified tools
            for tool_name in &self.tool_names {
                // Check if the tool exists
                if !Tool::all_tool_names().contains(&tool_name.as_str()) {
                    queue!(
                        ctx.output,
                        style::SetForegroundColor(Color::Red),
                        style::Print(format!("\nUnknown tool: '{}'\n", tool_name)),
                        style::ResetColor
                    )?;
                    continue;
                }

                // Trust the tool
                ctx.tool_permissions.trust_tool(tool_name);

                queue!(
                    ctx.output,
                    style::SetForegroundColor(Color::Green),
                    style::Print(format!("\nTool '{}' is now trusted. I will ", tool_name)),
                    style::SetAttribute(Attribute::Bold),
                    style::Print("not"),
                    style::SetAttribute(Attribute::NoBold),
                    style::Print(" ask for confirmation before running this tool.\n"),
                    style::ResetColor
                )?;
            }

            queue!(ctx.output, style::Print("\n"))?;
            ctx.output.flush()?;

            Ok(ChatState::PromptUser {
                tool_uses,
                pending_tool_index,
                skip_printing_tools: false,
            })
        })
    }

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        false // Trust command doesn't require confirmation
    }
}
