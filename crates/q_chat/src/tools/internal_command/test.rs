#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use eyre::Result;
    use fig_os_shim::Context;

    use crate::tools::Tool;
    use crate::tools::internal_command::schema::InternalCommand;

    #[tokio::test]
    async fn test_internal_command_help() -> Result<()> {
        let ctx = Context::new_fake();
        let mut output = Cursor::new(Vec::new());

        let command = InternalCommand {
            command: "help".to_string(),
            subcommand: None,
            args: None,
            flags: None,
            tool_use_id: None,
        };

        let tool = Tool::InternalCommand(command);
        let result = tool.invoke(&ctx, &mut output).await?;

        // Check that the output contains the help text
        let output_str = String::from_utf8(output.into_inner())?;
        assert!(output_str.contains("/help"));

        // Check that the next state is ExecuteCommand
        assert!(result.next_state.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_internal_command_quit() -> Result<()> {
        let ctx = Context::new_fake();
        let mut output = Cursor::new(Vec::new());

        let command = InternalCommand {
            command: "quit".to_string(),
            subcommand: None,
            args: None,
            flags: None,
            tool_use_id: None,
        };

        let tool = Tool::InternalCommand(command);
        let result = tool.invoke(&ctx, &mut output).await?;

        // Check that the output contains the quit command
        let output_str = String::from_utf8(output.into_inner())?;
        assert!(output_str.contains("/quit"));

        // Check that the next state is ExecuteCommand
        assert!(result.next_state.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_internal_command_context_add() -> Result<()> {
        let ctx = Context::new_fake();
        let mut output = Cursor::new(Vec::new());

        let command = InternalCommand {
            command: "context".to_string(),
            subcommand: Some("add".to_string()),
            args: Some(vec!["file.txt".to_string()]),
            flags: None,
            tool_use_id: None,
        };

        let tool = Tool::InternalCommand(command);
        let result = tool.invoke(&ctx, &mut output).await?;

        // Check that the output contains the context add command
        let output_str = String::from_utf8(output.into_inner())?;
        assert!(output_str.contains("/context add"));
        assert!(output_str.contains("file.txt"));

        // Check that the next state is ExecuteCommand
        assert!(result.next_state.is_some());

        Ok(())
    }
}
