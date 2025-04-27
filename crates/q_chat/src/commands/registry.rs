/// Command Registry
///
/// The CommandRegistry is a central repository for all commands available in the Q chat system.
/// It provides a unified interface for registering, discovering, and executing commands.
///
/// # Design Philosophy
///
/// The CommandRegistry follows these key principles:
///
/// 1. **Single Source of Truth**: Each command should be defined in exactly one place. The
///    CommandHandler for a command is the authoritative source for all information about that
///    command.
///
/// 2. **Bidirectional Mapping**: The registry should support bidirectional mapping between:
///    - Command names (strings) and CommandHandlers
///    - Command enum variants and CommandHandlers
///
/// 3. **DRY (Don't Repeat Yourself)**: Command parsing, validation, and execution logic should be
///    defined once in the CommandHandler and reused everywhere, including in tools like
///    internal_command.
///
/// # Future Enhancements
///
/// In future iterations, the CommandRegistry should be enhanced to:
///
/// 1. Add a `to_command` method to the CommandHandler trait that converts arguments to a Command
///    enum
/// 2. Add a `from_command` function that converts a Command enum to its corresponding
///    CommandHandler
/// 3. Merge the Command enum and CommandRegistry for a more cohesive command system
///
/// This will enable tools like internal_command to leverage the existing command infrastructure
/// without duplicating logic.
use std::collections::HashMap;
use std::sync::OnceLock;

use eyre::Result;

use crate::commands::{
    ClearCommand,
    CommandContextAdapter,
    CommandHandler,
    CompactCommand,
    ContextCommand,
    HelpCommand,
    ProfileCommand,
    QuitCommand,
    ToolsCommand,
};
use crate::{
    ChatContext,
    ChatState,
    QueuedTool,
};

/// A registry of available commands that can be executed
pub struct CommandRegistry {
    /// Map of command names to their handlers
    commands: HashMap<String, Box<dyn CommandHandler>>,
}

impl CommandRegistry {
    /// Create a new command registry with all built-in commands
    pub fn new() -> Self {
        let mut registry = Self {
            commands: HashMap::new(),
        };

        // Register built-in commands
        registry.register("quit", Box::new(QuitCommand::new()));
        registry.register("clear", Box::new(ClearCommand::new()));
        registry.register("help", Box::new(HelpCommand::new()));
        registry.register("context", Box::new(ContextCommand::new()));
        registry.register("profile", Box::new(ProfileCommand::new()));
        registry.register("tools", Box::new(ToolsCommand::new()));
        registry.register("compact", Box::new(CompactCommand::new()));

        registry
    }

