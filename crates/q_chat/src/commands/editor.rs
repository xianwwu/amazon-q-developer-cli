use std::future::Future;
use std::io::Write;
use std::pin::Pin;
use std::process::Command;
use std::{
    env,
    fs,
};

use crossterm::style::Color;
use crossterm::{
    queue,
    style,
};
use eyre::Result;
use tempfile::NamedTempFile;
use tracing::{
    debug,
    error,
};

use super::context_adapter::CommandContextAdapter;
use super::handler::CommandHandler;
use crate::{
    ChatState,
    QueuedTool,
};

/// Command handler for the `/editor` command
pub struct EditorCommand;

impl Default for EditorCommand {
    fn default() -> Self {
        Self
    }
}

impl EditorCommand {
    /// Create a new instance of the EditorCommand
    pub fn new() -> Self {
        Self
    }

    /// Get the default editor from environment or fallback to platform-specific defaults
    fn get_default_editor() -> String {
        if let Ok(editor) = env::var("EDITOR") {
            return editor;
        }

        #[cfg(target_os = "windows")]
        {
            return "notepad.exe".to_string();
        }

        #[cfg(not(target_os = "windows"))]
        {
            // Try to find common editors
            for editor in &["nano", "vim", "vi", "emacs"] {
                if Command::new("which")
                    .arg(editor)
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
                {
                    return (*editor).to_string();
                }
            }

            // Fallback to vi which should be available on most Unix systems
            "vi".to_string()
        }
    }
}

