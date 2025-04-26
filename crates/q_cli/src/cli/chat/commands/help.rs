use std::future::Future;
use std::pin::Pin;

use eyre::Result;
use fig_os_shim::Context;

use crate::cli::chat::commands::CommandHandler;
use crate::cli::chat::{
    ChatState,
    QueuedTool,
};

/// Handler for the help command
pub struct HelpCommand;

impl HelpCommand {
    pub fn new() -> Self {
        Self
    }
}

/// Help text displayed when the user types /help
pub const HELP_TEXT: &str = r#"

q (Amazon Q Chat)

Commands:
/clear        Clear the conversation history
/issue        Report an issue or make a feature request
/editor       Open $EDITOR (defaults to vi) to compose a prompt
/help         Show this help dialogue
/quit         Quit the application
/compact      Summarize the conversation to free up context space
  help        Show help for the compact command
  [prompt]    Optional custom prompt to guide summarization
  --summary   Display the summary after compacting
/tools        View and manage tools and permissions
  help        Show an explanation for the trust command
  trust       Trust a specific tool for the session
  untrust     Revert a tool to per-request confirmation
  trustall    Trust all tools (equivalent to deprecated /acceptall)
  reset       Reset all tools to default permission levels
/profile      Manage profiles
  help        Show profile help
  list        List profiles
  set         Set the current profile
  create      Create a new profile
  delete      Delete a profile
  rename      Rename a profile
/context      Manage context files and hooks for the chat session
  help        Show context help
  show        Display current context rules configuration [--expand]
  add         Add file(s) to context [--global] [--force]
  rm          Remove file(s) from context [--global]
  clear       Clear all files from current context [--global]
  hooks       View and manage context hooks
/usage      Show current session's context window usage

Tips:
!{command}            Quickly execute a command in your current session
Ctrl(^) + j           Insert new-line to provide multi-line prompt. Alternatively, [Alt(⌥) + Enter(⏎)]
Ctrl(^) + k           Fuzzy search commands and context files. Use Tab to select multiple items.
                      Change the keybind to ctrl+x with: q settings chat.skimCommandKey x (where x is any key)

"#;

impl CommandHandler for HelpCommand {
    fn name(&self) -> &'static str {
        "help"
    }

    fn description(&self) -> &'static str {
        "Show help information"
    }

    fn usage(&self) -> &'static str {
        "/help"
    }

    fn help(&self) -> String {
        "Shows the help dialogue with available commands and their descriptions.".to_string()
    }

    fn execute<'a>(
        &'a self,
        _args: Vec<&'a str>,
        _ctx: &'a Context,
        tool_uses: Option<Vec<QueuedTool>>,
        pending_tool_index: Option<usize>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatState>> + Send + 'a>> {
        Box::pin(async move {
            // Return DisplayHelp state with the comprehensive help text
            Ok(ChatState::DisplayHelp {
                help_text: HELP_TEXT.to_string(),
                tool_uses,
                pending_tool_index,
            })
        })
    }

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        false // Help command doesn't require confirmation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_command() {
        let command = HelpCommand::new();
        assert_eq!(command.name(), "help");
        assert_eq!(command.description(), "Show help information");
        assert_eq!(command.usage(), "/help");
        assert!(!command.requires_confirmation(&[]));
    }
}
