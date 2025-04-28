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

/// Handler for the profile help command
pub struct HelpProfileCommand;

impl HelpProfileCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HelpProfileCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandHandler for HelpProfileCommand {
    fn name(&self) -> &'static str {
        "help"
    }

    fn description(&self) -> &'static str {
        "Show profile help"
    }

    fn usage(&self) -> &'static str {
        "/profile help"
    }

    fn help(&self) -> String {
        "Show help for the profile command.".to_string()
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
<magenta,em>(Beta) Profile Management</magenta,em>

Profiles allow you to organize and manage different sets of context files for different projects or tasks.

<cyan!>Available commands</cyan!>
  <em>help</em>                <black!>Show an explanation for the profile command</black!>
  <em>list</em>                <black!>List all available profiles</black!>
  <em>create <<n>></em>       <black!>Create a new profile with the specified name</black!>
  <em>delete <<n>></em>       <black!>Delete the specified profile</black!>
  <em>set <<n>></em>          <black!>Switch to the specified profile</black!>
  <em>rename <<old>> <<new>></em>  <black!>Rename a profile</black!>

<cyan!>Notes</cyan!>
• The "global" profile contains context files that are available in all profiles
• The "default" profile is used when no profile is specified
• You can switch between profiles to work on different projects
• Each profile maintains its own set of context files
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