impl CommandHandler for EditorCommand {
    fn name(&self) -> &'static str {
        "editor"
    }

    fn description(&self) -> &'static str {
        "Open an external editor for composing prompts"
    }

    fn usage(&self) -> &'static str {
        "/editor [initial_text]"
    }

    fn help(&self) -> String {
        color_print::cformat!(
            r#"
<magenta,em>External Editor</magenta,em>

Opens your default text editor to compose a longer or more complex prompt.

<cyan!>Usage: /editor [initial_text]</cyan!>

<cyan!>Description</cyan!>
  Opens your system's default text editor (as specified by the EDITOR environment variable)
  with optional initial text. After you save and close the editor, the content is sent as
  a prompt to Amazon Q.

<cyan!>Examples</cyan!>
  /editor
  /editor Please help me with the following code:

<cyan!>Notes</cyan!>
• Uses your system's default editor (EDITOR environment variable)
• Common editors include vim, nano, emacs, VS Code, etc.
• Useful for multi-paragraph prompts or code snippets
• All content from the editor is sent as a single prompt
"#
        )
    }

    fn llm_description(&self) -> String {
        r#"
The editor command opens an external text editor for composing longer or more complex prompts.

Usage:
- /editor [initial_text]

This command:
- Opens the user's default text editor (from EDITOR environment variable)
- Pre-populates the editor with initial_text if provided
- Sends the edited content as a prompt to Amazon Q when the editor is closed

This command is useful when:
- The user wants to compose a multi-paragraph prompt
- The user needs to include code snippets with proper formatting
- The user wants to carefully edit their prompt before sending it
- The prompt contains special characters or formatting

The command takes an optional initial text parameter that will be pre-populated in the editor.

Examples:
- "/editor" - Opens an empty editor
- "/editor Please help me with this code:" - Opens editor with initial text
"#
        .to_string()
    }

    fn to_command(&self, args: Vec<&str>) -> Result<crate::command::Command> {
        let initial_text = if !args.is_empty() { Some(args.join(" ")) } else { None };

        Ok(crate::command::Command::PromptEditor { initial_text })
    }

    fn execute<'a>(
        &'a self,
        args: Vec<&'a str>,
        ctx: &'a mut CommandContextAdapter<'a>,
        _tool_uses: Option<Vec<QueuedTool>>,
        _pending_tool_index: Option<usize>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatState>> + Send + 'a>> {
        Box::pin(async move {
            // Get initial text from args if provided
            let initial_text = if !args.is_empty() { Some(args.join(" ")) } else { None };

            // Create a temporary file for editing
            let mut temp_file = match NamedTempFile::new() {
                Ok(file) => file,
                Err(e) => {
                    error!("Failed to create temporary file: {}", e);
                    queue!(
                        ctx.output,
                        style::SetForegroundColor(Color::Red),
                        style::Print("Error: Failed to create temporary file for editor.\n"),
                        style::ResetColor
                    )?;
                    return Ok(ChatState::PromptUser {
                        tool_uses: None,
                        pending_tool_index: None,
                        skip_printing_tools: false,
                    });
                },
            };

            // Write initial text to the file if provided
            if let Some(text) = initial_text {
                if let Err(e) = temp_file.write_all(text.as_bytes()) {
                    error!("Failed to write initial text to temporary file: {}", e);
                    queue!(
                        ctx.output,
                        style::SetForegroundColor(Color::Red),
                        style::Print("Error: Failed to write initial text to editor.\n"),
                        style::ResetColor
                    )?;
                    return Ok(ChatState::PromptUser {
                        tool_uses: None,
                        pending_tool_index: None,
                        skip_printing_tools: false,
                    });
                }
                // Flush to ensure content is written before editor opens
                if let Err(e) = temp_file.flush() {
                    error!("Failed to flush temporary file: {}", e);
                }
            }

            // Get the path to the temporary file
            let temp_path = temp_file.path().to_path_buf();

            // Get the editor command
            let editor = Self::get_default_editor();

            // Inform the user about the editor being opened
            queue!(
                ctx.output,
                style::Print("\nOpening external editor ("),
                style::SetForegroundColor(Color::Cyan),
                style::Print(&editor),
                style::ResetColor,
                style::Print(")...\n")
            )?;

            // Close the file to allow the editor to access it
            drop(temp_file);

            // Open the editor
            debug!("Opening editor {} with file {:?}", editor, temp_path);
            let status = Command::new(&editor).arg(&temp_path).status();

            match status {
                Ok(exit_status) if exit_status.success() => {
                    // Read the content from the file
                    match fs::read_to_string(&temp_path) {
                        Ok(content) if !content.trim().is_empty() => {
                            // Inform the user that the content is being sent
                            queue!(
                                ctx.output,
                                style::Print("\nSending content from editor to Amazon Q...\n\n")
                            )?;

                            // Return the content as a prompt
                            Ok(ChatState::HandleInput {
                                input: content,
                                tool_uses: None,
                                pending_tool_index: None,
                            })
                        },
                        Ok(_) => {
                            // Empty content
                            queue!(
                                ctx.output,
                                style::SetForegroundColor(Color::Yellow),
                                style::Print("\nEditor content was empty. No prompt sent.\n"),
                                style::ResetColor
                            )?;

                            Ok(ChatState::PromptUser {
                                tool_uses: None,
                                pending_tool_index: None,
                                skip_printing_tools: false,
                            })
                        },
                        Err(e) => {
                            error!("Failed to read content from temporary file: {}", e);
                            queue!(
                                ctx.output,
                                style::SetForegroundColor(Color::Red),
                                style::Print("\nError: Failed to read content from editor.\n"),
                                style::ResetColor
                            )?;

                            Ok(ChatState::PromptUser {
                                tool_uses: None,
                                pending_tool_index: None,
                                skip_printing_tools: false,
                            })
                        },
                    }
                },
                Ok(_) => {
                    // Editor exited with non-zero status
                    queue!(
                        ctx.output,
                        style::SetForegroundColor(Color::Yellow),
                        style::Print("\nEditor closed without saving or encountered an error.\n"),
                        style::ResetColor
                    )?;

                    Ok(ChatState::PromptUser {
                        tool_uses: None,
                        pending_tool_index: None,
                        skip_printing_tools: false,
                    })
                },
                Err(e) => {
                    error!("Failed to open editor: {}", e);
                    queue!(
                        ctx.output,
                        style::SetForegroundColor(Color::Red),
                        style::Print(format!(
                            "\nError: Failed to open editor ({}). Make sure it's installed and in your PATH.\n",
                            editor
                        )),
                        style::ResetColor
                    )?;

                    Ok(ChatState::PromptUser {
                        tool_uses: None,
                        pending_tool_index: None,
                        skip_printing_tools: false,
                    })
                },
            }
        })
    }

    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        // Editor command doesn't require confirmation
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_editor_command_help() {
        let command = EditorCommand::new();

        // Verify the command metadata
        assert_eq!(command.name(), "editor");
        assert_eq!(command.description(), "Open an external editor for composing prompts");
        assert_eq!(command.usage(), "/editor [initial_text]");

        // Verify help text contains key information
        let help_text = command.help();
        assert!(help_text.contains("External Editor"));
        assert!(help_text.contains("EDITOR environment variable"));
    }

    // Note: We can't easily test the actual editor execution in unit tests
    // as it depends on the system environment and available editors.
    // Instead, we focus on testing the command setup and metadata.
}
