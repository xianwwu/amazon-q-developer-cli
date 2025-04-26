#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use eyre::Result;
    use fig_os_shim::Context;

    use crate::cli::chat::ChatState;
    use crate::cli::chat::command::{Command, ContextSubcommand, ProfileSubcommand, ToolsSubcommand};

    use super::super::schema::InternalCommand;

    #[test]
    fn test_format_command_string() {
        let internal_command = InternalCommand {
            command: "context".to_string(),
            subcommand: Some("add".to_string()),
            args: Some(vec!["file1.txt".to_string(), "file2.txt".to_string()]),
            flags: Some([("global".to_string(), "".to_string())].iter().cloned().collect()),
            tool_use_id: None,
        };

        let command_str = internal_command.format_command_string();
        assert_eq!(command_str, "/context add file1.txt file2.txt --global");
    }

    #[test]
    fn test_get_command_description() {
        let internal_command = InternalCommand {
            command: "context".to_string(),
            subcommand: Some("add".to_string()),
            args: Some(vec!["file.txt".to_string()]),
            flags: None,
            tool_use_id: None,
        };

        let description = internal_command.get_command_description();
        assert_eq!(description, "Add a file to the conversation context");
    }

    #[test]
    fn test_validate_simple_valid_command() {
        let internal_command = InternalCommand {
            command: "help".to_string(),
            subcommand: None,
            args: None,
            flags: None,
            tool_use_id: None,
        };

        let result = internal_command.validate_simple();
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_simple_invalid_command() {
        let internal_command = InternalCommand {
            command: "invalid".to_string(),
            subcommand: None,
            args: None,
            flags: None,
            tool_use_id: None,
        };

        let result = internal_command.validate_simple();
        assert!(result.is_err());
    }

    #[test]
    fn test_requires_acceptance_simple() {
        // Help command should not require acceptance
        let help_command = InternalCommand {
            command: "help".to_string(),
            subcommand: None,
            args: None,
            flags: None,
            tool_use_id: None,
        };
        assert!(!help_command.requires_acceptance_simple());

        // Context show should not require acceptance
        let context_show = InternalCommand {
            command: "context".to_string(),
            subcommand: Some("show".to_string()),
            args: None,
            flags: None,
            tool_use_id: None,
        };
        assert!(!context_show.requires_acceptance_simple());

        // Profile list should not require acceptance
        let profile_list = InternalCommand {
            command: "profile".to_string(),
            subcommand: Some("list".to_string()),
            args: None,
            flags: None,
            tool_use_id: None,
        };
        assert!(!profile_list.requires_acceptance_simple());

        // Quit command should require acceptance
        let quit_command = InternalCommand {
            command: "quit".to_string(),
            subcommand: None,
            args: None,
            flags: None,
            tool_use_id: None,
        };
        assert!(quit_command.requires_acceptance_simple());
    }

    // New tests for ChatState transitions and tool use

    #[tokio::test]
    async fn test_invoke_returns_execute_parsed_command_state() -> Result<()> {
        // Create a simple command
        let internal_command = InternalCommand {
            command: "help".to_string(),
            subcommand: None,
            args: None,
            flags: None,
            tool_use_id: None,
        };

        // Create a mock context
        let context = Context::new_fake();
        let mut output = Vec::new();

        // Invoke the command
        let result = internal_command.invoke(&context, &mut output).await?;

        // Check that the result contains the expected next state
        if let Some(ChatState::ExecuteParsedCommand(command)) = result.next_state {
            assert!(matches!(command, Command::Help));
        } else {
            panic!("Expected ExecuteParsedCommand state, got {:?}", result.next_state);
        }

        // Check that the output contains the expected text
        let output_text = String::from_utf8(output)?;
        assert!(output_text.contains("Suggested command:"));
        assert!(output_text.contains("/help"));

        Ok(())
    }

    #[tokio::test]
    async fn test_invoke_quit_command() -> Result<()> {
        // Create a quit command
        let internal_command = InternalCommand {
            command: "quit".to_string(),
            subcommand: None,
            args: None,
            flags: None,
            tool_use_id: None,
        };

        // Create a mock context
        let context = Context::new_fake();
        let mut output = Vec::new();

        // Invoke the command
        let result = internal_command.invoke(&context, &mut output).await?;

        // Check that the result contains the expected next state
        if let Some(ChatState::ExecuteParsedCommand(command)) = result.next_state {
            assert!(matches!(command, Command::Quit));
        } else {
            panic!("Expected ExecuteParsedCommand state with Quit command, got {:?}", result.next_state);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_invoke_context_add_command() -> Result<()> {
        // Create a context add command
        let internal_command = InternalCommand {
            command: "context".to_string(),
            subcommand: Some("add".to_string()),
            args: Some(vec!["file.txt".to_string()]),
            flags: Some([("global".to_string(), "".to_string())].iter().cloned().collect()),
            tool_use_id: None,
        };

        // Create a mock context
        let context = Context::new_fake();
        let mut output = Vec::new();

        // Invoke the command
        let result = internal_command.invoke(&context, &mut output).await?;

        // Check that the result contains the expected next state
        if let Some(ChatState::ExecuteParsedCommand(Command::Context { subcommand })) = result.next_state {
            if let ContextSubcommand::Add { global, paths, .. } = subcommand {
                assert!(global);
                assert_eq!(paths, vec!["file.txt"]);
            } else {
                panic!("Expected ContextSubcommand::Add, got {:?}", subcommand);
            }
        } else {
            panic!("Expected ExecuteParsedCommand state with Context command, got {:?}", result.next_state);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_invoke_profile_list_command() -> Result<()> {
        // Create a profile list command
        let internal_command = InternalCommand {
            command: "profile".to_string(),
            subcommand: Some("list".to_string()),
            args: None,
            flags: None,
            tool_use_id: None,
        };

        // Create a mock context
        let context = Context::new_fake();
        let mut output = Vec::new();

        // Invoke the command
        let result = internal_command.invoke(&context, &mut output).await?;

        // Check that the result contains the expected next state
        if let Some(ChatState::ExecuteParsedCommand(Command::Profile { subcommand })) = result.next_state {
            assert!(matches!(subcommand, ProfileSubcommand::List));
        } else {
            panic!("Expected ExecuteParsedCommand state with Profile command, got {:?}", result.next_state);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_invoke_tools_trust_command() -> Result<()> {
        // Create a tools trust command
        let internal_command = InternalCommand {
            command: "tools".to_string(),
            subcommand: Some("trust".to_string()),
            args: Some(vec!["fs_write".to_string()]),
            flags: None,
            tool_use_id: None,
        };

        // Create a mock context
        let context = Context::new_fake();
        let mut output = Vec::new();

        // Invoke the command
        let result = internal_command.invoke(&context, &mut output).await?;

        // Check that the result contains the expected next state
        if let Some(ChatState::ExecuteParsedCommand(Command::Tools { subcommand })) = result.next_state {
            if let Some(ToolsSubcommand::Trust { tool_names }) = subcommand {
                assert!(tool_names.contains("fs_write"));
            } else {
                panic!("Expected ToolsSubcommand::Trust, got {:?}", subcommand);
            }
        } else {
            panic!("Expected ExecuteParsedCommand state with Tools command, got {:?}", result.next_state);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_invoke_compact_command() -> Result<()> {
        // Create a compact command with summary flag
        let internal_command = InternalCommand {
            command: "compact".to_string(),
            subcommand: None,
            args: Some(vec!["summarize this conversation".to_string()]),
            flags: Some([("summary".to_string(), "".to_string())].iter().cloned().collect()),
            tool_use_id: None,
        };

        // Create a mock context
        let context = Context::new_fake();
        let mut output = Vec::new();

        // Invoke the command
        let result = internal_command.invoke(&context, &mut output).await?;

        // Check that the result contains the expected next state
        if let Some(ChatState::ExecuteParsedCommand(Command::Compact { prompt, show_summary, help })) = result.next_state {
            assert_eq!(prompt, Some("summarize this conversation".to_string()));
            assert!(show_summary);
            assert!(!help);
        } else {
            panic!("Expected ExecuteParsedCommand state with Compact command, got {:?}", result.next_state);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_invoke_editor_command() -> Result<()> {
        // Create an editor command
        let internal_command = InternalCommand {
            command: "editor".to_string(),
            subcommand: None,
            args: Some(vec!["initial text".to_string()]),
            flags: None,
            tool_use_id: None,
        };

        // Create a mock context
        let context = Context::new_fake();
        let mut output = Vec::new();

        // Invoke the command
        let result = internal_command.invoke(&context, &mut output).await?;

        // Check that the result contains the expected next state
        if let Some(ChatState::ExecuteParsedCommand(Command::PromptEditor { initial_text })) = result.next_state {
            assert_eq!(initial_text, Some("initial text".to_string()));
        } else {
            panic!("Expected ExecuteParsedCommand state with PromptEditor command, got {:?}", result.next_state);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_invoke_invalid_command() -> Result<()> {
        // Create an invalid command
        let internal_command = InternalCommand {
            command: "invalid".to_string(),
            subcommand: None,
            args: None,
            flags: None,
            tool_use_id: None,
        };

        // Create a mock context
        let context = Context::new_fake();
        let mut output = Vec::new();

        // Invoke the command should fail
        let result = internal_command.invoke(&context, &mut output).await;
        assert!(result.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_invoke_missing_required_args() -> Result<()> {
        // Create a command missing required args
        let internal_command = InternalCommand {
            command: "context".to_string(),
            subcommand: Some("add".to_string()),
            args: None, // Missing required file path
            flags: None,
            tool_use_id: None,
        };

        // Create a mock context
        let context = Context::new_fake();
        let mut output = Vec::new();

        // Invoke the command should fail
        let result = internal_command.invoke(&context, &mut output).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing file path"));

        Ok(())
    }

    #[tokio::test]
    async fn test_queue_description() -> Result<()> {
        // Create a command
        let internal_command = InternalCommand {
            command: "help".to_string(),
            subcommand: None,
            args: None,
            flags: None,
            tool_use_id: None,
        };

        // Create output buffer
        let mut output = Vec::new();

        // Queue description
        internal_command.queue_description(&mut output)?;

        // Check output contains expected text
        let output_text = String::from_utf8(output)?;
        assert!(output_text.contains("Suggested command:"));
        assert!(output_text.contains("/help"));

        Ok(())
    }
}
