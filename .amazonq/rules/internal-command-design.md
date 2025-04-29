# Internal Command Tool Design Principles

## Command-Centric Implementation

The `internal_command` tool now follows a command-centric approach by leveraging the `Command` enum for all command-related functionality:

1. **Command Validation**: Use the `Command` enum to validate commands rather than hardcoding command names
2. **Command Descriptions**: Get command descriptions from the respective `CommandHandler` instances via the Command enum
3. **Command Parsing**: Use the handlers' parsing logic through the bidirectional relationship
4. **Command Execution**: Delegate to the handlers for execution through the Command enum

## Implementation Guidelines

- Avoid hardcoded command strings or match statements that enumerate all commands
- Use the bidirectional relationship between Commands and Handlers:
  - `handler.to_command(args)` to convert arguments to Command enums
  - `command.to_handler()` to get the appropriate handler for a Command
- ✅ **Implemented**: Added `to_command()` method to `CommandHandler` trait to convert arguments to Command enums
- ✅ **Implemented**: Added `to_handler()` method to `Command` enum to get the appropriate handler for a Command

## Benefits

- Command-centric architecture with bidirectional relationships
- Automatic support for new commands without modifying the `internal_command` tool
- Consistent behavior between direct command execution and tool-based execution
- Reduced maintenance burden and code duplication
- Simplified command addition process

## Bidirectional Relationship

The bidirectional relationship between Commands and Handlers has been successfully implemented:

### CommandHandler to Command (`to_command`)

```rust
fn to_command<'a>(&self, args: Vec<&'a str>) -> Result<Command>;
```

This method:
- Parses command arguments into the appropriate Command enum variant
- Separates parsing logic from execution logic
- Makes the command system more type-safe
- Provides a foundation for future architecture refinement

### Command to CommandHandler (`to_handler`)

```rust
fn to_handler(&self) -> &'static dyn CommandHandler;
```

This method:
- Returns the appropriate CommandHandler for a given Command variant
- Creates a bidirectional relationship between Commands and Handlers
- Shifts from a registry-based approach to a command-centric approach
- Reduces dependency on the CommandRegistry

The `execute` method now delegates to `to_command`:

```rust
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
```

All command handlers and command variants have been updated to implement their respective methods, ensuring consistent behavior between direct command execution and tool-based execution.
