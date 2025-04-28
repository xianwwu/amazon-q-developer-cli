use std::future::Future;
use std::pin::Pin;

use eyre::Result;

use super::context_adapter::CommandContextAdapter;
use super::handler::CommandHandler;
use crate::ChatState;
use crate::QueuedTool;
use crate::tools::gh_issue::GhIssue;
use crate::tools::gh_issue::GhIssueContext;
use crate::tools::Tool;

/// Command handler for the `/issue` command
pub struct IssueCommand;

impl IssueCommand {
    /// Create a new instance of the IssueCommand
    pub fn new() -> Self {
        Self
    }
}

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
        color_print::cformat!(
            r#"
<magenta,em>Report an Issue</magenta,em>

Opens a pre-filled GitHub issue template to report problems with Amazon Q.

<cyan!>Usage: /issue [title]</cyan!>

<cyan!>Description</cyan!>
  Creates a GitHub issue with the conversation transcript, context files,
  and other relevant information to help diagnose and fix problems.

<cyan!>Examples</cyan!>
  <em>/issue</em>                      Opens a blank issue template
  <em>/issue Chat not responding</em>  Creates an issue with the specified title
"#
        )
    }

    fn llm_description(&self) -> String {
        r#"
The issue command opens the browser to a pre-filled GitHub issue template to report chat issues, bugs, or feature requests. 
Pre-filled information includes the conversation transcript, chat context, and chat request IDs from the service.

Usage:
- /issue [title]

Examples:
- "/issue" - Opens a blank issue template
- "/issue Chat not responding" - Creates an issue with the specified title

This command is useful when:
- The user encounters a bug or error
- The user wants to request a new feature
- The user wants to report unexpected behavior
- The user needs to share conversation context with the development team

The command automatically includes:
- Recent conversation history
- Current context files
- System information
- Request IDs for failed requests
"#.to_string()
    }

    fn execute<'a>(
        &'a self,
        args: Vec<&'a str>,
        ctx: &'a mut CommandContextAdapter<'a>,
        _tool_uses: Option<Vec<QueuedTool>>,
        _pending_tool_index: Option<usize>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatState>> + Send + 'a>> {
        Box::pin(async move {
            // Create a title from the arguments or use a default
            let title = if args.is_empty() {
                "Issue with Amazon Q".to_string()
            } else {
                args.join(" ")
            };

            // Create the GhIssue tool
            let mut gh_issue = GhIssue {
                title,
                expected_behavior: None,
                actual_behavior: None,
                steps_to_reproduce: None,
                context: None,
            };

            // Set up the context for the issue
            let issue_context = GhIssueContext {
                context_manager: ctx.conversation_state.context_manager().cloned(),
                transcript: ctx.conversation_state.transcript().clone(),
                failed_request_ids: ctx.conversation_state.failed_request_ids().clone(),
                tool_permissions: ctx.tool_permissions.get_all_permissions(),
                interactive: ctx.interactive,
            };

            gh_issue.set_context(issue_context);

            // Create a tool from the GhIssue
            let tool = Tool::GhIssue(gh_issue);

            // Queue the description
            tool.queue_description(ctx.context, ctx.output).await?;

            // Invoke the tool
            tool.invoke(ctx.context, ctx.output).await?;

            Ok(ChatState::Continue)
        })
    }

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        // Issue command doesn't require confirmation
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::context_adapter::CommandContextAdapter;
    use crate::context::Context;
    use crate::conversation_state::ConversationState;
    use crate::shared_writer::SharedWriter;
    use crate::tools::ToolPermissions;
    use crate::input_source::InputSource;
    use crate::Settings;
    use std::io::Cursor;

    #[tokio::test]
    async fn test_issue_command() {
        // This is a minimal test to ensure the command handler works
        // A full integration test would require mocking the GitHub API
        let command = IssueCommand::new();
        
        // Create a minimal context
        let context = Context::default();
        let mut output = SharedWriter::new(Cursor::new(Vec::new()));
        let mut conversation_state = ConversationState::default();
        let mut tool_permissions = ToolPermissions::default();
        let mut input_source = InputSource::default();
        let settings = Settings::default();
        
        let mut ctx = CommandContextAdapter::new(
            &context,
            &mut output,
            &mut conversation_state,
            &mut tool_permissions,
            true,
            &mut input_source,
            &settings,
        );
        
        // Execute the command
        let args = vec!["Test Issue"];
        let result = command.execute(args, &mut ctx, None, None).await;
        
        // We can't fully test the result since it would open a browser
        // But we can at least check that it doesn't error
        assert!(result.is_ok());
    }
}
