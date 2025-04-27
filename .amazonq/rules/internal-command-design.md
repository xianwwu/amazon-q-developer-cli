# Internal Command Tool Design Principles

## DRY Implementation

The `internal_command` tool MUST follow the DRY (Don't Repeat Yourself) principle by leveraging the `CommandRegistry` for all command-related functionality:

1. **Command Validation**: Use `CommandRegistry` to validate commands rather than hardcoding command names
2. **Command Descriptions**: Get command descriptions from the respective `CommandHandler` instances
3. **Command Parsing**: Use the handlers' parsing logic rather than duplicating it
4. **Command Execution**: Delegate to the handlers for execution

## Implementation Guidelines

- Avoid hardcoded command strings or match statements that enumerate all commands
- Use `CommandRegistry::global().get(cmd)` to access command handlers
- Use handler methods like `description()`, `requires_confirmation()`, etc.
- Future enhancement: Add `to_command()` method to `CommandHandler` trait to convert arguments to Command enums

## Benefits

- Single source of truth for command behavior
- Automatic support for new commands without modifying the `internal_command` tool
- Consistent behavior between direct command execution and tool-based execution
- Reduced maintenance burden and code duplication
