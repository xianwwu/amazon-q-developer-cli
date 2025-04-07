- Feature Name: use_q_command_tool
- Start Date: 2025-03-28

# Summary

[summary]: #summary

This RFC proposes adding a new tool called `use_q_command` to the Amazon Q Developer CLI that will enable the AI assistant to directly execute internal commands within the q chat system. This will improve user experience by handling vague or incorrectly typed requests more gracefully and providing more direct assistance with command execution.

# Motivation

[motivation]: #motivation

Currently, when users make vague requests or use incorrect syntax (e.g., typing "Bye" instead of "/quit"), the system responds with suggestions like "You can quit the application by typing /quit" but doesn't take action. This creates friction in the user experience as users must:

1. Read the suggestion
2. Manually type the correct command
3. Wait for execution

Additionally, users may not be familiar with all available internal commands, their syntax, or their capabilities, leading to frustration and reduced productivity.

# Guide-level explanation

[guide-level-explanation]: #guide-level-explanation

The `use_q_command` tool allows the AI assistant to directly execute internal commands within the q chat system on behalf of the user. This creates a more natural and fluid interaction model where users can express their intent in natural language, and the AI can take appropriate action.

For example, instead of this interaction:

```
User: Bye
AI: You can quit the application by typing /quit
User: /quit
[Application exits]
```

The user would experience:

```
User: Bye
AI: I'll help you exit the application.
[AI executes /quit command]
[Application exits]
```

The tool supports various categories of internal commands:

1. **Slashcommands** - Direct execution of slash commands like `/quit`, `/clear`, `/help`, etc.
2. **Context Management** - Operations on conversation history like querying, pruning, or summarizing
3. **Tools Management** - Listing, enabling, disabling, or installing tools
4. **Settings Management** - Viewing or changing settings
5. **Controls** - Read-only access to system state

This feature makes the Amazon Q Developer CLI more intuitive and responsive to user needs, reducing the learning curve and improving overall productivity.

# Reference-level explanation

[reference-level-explanation]: #reference-level-explanation

## Tool Interface

The `use_q_command` tool will be implemented as part of the existing tools framework in the `q_cli` crate. It will have the following interface:

```rust
pub struct UseQCommand {
    /// The command to execute (e.g., "quit", "context", "settings")
    pub command: String,
    
    /// Optional subcommand (e.g., "list", "add", "remove")
    pub subcommand: Option<String>,
    
    /// Optional arguments for the command
    pub args: Option<Vec<String>>,
    
    /// Optional flags for the command
    pub flags: Option<HashMap<String, String>>,
}

pub struct UseQCommandResponse {
    /// Whether the command was executed successfully
    pub success: bool,
    
    /// Output from the command execution
    pub output: Option<String>,
    
    /// Error message if the command failed
    pub error: Option<String>,
}
```

### Integration with Tool Manager Pattern

The `use_q_command` tool will be integrated with the existing Tool enum in the toolmanager:

```rust
// Add to the Tool enum in tools/mod.rs
pub enum Tool {
    FsRead(FsRead),
    FsWrite(FsWrite),
    ExecuteBash(ExecuteBash),
    UseAws(UseAws),
    GhIssue(GhIssue),
    UseQCommand(UseQCommand), // New variant
}
```

The tool will implement all required trait methods:

```rust
impl Tool {
    pub fn display_name(&self) -> &'static str {
        match self {
            // ...existing cases...
            Tool::UseQCommand(_) => "Execute Q command",
        }
    }
    
    pub fn display_name_action(&self) -> String {
        match self {
            // ...existing cases...
            Tool::UseQCommand(use_q_command) => format!("Executing command `{}`", use_q_command.command),
        }
        .to_owned()
    }
    
    pub fn requires_acceptance(&self, ctx: &Context) -> bool {
        match self {
            // ...existing cases...
            Tool::UseQCommand(use_q_command) => use_q_command.requires_acceptance(ctx),
        }
    }
    
    // Similar implementations for invoke, queue_description, and validate
}
```

The tool will be convertible from a generic `ToolUse` instance:

```rust
impl TryFrom<ToolUse> for Tool {
    type Error = ToolResult;

    fn try_from(value: ToolUse) -> std::result::Result<Self, Self::Error> {
        let map_err = |parse_error| /* existing error mapping */;

        Ok(match value.name.as_str() {
            // ...existing cases...
            "use_q_command" => Self::UseQCommand(
                serde_json::from_value::<UseQCommand>(value.args).map_err(map_err)?
            ),
            // ...
        })
    }
}
```

The tool will also integrate with the `ToolPermissions` system:

