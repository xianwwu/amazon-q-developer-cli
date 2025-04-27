use std::future::Future;
use std::pin::Pin;

use eyre::Result;

use super::{
    CommandContextAdapter,
    CommandHandler,
};
use crate::{
    ChatState,
    QueuedTool,
};

/// Compact command handler
pub struct CompactCommand;

impl CompactCommand {
    /// Create a new compact command handler
    pub fn new() -> Self {
        Self
    }
}

impl Default for CompactCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandHandler for CompactCommand {
    fn name(&self) -> &'static str {
        "compact"
    }

    fn description(&self) -> &'static str {
        "Summarize the conversation to free up context space"
    }

    fn usage(&self) -> &'static str {
        "/compact [prompt] [--summary]"
    }

    fn help(&self) -> String {
        "Summarize the conversation history to free up context space while preserving essential information.\n\
        This is useful for long-running conversations that may eventually reach memory constraints.\n\n\
        Usage:\n\
        /compact                   Summarize the conversation and clear history\n\
        /compact [prompt]          Provide custom guidance for summarization\n\
        /compact --summary         Show the summary after compacting"
            .to_string()
    }

    fn execute<'a>(
        &'a self,
        args: Vec<&'a str>,
        _ctx: &'a mut CommandContextAdapter<'a>,
        tool_uses: Option<Vec<QueuedTool>>,
        pending_tool_index: Option<usize>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatState>> + Send + 'a>> {
        Box::pin(async move {
            // Parse arguments to determine if this is a help request, has a custom prompt, or shows summary
            let mut prompt = None;
            let mut show_summary = false;
            let mut help = false;

            for arg in args {
                match arg {
                    "--summary" => show_summary = true,
                    "help" => help = true,
                    _ => prompt = Some(arg.to_string()),
                }
            }

            Ok(ChatState::CompactHistory {
                tool_uses,
                pending_tool_index,
                prompt,
                show_summary,
                help,
            })
        })
    }

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        false // Compact command doesn't require confirmation
    }
}
