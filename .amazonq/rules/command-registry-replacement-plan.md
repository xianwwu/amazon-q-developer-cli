# Command Registry Replacement Plan

## Overview

This document outlines the plan for replacing the CommandRegistry with a command-centric architecture that leverages the bidirectional relationship between Commands and Handlers. The Command enum will become the central point for command-related functionality, making the code more maintainable and reducing indirection.

## Key Changes

1. **Command Enum Enhancement**:
   - Add `parse()` static method to parse command strings into Command enums
   - Add `execute()` method for direct command execution
   - Add `generate_llm_descriptions()` static method for LLM integration
   - Use a static HashMap for command name to handler mapping

2. **CommandHandler Trait Enhancement**:
   - Add `execute_command()` method that works directly with Command objects
   - Each handler implements this method to handle its specific Command variant
   - Handlers return errors for unexpected command types

3. **CommandRegistry Removal**:
   - Replace all CommandRegistry calls with direct Command enum calls
   - Remove the CommandRegistry class entirely

## Implementation Details

### 1. Update CommandHandler Trait

```rust
// In crates/q_chat/src/commands/handler.rs

pub trait CommandHandler {
    // Existing methods
    fn to_command<'a>(&self, args: Vec<&'a str>) -> Result<Command>;
    
    fn execute<'a>(&self, args: Vec<&'a str>, ctx: &'a mut CommandContextAdapter<'a>, 
                 tool_uses: Option<Vec<QueuedTool>>, 
                 pending_tool_index: Option<usize>) -> Pin<Box<dyn Future<Output = Result<ChatState>> + 'a>> {
        Box::pin(async move {
            let command = self.to_command(args)?;
            Ok(ChatState::ExecuteCommand {
                command,
                tool_uses,
                pending_tool_index,
            })
        })
    }
    
    // New method that works directly with Command objects
    fn execute_command<'a>(&'a self, 
                         command: &'a Command, 
                         ctx: &'a mut CommandContextAdapter<'a>,
                         tool_uses: Option<Vec<QueuedTool>>,
                         pending_tool_index: Option<usize>) -> Pin<Box<dyn Future<Output = Result<ChatState>> + 'a>> {
        // Default implementation that returns an error for unexpected command types
        Box::pin(async move {
            Err(anyhow!("Unexpected command type for this handler"))
        })
    }
    
    // Other methods like llm_description(), etc.
}
```

### 2. Enhance Command Enum

```rust
// In crates/q_chat/src/command.rs

use std::collections::HashMap;
use std::sync::OnceLock;

impl Command {
    // Static HashMap for command name to handler mapping
    fn command_handlers() -> &'static HashMap<&'static str, &'static dyn CommandHandler> {
        static HANDLERS: OnceLock<HashMap<&'static str, &'static dyn CommandHandler>> = OnceLock::new();
        HANDLERS.get_or_init(|| {
            let mut map = HashMap::new();
            map.insert("help", &HELP_HANDLER as &dyn CommandHandler);
            map.insert("quit", &QUIT_HANDLER as &dyn CommandHandler);
            map.insert("clear", &CLEAR_HANDLER as &dyn CommandHandler);
            map.insert("context", &CONTEXT_HANDLER as &dyn CommandHandler);
            map.insert("profile", &PROFILE_HANDLER as &dyn CommandHandler);
            map.insert("tools", &TOOLS_HANDLER as &dyn CommandHandler);
            map.insert("issue", &ISSUE_HANDLER as &dyn CommandHandler);
            map.insert("compact", &COMPACT_HANDLER as &dyn CommandHandler);
            map.insert("editor", &EDITOR_HANDLER as &dyn CommandHandler);
            map.insert("usage", &USAGE_HANDLER as &dyn CommandHandler);
            map
        })
    }

    // Parse a command string into a Command enum
    pub fn parse(command_str: &str) -> Result<Self> {
        // Skip the leading slash if present
        let command_str = command_str.trim_start();
        let command_str = if command_str.starts_with('/') {
            &command_str[1..]
        } else {
            command_str
        };
        
        // Split into command and arguments
        let mut parts = command_str.split_whitespace();
        let command_name = parts.next().ok_or_else(|| anyhow!("Empty command"))?;
        let args: Vec<&str> = parts.collect();
        
        // Look up handler in the static HashMap
        let handler = Self::command_handlers().get(command_name)
            .ok_or_else(|| anyhow!("Unknown command: {}", command_name))?;
        
        // Use the handler to create the command
        handler.to_command(args)
    }
    
    // Execute the command directly
    pub async fn execute<'a>(
        &'a self,
        ctx: &'a mut CommandContextAdapter<'a>,
        tool_uses: Option<Vec<QueuedTool>>,
        pending_tool_index: Option<usize>,
    ) -> Result<ChatState> {
        // Get the appropriate handler and execute the command
        let handler = self.to_handler();
        handler.execute_command(self, ctx, tool_uses, pending_tool_index).await
    }
    
    // to_args is only for display purposes
    pub fn to_args(&self) -> Vec<String> {
        match self {
            Command::Help { help_text } => {
                let mut args = vec!["help".to_string()];
                if let Some(text) = help_text {
                    args.push(text.clone());
                }
                args
            },
            Command::Quit => vec!["quit".to_string()],
            Command::Clear => vec!["clear".to_string()],
            // Implement for other commands...
            _ => vec![],
        }
    }
    
    // Generate LLM descriptions for all commands
    pub fn generate_llm_descriptions() -> serde_json::Value {
        let mut descriptions = json!({});
        
        // Use the same static HashMap to generate descriptions
        for (command_name, handler) in Self::command_handlers().iter() {
            descriptions[command_name] = handler.llm_description();
        }
        
        descriptions
    }
}
```

