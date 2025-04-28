use std::future::Future;
use std::io::Write;
use std::pin::Pin;

use crossterm::queue;
use crossterm::style::{
    self,
};
use eyre::Result;

use crate::commands::context_adapter::CommandContextAdapter;
use crate::commands::handler::CommandHandler;
use crate::{
    ChatState,
    QueuedTool,
};

/// Handler for the tools help command
pub struct HelpToolsCommand;

impl HelpToolsCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HelpToolsCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandHandler for HelpToolsCommand {
    fn name(&self) -> &'static str {
        "help"
    }

    fn description(&self) -> &'static str {
        "Show tools help"
    }

    fn usage(&self) -> &'static str {
        "/tools help"
    }

    fn help(&self) -> String {
        "Show help for the tools command.".to_string()
    }

    fn execute<'a>(
        &'a self,
        _args: Vec<&'a str>,
        ctx: &'a mut CommandContextAdapter<'a>,
        tool_uses: Option<Vec<QueuedTool>>,
        pending_tool_index: Option<usize>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatState>> + Send + 'a>> {
        Box::pin(async move {
            // Display help text
            let help_text = color_print::cformat!(
                r#"
<magenta,em>Tools Management</magenta,em>

Tools allow Amazon Q to perform actions on your system, such as executing commands or modifying files.
You can view and manage tool permissions using the following commands:

<cyan!>Available commands</cyan!>
  <em>list</em>                <black!>List all available tools and their status</black!>
  <em>trust <<tool>></em>      <black!>Trust a specific tool for the session</black!>
  <em>untrust <<tool>></em>    <black!>Revert a tool to per-request confirmation</black!>
  <em>trustall</em>            <black!>Trust all tools for the session</black!>
  <em>reset</em>               <black!>Reset all tools to default permission levels</black!>

<cyan!>Notes</cyan!>
• You will be prompted for permission before any tool is used
• You can trust tools for the duration of a session
• Trusted tools will not require confirmation each time they're used
"#
            );

            queue!(
                ctx.output,
                style::Print("\n"),
                style::Print(help_text),
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
        false // Help command doesn't require confirmation
    }
}