```rust
impl UseQCommand {
    pub fn requires_acceptance(&self, ctx: &Context) -> bool {
        // Logic to determine if this command requires user acceptance
        match self.command.as_str() {
            "quit" | "clear" => true,  // Destructive/session-ending commands
            "help" | "context show" => false,  // Informational commands
            _ => true,  // Default to requiring acceptance
        }
    }
}

// In ToolPermissions::default_permission_label
fn default_permission_label(tool_name: &str) -> String {
    let label = match tool_name {
        // ...existing cases...
        "use_q_command" => "Per-request",
        _ => "Per-request",
    };

    format!("{label} [Default] ")
}
```

## Implementation Details

The tool will be implemented in the `q_cli` crate under `src/cli/chat/tools/use_q_command/`. The implementation will:

1. Parse the incoming request into the appropriate internal command format
2. Validate the command and arguments
3. Execute the command using the command registry infrastructure
4. Capture the output/results
5. Return the results to the AI assistant

### Project Structure Changes

To improve organization and maintainability, we will restructure the command-related code:

```
src/cli/chat/
├── commands/           # New directory for all command-related code
│   ├── mod.rs          # Exports the CommandRegistry and CommandHandler trait
│   ├── registry.rs     # CommandRegistry implementation
│   ├── handler.rs      # CommandHandler trait definition
│   ├── result.rs       # CommandResult enum definition
│   ├── quit.rs         # QuitCommand implementation
│   ├── clear.rs        # ClearCommand implementation
│   ├── help.rs         # HelpCommand implementation
│   ├── context/        # Context command and subcommands
│   ├── profile/        # Profile command and subcommands
│   └── tools/          # Tools command and subcommands
├── tools/              # Existing directory for tools
│   ├── mod.rs
│   ├── execute_bash.rs
│   ├── fs_read.rs
│   ├── fs_write.rs
│   ├── gh_issue.rs
│   ├── use_aws.rs
│   └── use_q_command/  # New tool that uses the command registry
└── mod.rs
```

This structure parallels the existing `tools/` directory, creating a clear separation between tools (which are used by the AI) and commands (which are used by both users and the AI via the `use_q_command` tool).

### Command Registry Pattern

To improve maintainability and reduce the reliance on match statements, we will introduce a new command registry pattern:

```rust
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
        
        registry
    }
    
    /// Register a new command handler
    pub fn register(&mut self, name: &str, handler: Box<dyn CommandHandler>) {
        self.commands.insert(name.to_string(), handler);
    }
    
    /// Get a command handler by name
    pub fn get(&self, name: &str) -> Option<&dyn CommandHandler> {
        self.commands.get(name).map(|h| h.as_ref())
    }
    
    /// Parse and execute a command string
    pub fn parse_and_execute(&self, input: &str, ctx: &Context) -> Result<CommandResult> {
        let (name, args) = self.parse_command(input)?;
        
        if let Some(handler) = self.get(name) {
            handler.execute(args, ctx)
        } else {
            // If not a registered command, treat as a question to the AI
            Ok(CommandResult::Question(input.to_string()))
        }
    }
}

/// Trait for command handlers
pub trait CommandHandler: Send + Sync {
    /// Returns the name of the command
    fn name(&self) -> &'static str;
    
    /// Returns a description of the command
    fn description(&self) -> &'static str;
    
    /// Returns usage information for the command
    fn usage(&self) -> &'static str;
    
    /// Returns detailed help text for the command
    fn help(&self) -> String;
    
    /// Execute the command with the given arguments
    fn execute(&self, args: Vec<&str>, ctx: &Context) -> Result<CommandResult>;
    
    /// Check if this command requires confirmation before execution
    fn requires_confirmation(&self, args: &[&str]) -> bool {
        false // Most commands don't require confirmation by default
    }
}

/// Result of executing a command
pub enum CommandResult {
    /// Command executed successfully with no output
    Success,
    
    /// Command executed successfully with text output
    Output(String),
    
    /// Command requires user confirmation before proceeding
    NeedsConfirmation {
        /// Message to display to the user
        message: String,
        
        /// Callback to execute if confirmed
        callback: Box<dyn FnOnce() -> Result<CommandResult> + Send>,
    },
    
    /// Command was not a command but a question to the AI
    Question(String),
    
    /// Command execution failed
    Error(String),
}
```

Example implementation of a command:

