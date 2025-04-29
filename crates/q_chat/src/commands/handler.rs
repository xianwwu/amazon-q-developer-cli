/// CommandHandler Trait
///
/// The CommandHandler trait defines the interface for all command handlers in the Q chat system.
/// Each command handler is responsible for parsing, validating, and executing a specific command.
///
/// # Design Philosophy
///
/// The CommandHandler trait follows these key principles:
///
/// 1. **Encapsulation**: Each handler encapsulates all knowledge about a specific command,
///    including its name, description, usage, parsing logic, and execution behavior.
///
/// 2. **Single Responsibility**: Each handler is responsible for one command and does it well.
///
/// 3. **Extensibility**: The trait is designed to be extended with new methods as needed, such as
///    `to_command` for converting arguments to a Command enum.
///
/// # Command Parsing and Execution
///
/// The trait separates command parsing from execution:
///
/// - `to_command`: Converts string arguments to a Command enum variant
/// - `execute`: Default implementation that delegates to `to_command` and wraps the result in a
///   ChatState
///
/// This separation allows tools like internal_command to leverage the parsing logic
/// without duplicating code, while preserving the execution flow for direct command invocation.
use std::future::Future;
use std::pin::Pin;

use eyre::Result;

use super::context_adapter::CommandContextAdapter;
use crate::command::Command;
use crate::{
    ChatState,
    QueuedTool,
};

/// Trait for command handlers
pub trait CommandHandler: Send + Sync {
    /// Returns the name of the command
    #[allow(dead_code)]
    fn name(&self) -> &'static str;

    /// Returns a short description of the command for help text
    #[allow(dead_code)]
    fn description(&self) -> &'static str;

    /// Returns usage information for the command
    fn usage(&self) -> &'static str;

    /// Returns detailed help text for the command
    fn help(&self) -> String;

    /// Converts string arguments to a Command enum variant
    ///
    /// This method takes a vector of string slices and returns a Command enum.
    /// It's used by the execute method and can also be used directly by tools
    /// like internal_command to parse commands without executing them.
    fn to_command(&self, args: Vec<&str>) -> Result<Command>;

    /// Returns a detailed description with examples for LLM tool descriptions
    /// This is used to provide more context to the LLM about how to use the command
    #[allow(dead_code)]
    fn llm_description(&self) -> String {
        // Default implementation returns the regular help text
        self.help()
    }

    /// Execute the command with the given arguments
    ///
    /// This method is async to allow for operations that require async/await,
    /// such as file system operations or network requests.
    ///
    /// The default implementation delegates to to_command and wraps the result
    /// in a ChatState::ExecuteCommand.
    fn execute<'a>(
        &'a self,
        args: Vec<&'a str>,
        _ctx: &'a mut CommandContextAdapter<'a>,
        tool_uses: Option<Vec<QueuedTool>>,
        pending_tool_index: Option<usize>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatState>> + Send + 'a>> {
        Box::pin(async move {
            let command = self.to_command(args)?;
            Ok(ChatState::ExecuteCommand {
                command,
                tool_uses,
                pending_tool_index,
            })
        })
    }

    /// Check if this command requires confirmation before execution
    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        true // Most commands require confirmation by default
    }

    /// Parse arguments for this command
    ///
    /// This method takes a vector of string slices and returns a vector of string slices.
    /// The lifetime of the returned slices must be the same as the lifetime of the input slices.
    fn parse_args<'a>(&self, args: Vec<&'a str>) -> Result<Vec<&'a str>> {
        Ok(args)
    }
}
