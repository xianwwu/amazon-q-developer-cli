# Command Registry Migration Plan

This document outlines the plan for migrating all commands to the new CommandRegistry system, ensuring consistent behavior whether commands are invoked directly or through the `internal_command` tool.

## Migration Goals

1. Ensure all commands behave identically regardless of invocation method
2. Improve code maintainability by centralizing command logic
3. Enhance testability with a consistent command interface
4. Provide a foundation for future natural language command understanding

## Implementation Strategy

After evaluating various options, we've selected a Command Result approach that leverages the existing `Command` enum:

1. The `internal_command` tool will parse input parameters into the existing `Command` enum structure
2. The tool will return a `CommandResult` containing the parsed command
3. The chat loop will extract the command from the result and execute it using existing command execution logic

This approach minimizes changes to the codebase while ensuring consistent behavior between direct command execution and tool-based execution.

### CommandResult Structure

```rust
/// Result of a command execution from the internal_command tool
#[derive(Debug, Serialize, Deserialize)]
pub struct CommandResult {
    /// The command to execute
    pub command: Command,
}

impl CommandResult {
    /// Create a new command result with the given command
    pub fn new(command: Command) -> Self {
        Self { command }
    }
}
```

### InternalCommand Tool Implementation

```rust
impl Tool for InternalCommand {
    async fn invoke(&self, context: &Context, output: &mut impl Write) -> Result<InvokeOutput> {
        // Parse the command string into a Command enum
        let command = parse_command(&self.command, &self.args)?;
        
        // Create a CommandResult with the parsed Command
        let result = CommandResult::new(command);
        
        // Return a serialized version of the CommandResult
        let result_json = serde_json::to_string(&result)?;
        Ok(InvokeOutput::new(result_json))
    }
}
```

### Chat Loop Integration

```rust
async fn process_tool_response(&mut self, response: InvokeOutput) -> Result<ChatState> {
    // Try to parse the response as a CommandResult
    if let Ok(command_result) = serde_json::from_str::<CommandResult>(&response.content) {
        // Execute the command using the existing command execution logic
        return self.execute_command(command_result.command).await;
    }
    
    // If it's not a CommandResult, handle it as a regular tool response
    // (existing code)
    
    Ok(ChatState::Continue)
}
```

## Migration Process

For each command, we will follow this process:

1. **Documentation**
   - Document current behavior and implementation
   - Create test cases that verify current behavior
   - Define expected behavior after migration

2. **Implementation**
   - Implement or update the command handler in the `commands/` directory
   - Update the command execution flow to use the CommandRegistry
   - Ensure proper argument parsing and validation

3. **Testing**
   - Test direct command execution
   - Test tool-based command execution
   - Verify identical behavior between both paths
   - Test edge cases and error handling

4. **Cleanup**
   - Remove the direct implementation once migration is complete
   - Update documentation to reflect the new implementation

## Command Migration Tracking

| Command | Subcommands | Handler Implemented | Tests Written | Migration Complete | Notes |
|---------|-------------|---------------------|---------------|-------------------|-------|
| help    | -           | ✅                  | ✅            | ✅                | First test case |
| quit    | -           | ✅                  | ✅            | ✅                | Requires confirmation |
| clear   | -           | ✅                  | ✅            | ✅                | - |
| context | add         | ✅                  | ✅            | ✅                | File operations |
|         | rm          | ✅                  | ✅            | ✅                | File operations |
|         | clear       | ✅                  | ✅            | ✅                | - |
|         | show        | ✅                  | ✅            | ✅                | - |
|         | hooks       | ✅                  | ✅            | ✅                | - |
| profile | list        | ✅                  | ✅            | 🟡                | In progress |
|         | create      | ✅                  | ✅            | 🟡                | In progress |
|         | delete      | ✅                  | ✅            | 🟡                | Requires confirmation |
|         | set         | ✅                  | ✅            | 🟡                | In progress |
|         | rename      | ✅                  | ✅            | 🟡                | In progress |
| tools   | list        | ✅                  | ✅            | 🟡                | In progress |
|         | trust       | ✅                  | ✅            | 🟡                | In progress |
|         | untrust     | ✅                  | ✅            | 🟡                | In progress |
|         | trustall    | ✅                  | ✅            | 🟡                | In progress |
|         | reset       | ✅                  | ✅            | 🟡                | In progress |
| issue   | -           | ✅                  | ✅            | ✅                | Using existing report_issue tool |
| compact | -           | ✅                  | ✅            | ✅                | Implemented with summarization support |
| editor  | -           | ✅                  | ✅            | ✅                | Implemented with external editor support |
| usage   | -           | ✅                  | ✅            | ✅                | Implemented with token statistics display |

## Migration Schedule

### Week 8
- Create migration documentation and tracking
- Migrate basic commands (help, quit, clear)
- Document process and results

### Week 9
- Migrate complex commands with existing handlers (context, profile, tools, issue)
- Update tracking document with progress

### Week 10
- Implement and migrate remaining commands (compact, editor)
- Run comprehensive test suite
- Create final migration report
- Create user-facing documentation for all commands in docs/commands/
- Update SUMMARY.md with links to command documentation

## Test Case Template

For each command, we will create test cases that cover:

1. **Basic Usage**: Standard command invocation
2. **Arguments**: Various argument combinations
3. **Error Handling**: Invalid arguments, missing resources
4. **Edge Cases**: Unusual inputs or states
5. **Confirmation**: For commands that require confirmation

## Documentation Template

For each migrated command, we will create documentation that includes:

```markdown
# Command Migration: [COMMAND_NAME]

## Before Migration

### Implementation
```rust
// Code snippet of the original implementation
```

### Behavior
- Description of the command's behavior
- Expected output
- Any special cases or edge conditions

## After Migration

### Implementation
```rust
// Code snippet of the new implementation
```

### Behavior
- Description of the command's behavior after migration
- Verification that output matches the original
- Any differences or improvements

## Test Results

| Test Case | Before | After | Match | Notes |
|-----------|--------|-------|-------|-------|
| Basic usage | [Result] | [Result] | ✅/❌ | |
| With arguments | [Result] | [Result] | ✅/❌ | |
| Edge cases | [Result] | [Result] | ✅/❌ | |

## Conclusion
Summary of the migration results and any follow-up tasks
```

## User Documentation

For each command, we will also create user-facing documentation in the `docs/commands/` directory:

```markdown
# [Command Name]

## Overview
Brief description of what the command does and its purpose.

## Command Details
- **Name**: `command_name`
- **Description**: Short description
- **Usage**: `/command [arguments]`
- **Requires Confirmation**: Yes/No

## Functionality
Detailed explanation of what the command does.

## Example Usage
```
/command argument
```

Output:
```
Expected output
```

## Related Commands
- `/related_command`: Brief description of relationship

## Use Cases
- Common use case 1
- Common use case 2

## Notes
Additional information and tips
```

## Success Metrics

We will consider the migration successful when:

1. All commands are implemented using the CommandRegistry system
2. All commands behave identically whether invoked directly or through the tool
3. All commands have comprehensive test coverage
4. All direct command implementations have been removed
5. Documentation is updated to reflect the new implementation
   - Each command has a dedicated documentation page in docs/commands/
   - SUMMARY.md includes links to all command documentation
   - Documentation follows a consistent format
   - Examples and use cases are included for each command