```rust
/// Handler for the quit command
pub struct QuitCommand;

impl QuitCommand {
    pub fn new() -> Self {
        Self
    }
}

impl CommandHandler for QuitCommand {
    fn name(&self) -> &'static str {
        "quit"
    }
    
    fn description(&self) -> &'static str {
        "Exit the application"
    }
    
    fn usage(&self) -> &'static str {
        "/quit"
    }
    
    fn help(&self) -> String {
        "Exits the Amazon Q CLI application.".to_string()
    }
    
    fn execute(&self, _args: Vec<&str>, _ctx: &Context) -> Result<CommandResult> {
        // Implementation to exit the application
        std::process::exit(0);
    }
    
    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        true // Quitting should require confirmation
    }
}
```

Integration with the `UseQCommand` tool:

```rust
impl UseQCommand {
    pub async fn invoke(&self, context: &Context, updates: &mut impl Write) -> Result<InvokeOutput> {
        // Get the command registry
        let registry = CommandRegistry::new();
        
        // Parse the command string
        let cmd_str = if !self.command.starts_with('/') {
            format!("/{}", self.command)
        } else {
            self.command.clone()
        };
        
        // Execute the command
        match registry.parse_and_execute(&cmd_str, context) {
            Ok(CommandResult::Success) => {
                Ok(InvokeOutput {
                    output: OutputKind::Text("Command executed successfully".to_string()),
                })
            },
            Ok(CommandResult::Output(output)) => {
                Ok(InvokeOutput {
                    output: OutputKind::Text(output),
                })
            },
            // Handle other result types...
            _ => {
                Ok(InvokeOutput {
                    output: OutputKind::Text("Error executing command".to_string()),
                })
            }
        }
    }
}
```

## Command Categories

The tool will support the following categories of internal commands:

1. **Slashcommands**
   - `/quit` - Quit the application
   - `/clear` - Clear the conversation history
   - `/help` - Show the help dialogue
   - `/profile` - Manage profiles (with subcommands: help, list, set, create, delete, rename)
   - `/context` - Manage context files (with subcommands: help, show, add, rm, clear)

2. **Context Management**
   - `context query` - Search through conversation history
   - `context prune` - Remove specific portions of the conversation history
   - `context rollback` - Revert to a previous point in the conversation
   - `context summarize` - Generate a summary of the conversation or portions of it
   - `context export` - Export conversation history to a file
   - `context import` - Import conversation history from a file

3. **Tools Management**
   - `tools list` - List available tools
   - `tools enable` - Enable a tool
   - `tools disable` - Disable a tool
   - `tools install` - Install MCP-compatible tools
   - `tools uninstall` - Uninstall MCP-compatible tools
   - `tools update` - Update MCP-compatible tools
   - `tools info` - Show information about installed tools

4. **Settings Management**
   - `settings list` - List current settings
   - `settings set` - Change a setting
   - `settings reset` - Reset settings to default

5. **Controls**
   - Read-only access to system state
   - Check if acceptall mode is enabled
   - Check if `--non-interactive` mode is active
   - View current conversation mode
   - Access other runtime configuration information

