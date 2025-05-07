use super::handler::CommandHandler;
use crate::cli::chat::ChatError;
use crate::cli::chat::command::Command;

/// Command handler for the `/issue` command
pub struct IssueCommand;

impl IssueCommand {
    /// Create a new instance of the IssueCommand
    pub fn new() -> Self {
        Self
    }
}

impl Default for IssueCommand {
    fn default() -> Self {
        Self::new()
    }
}

/// Static instance of the issue command handler
pub static ISSUE_HANDLER: IssueCommand = IssueCommand;

impl CommandHandler for IssueCommand {
    fn name(&self) -> &'static str {
        "issue"
    }

    fn description(&self) -> &'static str {
        "Report an issue with Amazon Q"
    }

    fn usage(&self) -> &'static str {
        "/issue [title]"
    }

    fn help(&self) -> String {
        "Report an issue with Amazon Q. This will open a GitHub issue template with details about your session."
            .to_string()
    }

    fn llm_description(&self) -> String {
        r#"The issue command opens a pre-filled GitHub issue template to report problems with Amazon Q.

Usage:
/issue [title]

Examples:
- "/issue" - Opens a blank issue template
- "/issue Amazon Q is not responding correctly" - Opens an issue template with the specified title

This command helps users report bugs, request features, or provide feedback about Amazon Q."#
            .to_string()
    }

    fn to_command(&self, args: Vec<&str>) -> Result<Command, ChatError> {
        let prompt = if args.is_empty() { None } else { Some(args.join(" ")) };

        Ok(Command::Issue { prompt })
    }

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        true // Issue command requires confirmation as it's a mutative operation
    }
}
