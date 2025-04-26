pub mod schema;
#[cfg(test)]
mod test;
pub mod tool;

pub use schema::InternalCommand;

use crate::ToolSpec;

/// Get the tool specification for internal_command
///
/// This function builds the tool specification for the internal_command tool
/// with a comprehensive description of available commands.
pub fn get_tool_spec() -> ToolSpec {
    // Build a comprehensive description that includes all commands
    let mut description = "Tool for suggesting internal Q commands based on user intent. ".to_string();
    description.push_str("This tool helps the AI suggest appropriate commands within the Q chat system ");
    description.push_str("when a user's natural language query indicates they want to perform a specific action.\n\n");
    description.push_str("Available commands:\n");

    // Add each command to the description
    description.push_str("- help: Show help information\n");
    description.push_str("- quit: Exit the chat session\n");
    description.push_str("- clear: Clear the conversation history\n");
    description.push_str("- context: Manage conversation context files\n");
    description.push_str("- profile: Manage profiles\n");
    description.push_str("- tools: Manage tool permissions and settings\n");
    description.push_str("- issue: Create a GitHub issue for reporting bugs or feature requests\n");
    description.push_str("- compact: Summarize and compact the conversation history\n");
    description.push_str("- editor: Open an external editor to compose a prompt\n");

    // Add examples of natural language that should trigger this tool
    description.push_str("\nExamples of natural language that should trigger this tool:\n");
    description.push_str("- \"Clear my conversation\" -> internal_command with command=\"clear\"\n");
    description.push_str(
        "- \"I want to add a file as context\" -> internal_command with command=\"context\", subcommand=\"add\"\n",
    );
    description.push_str(
        "- \"Show me the available profiles\" -> internal_command with command=\"profile\", subcommand=\"list\"\n",
    );
    description.push_str("- \"Exit the application\" -> internal_command with command=\"quit\"\n");
    description.push_str("- \"Add this file to my context\" -> internal_command with command=\"context\", subcommand=\"add\", args=[\"file.txt\"]\n");
    description.push_str(
        "- \"How do I switch profiles?\" -> internal_command with command=\"profile\", subcommand=\"help\"\n",
    );
    description.push_str("- \"I need to report a bug\" -> internal_command with command=\"issue\"\n");
    description.push_str("- \"Let me trust the file write tool\" -> internal_command with command=\"tools\", subcommand=\"trust\", args=[\"fs_write\"]\n");
    description.push_str(
        "- \"Show what tools are available\" -> internal_command with command=\"tools\", subcommand=\"list\"\n",
    );
    description.push_str("- \"I want to start fresh\" -> internal_command with command=\"clear\"\n");
    description.push_str("- \"Can you help me create a new profile?\" -> internal_command with command=\"profile\", subcommand=\"create\"\n");
    description.push_str("- \"I'd like to see what context files I have\" -> internal_command with command=\"context\", subcommand=\"show\"\n");
    description.push_str("- \"Remove the second context file\" -> internal_command with command=\"context\", subcommand=\"rm\", args=[\"2\"]\n");
    description.push_str(
        "- \"Trust all tools for this session\" -> internal_command with command=\"tools\", subcommand=\"trustall\"\n",
    );
    description.push_str(
        "- \"Reset tool permissions to default\" -> internal_command with command=\"tools\", subcommand=\"reset\"\n",
    );
    description.push_str("- \"I want to compact the conversation\" -> internal_command with command=\"compact\"\n");
    description.push_str("- \"Show me the help for context commands\" -> internal_command with command=\"context\", subcommand=\"help\"\n");

    // Create the tool specification
    serde_json::from_value(serde_json::json!({
        "name": "internal_command",
        "description": description,
        "input_schema": {
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command to execute (without the leading slash). Available commands: quit, clear, help, context, profile, tools, issue, compact, editor"
                },
                "subcommand": {
                    "type": "string",
                    "description": "Optional subcommand for commands that support them (context, profile, tools)"
                },
                "args": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "description": "Optional arguments for the command"
                },
                "flags": {
                    "type": "object",
                    "additionalProperties": {
                        "type": "string"
                    },
                    "description": "Optional flags for the command"
                }
            },
            "required": ["command"]
        }
    })).expect("Failed to create tool spec")
}
