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
use crate::{
    ChatState,
    QueuedTool,
};

/// Static instance of the tools trustall command handler
pub static TRUSTALL_TOOLS_HANDLER: TrustAllToolsCommand = TrustAllToolsCommand;

/// Handler for the tools trustall command
pub struct TrustAllToolsCommand;

impl CommandHandler for TrustAllToolsCommand {
    fn name(&self) -> &'static str {
        "trustall"
    }

    fn description(&self) -> &'static str {
        "Trust all tools for the session"
    }

    fn usage(&self) -> &'static str {
        "/tools trustall"
    }

    fn to_command(&self, _args: Vec<&str>) -> Result<Command> {
        Ok(Command::Tools {
            subcommand: Some(ToolsSubcommand::TrustAll),
        })
    }

    fn help(&self) -> String {
        "Trust all tools for the session. This will allow all tools to run without confirmation.".to_string()
    }

    fn execute<'a>(
        &'a self,
        _args: Vec<&'a str>,
        ctx: &'a mut CommandContextAdapter<'a>,
        tool_uses: Option<Vec<QueuedTool>>,
        pending_tool_index: Option<usize>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatState>> + Send + 'a>> {
        Box::pin(async move {
            // Trust all tools
            ctx.tool_permissions.trust_all_tools();

            queue!(
                ctx.output,
                style::SetForegroundColor(Color::Green),
                style::Print("\nAll tools are now trusted ("),
                style::SetForegroundColor(Color::Red),
                style::Print("!"),
                style::SetForegroundColor(Color::Green),
                style::Print("). Amazon Q will execute tools "),
                style::SetAttribute(Attribute::Bold),
                style::Print("without"),
                style::SetAttribute(Attribute::NoBold),
                style::Print(" asking for confirmation.\n"),
                style::Print("Agents can sometimes do unexpected things so understand the risks.\n"),
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
        true // Trustall command requires confirmation
    }
}
