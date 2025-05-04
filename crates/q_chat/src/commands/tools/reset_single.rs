use std::future::Future;
use std::io::Write;
use std::pin::Pin;

use crossterm::queue;
use crossterm::style::{
    self,
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

/// Static instance of the tools reset single command handler
pub static RESET_SINGLE_TOOL_HANDLER: ResetSingleToolCommand = ResetSingleToolCommand;

/// Handler for the tools reset single command
pub struct ResetSingleToolCommand;
impl CommandHandler for ResetSingleToolCommand {
    fn name(&self) -> &'static str {
        "reset"
    }

    fn description(&self) -> &'static str {
        "Reset a specific tool to default permission level"
    }

    fn usage(&self) -> &'static str {
        "/tools reset <tool_name>"
    }

    fn help(&self) -> String {
        "Reset a specific tool to its default permission level.".to_string()
    }

    fn to_command(&self, args: Vec<&str>) -> Result<Command> {
        if args.len() != 1 {
            return Err(eyre::eyre!("Expected tool name argument"));
        }

        Ok(Command::Tools {
            subcommand: Some(ToolsSubcommand::ResetSingle {
                tool_name: args[0].to_string(),
            }),
        })
    }

    fn execute<'a>(
        &'a self,
        args: Vec<&'a str>,
        ctx: &'a mut CommandContextAdapter<'a>,
        tool_uses: Option<Vec<QueuedTool>>,
        pending_tool_index: Option<usize>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatState>> + Send + 'a>> {
        Box::pin(async move {
            // Parse the command to get the tool name
            let command = self.to_command(args)?;

            // Extract the tool name from the command
            let tool_name = match command {
                Command::Tools {
                    subcommand: Some(ToolsSubcommand::ResetSingle { tool_name }),
                } => tool_name,
                _ => return Err(eyre::eyre!("Invalid command")),
            };

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
            ctx.output.flush()?;

            Ok(ChatState::PromptUser {
                tool_uses,
                pending_tool_index,
                skip_printing_tools: false,
            })
        })
    }

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        false // Reset single command doesn't require confirmation
    }
}