    /// Get the global instance of the command registry
    pub fn global() -> &'static CommandRegistry {
        static INSTANCE: OnceLock<CommandRegistry> = OnceLock::new();
        INSTANCE.get_or_init(CommandRegistry::new)
    }

    /// Register a new command handler
    pub fn register(&mut self, name: &str, handler: Box<dyn CommandHandler>) {
        self.commands.insert(name.to_string(), handler);
    }

    /// Get a command handler by name
    pub fn get(&self, name: &str) -> Option<&dyn CommandHandler> {
        self.commands.get(name).map(|h| h.as_ref())
    }

    /// Check if a command exists
    #[allow(dead_code)]
    pub fn command_exists(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }

    /// Get all command names
    #[allow(dead_code)]
    pub fn command_names(&self) -> Vec<&String> {
        self.commands.keys().collect()
    }

    /// Generate a description of all available commands for help text
    #[allow(dead_code)]
    pub fn generate_commands_description(&self) -> String {
        let mut description = String::new();

        for name in self.command_names() {
            if let Some(handler) = self.get(name) {
                description.push_str(&format!("{} - {}\n", handler.usage(), handler.description()));
            }
        }

        description
    }

    /// Generate structured command information for LLM reference
    pub fn generate_llm_descriptions(&self) -> serde_json::Value {
        let mut commands = serde_json::Map::new();

        for name in self.command_names() {
            if let Some(handler) = self.get(name) {
                commands.insert(
                    name.to_string(),
                    serde_json::json!({
                        "description": handler.llm_description(),
                        "usage": handler.usage(),
                        "help": handler.help()
                    }),
                );
            }
        }

        serde_json::json!(commands)
    }

    /// Parse and execute a command string
    pub async fn parse_and_execute(
        &self,
        input: &str,
        chat_context: &mut ChatContext,
        tool_uses: Option<Vec<QueuedTool>>,
        pending_tool_index: Option<usize>,
    ) -> Result<ChatState> {
        let (name, args) = Self::parse_command_string(input)?;

        if let Some(handler) = self.get(name) {
            let parsed_args = handler.parse_args(args)?;

            // Create a CommandContextAdapter from the ChatContext
            let mut adapter = CommandContextAdapter::new(
                &chat_context.ctx,
                &mut chat_context.output,
                &mut chat_context.conversation_state,
                &mut chat_context.tool_permissions,
                chat_context.interactive,
                &mut chat_context.input_source,
                &chat_context.settings,
            );

            handler
                .execute(parsed_args, &mut adapter, tool_uses, pending_tool_index)
                .await
        } else {
            // If not a registered command, treat as a question to the AI
            Ok(ChatState::HandleInput {
                input: input.to_string(),
                tool_uses,
                pending_tool_index,
            })
        }
    }

    /// Parse a command string into name and arguments
    pub fn parse_command_string(input: &str) -> Result<(&str, Vec<&str>)> {
        let input = input.trim();

        // Handle slash commands
        if let Some(stripped) = input.strip_prefix('/') {
            let parts: Vec<&str> = stripped.splitn(2, ' ').collect();
            let command = parts[0];
            let args = if parts.len() > 1 {
                parts[1].split_whitespace().collect()
            } else {
                Vec::new()
            };

            Ok((command, args))
        } else {
            // Not a slash command
            Err(eyre::eyre!("Not a command: {}", input))
        }
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::Pin;

    use super::*;

    #[test]
    fn test_command_registry_register_and_get() {
        let mut registry = CommandRegistry::new();

        // Create a simple command handler
        struct TestCommand;
        impl CommandHandler for TestCommand {
            fn name(&self) -> &'static str {
                "test"
            }

            fn description(&self) -> &'static str {
                "Test command"
            }

            fn usage(&self) -> &'static str {
                "/test"
            }

            fn help(&self) -> String {
                "Test command help".to_string()
            }

            fn execute<'a>(
                &'a self,
                _args: Vec<&'a str>,
                _ctx: &'a mut CommandContextAdapter<'a>,
                _tool_uses: Option<Vec<QueuedTool>>,
                _pending_tool_index: Option<usize>,
            ) -> Pin<Box<dyn Future<Output = Result<ChatState>> + Send + 'a>> {
                Box::pin(async move { Ok(ChatState::Exit) })
            }
        }

        // Register the test command
        registry.register("test", Box::new(TestCommand));

        // Verify the command exists
        assert!(registry.command_exists("test"));

        // Verify we can get the command
        let handler = registry.get("test").unwrap();
        assert_eq!(handler.name(), "test");
        assert_eq!(handler.description(), "Test command");
        assert_eq!(handler.usage(), "/test");
        assert_eq!(handler.help(), "Test command help");
    }

    #[test]
    fn test_parse_command_string() {
        // Test basic command
        let (name, args) = CommandRegistry::parse_command_string("/test").unwrap();
        assert_eq!(name, "test");
        assert!(args.is_empty());

        // Test command with arguments
        let (name, args) = CommandRegistry::parse_command_string("/test arg1 arg2").unwrap();
        assert_eq!(name, "test");
        assert_eq!(args, vec!["arg1", "arg2"]);

        // Test non-command
        assert!(CommandRegistry::parse_command_string("test").is_err());
    }
}
