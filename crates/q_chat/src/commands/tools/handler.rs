// This file is deprecated and should be removed.
// The functionality has been moved to individual command handlers in the tools directory.
// See tools/mod.rs for the new implementation.

use std::future::Future;
use std::pin::Pin;

use crossterm::style::{
    Attribute,
    Color,
};
use crossterm::{
    queue,
    style,
};
use eyre::Result;

use crate::command::ToolsSubcommand;
use crate::commands::context_adapter::CommandContextAdapter;
use crate::commands::handler::CommandHandler;
use crate::tools::Tool;
use crate::{
    ChatState,
    QueuedTool,
};

/// Handler for tools commands
pub struct ToolsCommandHandler;

impl ToolsCommandHandler {
    /// Create a new tools command handler
    pub fn new() -> Self {
        Self
    }
}

impl Default for ToolsCommandHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandHandler for ToolsCommandHandler {
    fn name(&self) -> &'static str {
        "tools"
    }

    fn description(&self) -> &'static str {
        "View and manage tools and permissions"
    }

    fn usage(&self) -> &'static str {
        "/tools [subcommand]"
    }

    fn help(&self) -> String {
        color_print::cformat!(
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
        )
    }

    fn llm_description(&self) -> String {
        r#"The tools command manages tool permissions and settings.

Subcommands:
- list: List all available tools and their trust status
- trust <tool_name>: Trust a specific tool (don't ask for confirmation)
- untrust <tool_name>: Untrust a specific tool (ask for confirmation)
- trustall: Trust all tools
- reset: Reset all tool permissions to default

Examples:
- "/tools list" - Lists all available tools
- "/tools trust fs_write" - Trusts the fs_write tool
- "/tools untrust execute_bash" - Untrusts the execute_bash tool
- "/tools trustall" - Trusts all tools
- "/tools reset" - Resets all tool permissions to default

To get the current tool status, use the command "/tools list" which will display all available tools with their current permission status."#.to_string()
    }

    fn to_command<'a>(&self, args: Vec<&'a str>) -> Result<crate::command::Command> {
        // Parse arguments to determine the subcommand
        let subcommand = if args.is_empty() {
            None // Default to list
        } else if let Some(first_arg) = args.first() {
            match *first_arg {
                "list" => None, // Default is to list tools
                "trust" => {
                    let tool_names = args[1..].iter().map(|s| (*s).to_string()).collect();
                    Some(ToolsSubcommand::Trust { tool_names })
                },
                "untrust" => {
                    let tool_names = args[1..].iter().map(|s| (*s).to_string()).collect();
                    Some(ToolsSubcommand::Untrust { tool_names })
                },
                "trustall" => Some(ToolsSubcommand::TrustAll),
                "reset" => {
                    if args.len() > 1 {
                        Some(ToolsSubcommand::ResetSingle {
                            tool_name: args[1].to_string(),
                        })
                    } else {
                        Some(ToolsSubcommand::Reset)
                    }
                },
                "help" => Some(ToolsSubcommand::Help),
                _ => {
                    // For unknown subcommands, show help
                    Some(ToolsSubcommand::Help)
                },
            }
        } else {
            None // Default to list if no arguments (should not happen due to earlier check)
        };

        Ok(crate::command::Command::Tools { subcommand })
    }

    fn execute_command<'a>(
        &'a self,
        command: &'a crate::command::Command,
        ctx: &'a mut CommandContextAdapter<'a>,
        tool_uses: Option<Vec<QueuedTool>>,
        pending_tool_index: Option<usize>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatState>> + Send + 'a>> {
        Box::pin(async move {
            // Extract the subcommand from the command
            let subcommand = match command {
                crate::command::Command::Tools { subcommand } => subcommand,
                _ => return Err(eyre::eyre!("Unexpected command type for this handler")),
            };

            match subcommand {
                None => {
                    // List all tools and their status
                    queue!(
                        ctx.output,
                        style::Print("\nTrusted tools can be run without confirmation\n\n")
                    )?;

                    // Get all tool names
                    let tool_names = Tool::all_tool_names();

                    // Display each tool with its permission status
                    for tool_name in tool_names {
                        let permission_label = ctx.tool_permissions.display_label(tool_name);

                        queue!(
                            ctx.output,
                            style::Print("- "),
                            style::Print(format!("{:<20} ", tool_name)),
                            style::Print(permission_label),
                            style::Print("\n")
                        )?;
                    }

                    // Add a note about default settings
                    queue!(
                        ctx.output,
                        style::SetForegroundColor(Color::DarkGrey),
                        style::Print("\n* Default settings\n\n"),
                        style::Print("💡 Use "),
                        style::SetForegroundColor(Color::Green),
                        style::Print("/tools help"),
                        style::SetForegroundColor(Color::DarkGrey),
                        style::Print(" to edit permissions.\n"),
                        style::ResetColor,
                        style::Print("\n")
                    )?;
                },
                Some(ToolsSubcommand::Trust { tool_names }) => {
                    // Trust the specified tools
                    for tool_name in tool_names {
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
                        ctx.tool_permissions.trust_tool(&tool_name);

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
                },
                Some(ToolsSubcommand::Untrust { tool_names }) => {
                    // Untrust the specified tools
                    for tool_name in tool_names {
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

                        // Untrust the tool
                        ctx.tool_permissions.untrust_tool(&tool_name);

                        queue!(
                            ctx.output,
                            style::SetForegroundColor(Color::Green),
                            style::Print(format!("\nTool '{}' is set to per-request confirmation.\n", tool_name)),
                            style::ResetColor
                        )?;
                    }

                    queue!(ctx.output, style::Print("\n"))?;
                },
                Some(ToolsSubcommand::TrustAll) => {
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
                },
                Some(ToolsSubcommand::Reset) => {
                    // Reset all tool permissions
                    ctx.tool_permissions.reset();

                    queue!(
                        ctx.output,
                        style::SetForegroundColor(Color::Green),
                        style::Print("\nReset all tools to the default permission levels.\n"),
                        style::ResetColor,
                        style::Print("\n")
                    )?;
                },
                Some(ToolsSubcommand::ResetSingle { tool_name }) => {
                    // Check if the tool exists
                    if !Tool::all_tool_names().contains(&tool_name.as_str()) {
                        queue!(
                            ctx.output,
                            style::SetForegroundColor(Color::Red),
                            style::Print(format!("\nUnknown tool: '{}'\n\n", tool_name)),
                            style::ResetColor
                        )?;
                    } else {
                        // Reset the tool permission
                        ctx.tool_permissions.reset_tool(&tool_name);

                        queue!(
                            ctx.output,
                            style::SetForegroundColor(Color::Green),
                            style::Print(format!("\nReset tool '{}' to default permission level.\n\n", tool_name)),
                            style::ResetColor
                        )?;
                    }
                },
                Some(ToolsSubcommand::Help) => {
                    // Display help text
                    queue!(
                        ctx.output,
                        style::Print("\n"),
                        style::Print(self.help()),
                        style::Print("\n")
                    )?;
                },
                Some(ToolsSubcommand::Schema) => {
                    // This is handled elsewhere
                    queue!(
                        ctx.output,
                        style::Print("\nShowing tool schemas is not implemented in this handler.\n\n")
                    )?;
                },
            }

            Ok(ChatState::PromptUser {
                tool_uses,
                pending_tool_index,
                skip_printing_tools: false,
            })
        })
    }

    fn requires_confirmation(&self, args: &[&str]) -> bool {
        if args.is_empty() {
            return false; // Default list doesn't require confirmation
        }

        match args[0] {
            "help" | "list" => false, // Help and list don't require confirmation
            "trustall" => true,       // Trustall requires confirmation
            _ => false,               // Other commands don't require confirmation
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use fig_os_shim::Context;

    use super::*;
    use crate::Settings;
    use crate::conversation_state::ConversationState;
    use crate::input_source::InputSource;
    use crate::util::shared_writer::SharedWriter;
    use crate::tools::ToolPermissions;

    #[tokio::test]
    async fn test_tools_list_command() {
        let handler = ToolsCommandHandler::new();

        // Create a minimal context
        let context = Arc::new(Context::new_fake());
        let output = SharedWriter::null();
        let mut conversation_state =
            ConversationState::new(Arc::clone(&context), HashMap::new(), None, Some(SharedWriter::null())).await;
        let mut tool_permissions = ToolPermissions::new(0);
        let mut input_source = InputSource::new_mock(vec![]);
        let settings = Settings::new_fake();

        let mut ctx = CommandContextAdapter {
            context: &context,
            output: &mut output.clone(),
            conversation_state: &mut conversation_state,
            tool_permissions: &mut tool_permissions,
            interactive: true,
            input_source: &mut input_source,
            settings: &settings,
        };

        // Execute the list subcommand
        let args = vec!["list"];
        let result = handler.execute(args, &mut ctx, None, None).await;

        assert!(result.is_ok());
    }
}
