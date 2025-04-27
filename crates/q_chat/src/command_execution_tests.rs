#[cfg(test)]
mod command_execution_tests {
    use std::collections::HashMap;

    use eyre::Result;
    use fig_api_client::StreamingClient;
    use fig_os_shim::Context;
    use fig_settings::{
        Settings,
        State,
    };

    use crate::command::Command;
    use crate::conversation_state::ConversationState;
    use crate::input_source::InputSource;
    use crate::shared_writer::SharedWriter;
    use crate::tools::internal_command::schema::InternalCommand;
    use crate::tools::{
        Tool,
        ToolPermissions,
    };
    use crate::{
        ChatContext,
        ChatState,
        ToolUseStatus,
    };

    #[tokio::test]
    async fn test_execute_parsed_command_quit() -> Result<()> {
        // Create a mock ChatContext
        let mut chat_context = create_test_chat_context().await?;

        // Execute the quit command
        let result = chat_context.execute_parsed_command(Command::Quit).await?;

        // Verify that the result is ChatState::Exit
        assert!(matches!(result, ChatState::Exit));

        Ok(())
    }

    #[tokio::test]
    async fn test_execute_parsed_command_help() -> Result<()> {
        // Create a mock ChatContext
        let mut chat_context = create_test_chat_context().await?;

        // Execute the help command
        let result = chat_context.execute_parsed_command(Command::Help).await?;

        // Verify that the result is ChatState::DisplayHelp
        if let ChatState::DisplayHelp { help_text, .. } = result {
            assert!(!help_text.is_empty());
        } else {
            panic!("Expected ChatState::DisplayHelp, got {:?}", result);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_execute_parsed_command_compact() -> Result<()> {
        // Create a mock ChatContext
        let mut chat_context = create_test_chat_context().await?;

        // Execute the compact command
        let result = chat_context
            .execute_parsed_command(Command::Compact {
                prompt: Some("test prompt".to_string()),
                show_summary: true,
                help: false,
            })
            .await?;

        // Verify that the result is ChatState::Compact
        if let ChatState::Compact {
            prompt,
            show_summary,
            help,
        } = result
        {
            assert_eq!(prompt, Some("test prompt".to_string()));
            assert!(show_summary);
            assert!(!help);
        } else {
            panic!("Expected ChatState::Compact, got {:?}", result);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_execute_parsed_command_other() -> Result<()> {
        // Create a mock ChatContext
        let mut chat_context = create_test_chat_context().await?;

        // Execute a command that falls back to handle_input
        let result = chat_context.execute_parsed_command(Command::Clear).await;

        // Just verify that the method doesn't panic
        assert!(result.is_ok());

        Ok(())
    }

    #[tokio::test]
    async fn test_tool_to_command_execution_flow() -> Result<()> {
        // Create a mock ChatContext
        let mut chat_context = create_test_chat_context().await?;

        // Create an internal command tool
        let internal_command = InternalCommand {
            command: "help".to_string(),
            subcommand: None,
            args: None,
            flags: None,
            tool_use_id: None,
        };
        let tool = Tool::InternalCommand(internal_command);

        // Invoke the tool
        let mut output = Vec::new();
        let invoke_result = tool.invoke(&chat_context.ctx, &mut output).await?;

        // Verify that the result contains ExecuteParsedCommand state
        if let Some(ChatState::ExecuteParsedCommand(command)) = invoke_result.next_state {
            assert!(matches!(command, Command::Help));

            // Now execute the parsed command
            let execute_result = chat_context.execute_parsed_command(command).await?;

            // Verify that the result is ChatState::DisplayHelp
            if let ChatState::DisplayHelp { help_text, .. } = execute_result {
                assert!(!help_text.is_empty());
            } else {
                panic!("Expected ChatState::DisplayHelp, got {:?}", execute_result);
            }
        } else {
            panic!(
                "Expected ChatState::ExecuteParsedCommand, got {:?}",
                invoke_result.next_state
            );
        }

        Ok(())
    }

    async fn create_test_chat_context() -> Result<ChatContext> {
        // Create a context - Context::new_fake() already returns an Arc<Context>
        let ctx = Context::new_fake();
        let settings = Settings::new_fake();
        let state = State::new_fake();
        let output = SharedWriter::null();
        let input_source = InputSource::new_mock(vec![]);
        let interactive = true;
        let client = StreamingClient::mock(vec![]);

        // Create a tool config
        let tool_config = HashMap::new();

        // Create a conversation state
        let conversation_state = ConversationState::new(ctx.clone(), tool_config, None, None).await;

        // Create the chat context
        let chat_context = ChatContext {
            ctx,
            settings,
            state,
            output,
            initial_input: None,
            input_source,
            interactive,
            client,
            terminal_width_provider: || Some(80),
            spinner: None,
            conversation_state,
            tool_permissions: ToolPermissions::new(10),
            tool_use_telemetry_events: HashMap::new(),
            tool_use_status: ToolUseStatus::Idle,
            failed_request_ids: Vec::new(),
        };

        Ok(chat_context)
    }
}
