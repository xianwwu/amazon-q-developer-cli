# Command Registry Replacement Plan

## Overview

This document outlines the plan for replacing the CommandRegistry with a command-centric architecture that leverages the bidirectional relationship between Commands and Handlers. The Command enum will become the central point for command-related functionality, making the code more maintainable and reducing indirection, while keeping implementation details in separate files.

## Key Changes

1. **Command Enum Enhancement**:
   - Keep the existing `to_handler()` method that returns the appropriate handler for each Command variant
   - Add `parse()` static method to parse command strings into Command enums
   - Add `execute()` method for direct command execution that delegates to the handler
   - Add `generate_llm_descriptions()` static method for LLM integration

2. **CommandHandler Trait Enhancement**:
   - Keep the existing `to_command()` method that converts arguments to Command enums
   - Add `execute_command()` method that works directly with Command objects
   - Each handler implements this method to handle its specific Command variant
   - Handlers return errors for unexpected command types

3. **Static Handler Instances**:
   - Define static instances of each handler in their respective files
   - Use these static instances in the Command enum's `to_handler()` method
   - Maintain the bidirectional relationship between Commands and Handlers

4. **CommandRegistry Removal**:
   - Replace all CommandRegistry calls with direct Command enum calls
   - Remove the CommandRegistry class entirely

## Implementation Details

### 1. Update CommandHandler Trait

```rust
// In crates/q_chat/src/commands/handler.rs

pub trait CommandHandler {
    // Existing methods
    fn to_command<'a>(&self, args: Vec<&'a str>) -> Result<Command>;
    
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

### 2. Define Static Handler Instances

```rust
// In crates/q_chat/src/commands/help.rs
pub static HELP_HANDLER: HelpCommandHandler = HelpCommandHandler;

// In crates/q_chat/src/commands/quit.rs
pub static QUIT_HANDLER: QuitCommandHandler = QuitCommandHandler;

// And so on for other commands...
```

### 3. Enhance Command Enum

```rust
// In crates/q_chat/src/command.rs

impl Command {
    // Get the appropriate handler for this command variant
    pub fn to_handler(&self) -> &'static dyn CommandHandler {
        match self {
            Command::Help { .. } => &HELP_HANDLER,
            Command::Quit => &QUIT_HANDLER,
            Command::Clear => &CLEAR_HANDLER,
            Command::Context { subcommand } => subcommand.to_handler(),
            Command::Profile { subcommand } => subcommand.to_handler(),
            Command::Tools { subcommand } => match subcommand {
                Some(sub) => sub.to_handler(),
                None => &TOOLS_LIST_HANDLER,
            },
            Command::Compact { .. } => &COMPACT_HANDLER,
            Command::Usage => &USAGE_HANDLER,
            // Other commands...
        }
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
        
        // Match on command name and use the handler to parse arguments
        match command_name {
            "help" => HELP_HANDLER.to_command(args),
            "quit" => QUIT_HANDLER.to_command(args),
            "clear" => CLEAR_HANDLER.to_command(args),
            "context" => CONTEXT_HANDLER.to_command(args),
            "profile" => PROFILE_HANDLER.to_command(args),
            "tools" => TOOLS_HANDLER.to_command(args),
            "compact" => COMPACT_HANDLER.to_command(args),
            "usage" => USAGE_HANDLER.to_command(args),
            // Other commands...
            _ => Err(anyhow!("Unknown command: {}", command_name)),
        }
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
    
    // Generate LLM descriptions for all commands
    pub fn generate_llm_descriptions() -> serde_json::Value {
        let mut descriptions = json!({});
        
        // Use the static handlers to generate descriptions
        descriptions["help"] = HELP_HANDLER.llm_description();
        descriptions["quit"] = QUIT_HANDLER.llm_description();
        descriptions["clear"] = CLEAR_HANDLER.llm_description();
        descriptions["context"] = CONTEXT_HANDLER.llm_description();
        descriptions["profile"] = PROFILE_HANDLER.llm_description();
        descriptions["tools"] = TOOLS_HANDLER.llm_description();
        descriptions["compact"] = COMPACT_HANDLER.llm_description();
        descriptions["usage"] = USAGE_HANDLER.llm_description();
        // Other commands...
        
        descriptions
    }
}
```

### 4. Update Subcommand Enums

```rust
// In crates/q_chat/src/commands/context/mod.rs
impl ContextSubcommand {
    pub fn to_handler(&self) -> &'static dyn CommandHandler {
        match self {
            ContextSubcommand::Add { .. } => &CONTEXT_ADD_HANDLER,
            ContextSubcommand::Remove { .. } => &CONTEXT_REMOVE_HANDLER,
            ContextSubcommand::Clear => &CONTEXT_CLEAR_HANDLER,
            ContextSubcommand::Show => &CONTEXT_SHOW_HANDLER,
            ContextSubcommand::Hooks => &CONTEXT_HOOKS_HANDLER,
        }
    }
}
```

### 5. Update Command Handlers

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

### 6. Update Integration Points

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

### 7. Update LLM Integration

```rust
// In crates/q_chat/src/tools/internal_command/schema.rs or wherever LLM descriptions are generated

fn generate_command_descriptions() -> serde_json::Value {
    // Replace CommandRegistry::global().generate_llm_descriptions() with Command::generate_llm_descriptions()
    Command::generate_llm_descriptions()
}
```

### 8. Remove CommandRegistry

After making these changes, we can remove the CommandRegistry class entirely:

```rust
// Remove crates/q_chat/src/commands/registry.rs or comment it out if you want to keep it for reference
```

## Implementation Steps

1. Update the CommandHandler trait with the new execute_command method
2. Define static handler instances in each command file
3. Enhance the Command enum with static methods
4. Update each command handler to implement execute_command
5. Update integration points to use Command directly
6. Remove the CommandRegistry class
7. Run tests to ensure everything works correctly
8. Run clippy to check for any issues
9. Format the code with cargo fmt
10. Commit the changes

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

## Benefits of This Approach

1. **Code Organization**: Maintains separation of concerns with each command in its own file
2. **Type Safety**: Preserves the static typing between Command variants and their handlers
3. **Bidirectional Relationship**: Maintains the bidirectional relationship between Commands and Handlers
4. **Reduced Indirection**: Eliminates the need for a separate CommandRegistry
5. **Maintainability**: Makes it easier to add new commands by following a consistent pattern
6. **Consistency**: Ensures consistent behavior between direct command execution and tool-based execution

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