The Tools Management category will include support for Model Context Protocol (MCP) tools (https://modelcontextprotocol.io/introduction), allowing users to extend the functionality of Amazon Q Developer CLI with third-party tools that follow the MCP specification.

## Security Considerations

To ensure security:

1. The tool will only execute predefined internal commands
2. File system access will be limited to the same permissions as the user
3. Potentially destructive operations will require confirmation
4. Command execution will be logged for audit purposes

## Implementation Plan

### Phase 1: Core Implementation

1. Create the new `commands/` directory structure alongside the existing `tools/` directory
2. Implement the `CommandRegistry`, `CommandHandler` trait, and `CommandResult` enum
3. Create the basic tool structure in `src/cli/chat/tools/use_q_command/`
4. Implement command parsing and validation
5. Add unit tests for basic functionality

### Phase 2: Command Handler Implementations

1. Implement handlers for simple commands (quit, clear, help)
2. Implement handlers for complex commands (context, profile, tools)
3. Add comprehensive tests for all command handlers
4. Create an adapter layer to allow the new registry to work alongside the existing system

### Phase 3: UseQCommand Integration

1. Implement the UseQCommand tool that uses the command registry
2. Add validation and permission handling
3. Implement execution for all command types
4. Add comprehensive tests for the tool

### Phase 4: Integration and Refinement

1. Integrate with the AI assistant's response generation
2. Add natural language understanding for command intent
3. Implement confirmation flows for potentially destructive operations
4. Add telemetry to track usage patterns

### Phase 5: Full Migration

1. Replace the existing match-based command parsing with the registry
2. Remove the adapter layer and legacy code
3. Update documentation and examples
4. Perform comprehensive testing of the new system

# Drawbacks

[drawbacks]: #drawbacks

There are several potential drawbacks to this feature:

1. **Security Risks**: Allowing the AI to execute commands directly could introduce security vulnerabilities if not properly constrained.

2. **User Confusion**: Users might not understand what actions the AI is taking on their behalf, leading to confusion or unexpected behavior.

3. **Implementation Complexity**: The feature requires careful integration with the existing command infrastructure and robust error handling.

4. **Maintenance Burden**: As new commands are added to the system, the `use_q_command` tool will need to be updated to support them.

5. **Potential for Misuse**: Users might become overly reliant on the AI executing commands, reducing their understanding of the underlying system.

# Rationale and alternatives

[rationale-and-alternatives]: #rationale-and-alternatives

## Why this design?

This design provides a balance between flexibility and security:

1. It leverages the existing command infrastructure rather than creating a parallel system
2. It provides a structured interface for the AI to interact with the system
3. It maintains clear boundaries around what commands can be executed
4. It captures output and errors for proper feedback to the user
5. The command registry pattern aligns with the toolmanager pattern, providing consistency across the codebase
6. It reduces the reliance on match statements, making the code more maintainable and extensible

## Benefits of the Command Registry Pattern

The proposed command registry pattern offers several advantages over the current match-based approach:

1. **Extensibility**: New commands can be added without modifying existing code, following the Open/Closed Principle.
2. **Testability**: Each command handler can be tested in isolation, making tests more focused and maintainable.
3. **Discoverability**: The registry provides a central place to discover all available commands and their capabilities.
4. **Consistency**: All commands follow the same interface, ensuring consistent behavior and error handling.
5. **Reduced Duplication**: Common functionality like argument parsing and validation can be shared across commands.
6. **Dynamic Registration**: Commands can be registered at runtime, enabling plugin-like extensibility.
7. **Improved Maintainability**: Changes to one command don't affect others, reducing the risk of regressions.
8. **Better Documentation**: Each command's documentation is co-located with its implementation.

## Alternatives Considered

### Enhanced Command Suggestions

Instead of executing commands directly, enhance the suggestion system to provide more detailed guidance. This was rejected because it still requires manual user action.

### Custom Command Aliases

Implement a system of aliases for common commands. This was rejected because it doesn't address the core issue of natural language understanding.

### Guided Command Builder

Implement a step-by-step command builder UI. This was rejected due to increased complexity and potential disruption to the chat flow.

### Keep the Current Match-Based Command System

Continue using the current match-based approach for command parsing and execution. This was rejected because it doesn't scale well as more commands are added and makes the code harder to maintain.

## Impact of Not Doing This

Without this feature:

1. Users will continue to experience friction when trying to use commands
2. The learning curve for new users will remain steeper
3. The AI assistant will appear less capable compared to competitors
4. User productivity will be limited by the need to manually execute commands
5. The codebase will continue to rely on large match statements, making it harder to maintain and extend

# Unresolved questions

[unresolved-questions]: #unresolved-questions

1. How should we handle ambiguous commands where the user's intent is unclear?
2. What level of confirmation should be required for potentially destructive operations?
3. How should we handle commands that require interactive input?
4. Should there be a way for users to disable this feature if they prefer to execute commands manually?
5. How will this feature interact with future enhancements to the command system?

# Future possibilities

[future-possibilities]: #future-possibilities

1. **Command Chaining**: Allow the AI to execute sequences of commands to accomplish more complex tasks.
2. **Custom Command Creation**: Enable users to define custom commands that the AI can execute.
3. **Contextual Command Suggestions**: Use conversation history to suggest relevant commands proactively.
4. **Cross-Session Command History**: Maintain a history of successful commands across sessions to improve future recommendations.
5. **Integration with External Tools**: Extend the command execution capability to interact with external tools and services.
6. **Natural Language Command Builder**: Develop a more sophisticated natural language understanding system to convert complex requests into command sequences.
7. **Command Explanation**: Add the ability for the AI to explain what a command does before executing it, enhancing user understanding.
8. **Command Undo**: Implement the ability to undo commands executed by the AI.
9. **Plugin System**: Extend the command registry to support third-party plugins that can register their own commands.
10. **Command Permissions**: Implement a more granular permission system for commands, allowing users to control which commands the AI can execute.
11. **Command Workflows**: Define sequences of commands as workflows that can be executed as a unit.
12. **Command Scheduling**: Allow commands to be scheduled for future execution.
13. **Command Templates**: Create parameterized command templates that can be filled in by the AI based on user intent.
14. **Command Analytics**: Track command usage patterns to improve the AI's ability to suggest and execute commands.
15. **Command Learning**: Use machine learning to improve command recognition and execution over time based on user feedback.