### 3. Update Command Handlers

```rust
// In crates/q_chat/src/commands/help.rs

impl CommandHandler for HelpCommandHandler {
    // Existing to_command implementation
    
    // Override execute_command to handle Help variant
    fn execute_command<'a>(&'a self, 
                         command: &'a Command, 
                         ctx: &'a mut CommandContextAdapter<'a>,
                         tool_uses: Option<Vec<QueuedTool>>,
                         pending_tool_index: Option<usize>) -> Pin<Box<dyn Future<Output = Result<ChatState>> + 'a>> {
        Box::pin(async move {
            // Check if it's a Help command
            if let Command::Help { help_text } = command {
                // Implementation of help command using help_text directly
                // ...
                Ok(ChatState::Continue)
            } else {
                // Return error for unexpected command types
                Err(anyhow!("HelpCommandHandler can only execute Help commands"))
            }
        })
    }
}
```

### 4. Update Integration Points

```rust
// In crates/q_chat/src/tools/internal_command/tool.rs

impl Tool for InternalCommand {
    async fn invoke(&self, context: &Context, output: &mut impl Write) -> Result<InvokeOutput> {
        // Parse the command string into a Command enum directly
        let command = Command::parse(&format!("{} {}", self.command, self.args.join(" ")))?;
        
        // Create a CommandResult with the parsed Command
        let result = CommandResult::new(command);
        
        // Return a serialized version of the CommandResult
        let result_json = serde_json::to_string(&result)?;
        Ok(InvokeOutput::new(result_json))
    }
}

// In crates/q_chat/src/lib.rs or wherever the chat loop is implemented

// Replace CommandRegistry::global().parse_and_execute with Command::parse and execute
async fn handle_input(&mut self, input: &str) -> Result<ChatState> {
    if input.trim_start().starts_with('/') {
        // It's a command
        let command = Command::parse(input)?;
        command.execute(&mut self.command_context_adapter(), None, None).await
    } else {
        // It's a regular message
        // Existing code...
    }
}

// Also update any other places that use CommandRegistry
async fn process_tool_response(&mut self, response: InvokeOutput) -> Result<ChatState> {
    // Try to parse the response as a CommandResult
    if let Ok(command_result) = serde_json::from_str::<CommandResult>(&response.content) {
        // Execute the command directly
        return command_result.command.execute(
            &mut self.command_context_adapter(),
            None,
            None
        ).await;
    }
    
    // If it's not a CommandResult, handle it as a regular tool response
    // (existing code)
    
    Ok(ChatState::Continue)
}
```

### 5. Update LLM Integration

```rust
// In crates/q_chat/src/tools/internal_command/schema.rs or wherever LLM descriptions are generated

fn generate_command_descriptions() -> serde_json::Value {
    // Replace CommandRegistry::global().generate_llm_descriptions() with Command::generate_llm_descriptions()
    Command::generate_llm_descriptions()
}
```

### 6. Remove CommandRegistry

After making these changes, we can remove the CommandRegistry class entirely:

```rust
// Remove crates/q_chat/src/commands/registry.rs or comment it out if you want to keep it for reference
```

## Implementation Steps

1. Update the CommandHandler trait with the new execute_command method
2. Enhance the Command enum with static methods
3. Update each command handler to implement execute_command
4. Update integration points to use Command directly
5. Remove the CommandRegistry class
6. Run tests to ensure everything works correctly
7. Run clippy to check for any issues
8. Format the code with cargo fmt
9. Commit the changes

## Testing Plan

1. **Unit Tests**:
   - Test Command::parse with various inputs
   - Test Command::execute with different command types
   - Test Command::generate_llm_descriptions

2. **Integration Tests**:
   - Test command execution through the chat loop
   - Test command execution through the internal_command tool
   - Test error handling for invalid commands

3. **Edge Cases**:
   - Test with empty commands
   - Test with unknown commands
   - Test with malformed commands

## Benefits

1. **Simplified Architecture**: Removes the need for a separate CommandRegistry class
2. **Reduced Indirection**: Direct access to commands without going through a registry
3. **Type Safety**: Each handler works directly with its specific Command variant
4. **Maintainability**: Adding a new command only requires updating the Command enum and adding a handler
5. **Consistency**: Commands behave the same whether invoked directly or through the tool

## Commit Message

```
refactor(commands): Remove CommandRegistry in favor of command-centric architecture

Replace the CommandRegistry with direct Command enum functionality:
- Add static methods to Command enum for parsing and LLM descriptions
- Add execute_command method to CommandHandler trait
- Update all handlers to work directly with Command objects
- Remove CommandRegistry class entirely

This change simplifies the architecture, reduces indirection, and
improves type safety by leveraging the bidirectional relationship
between Commands and Handlers.

🤖 Assisted by [Amazon Q Developer](https://aws.amazon.com/q/developer)
```

🤖 Assisted by [Amazon Q Developer](https://aws.amazon.com/q/developer)
