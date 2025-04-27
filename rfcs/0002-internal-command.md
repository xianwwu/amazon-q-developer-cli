- Feature Name: internal_command_tool
- Start Date: 2025-03-28

# Summary

[summary]: #summary

This RFC proposes adding a new tool called `internal_command` to the Amazon Q Developer CLI that will enable the AI assistant to directly execute internal commands within the q chat system. This will improve user experience by handling vague or incorrectly typed requests more gracefully and providing more direct assistance with command execution.

# Motivation

[motivation]: #motivation

Currently, when users make vague requests or use incorrect syntax (e.g., typing "Bye" instead of "/quit"), the system responds with suggestions like "You can quit the application by typing /quit" but doesn't take action. This creates friction in the user experience as users must:

1. Read the suggestion
2. Manually type the correct command
3. Wait for execution

Additionally, users may not be familiar with all available internal commands, their syntax, or their capabilities, leading to frustration and reduced productivity.

# Guide-level explanation

[guide-level-explanation]: #guide-level-explanation

The `internal_command` tool allows the AI assistant to directly execute internal commands within the q chat system on behalf of the user. This creates a more natural and fluid interaction model where users can express their intent in natural language, and the AI can take appropriate action.

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

The `internal_command` tool will be implemented as part of the existing tools framework in the `q_chat` crate. It will have the following interface:

```rust
pub struct InternalCommand {
    /// The command to execute (e.g., "quit", "context", "settings")
    pub command: String,
    
    /// Optional subcommand (e.g., "list", "add", "remove")
    pub subcommand: Option<String>,
    
    /// Optional arguments for the command
    pub args: Option<Vec<String>>,
    
    /// Optional flags for the command
    pub flags: Option<HashMap<String, String>>,
}
```

## Implementation Details

The tool will be implemented in the `q_chat` crate under `src/tools/internal_command/`. The implementation will:

1. Parse the incoming request into the appropriate internal command format
2. Validate the command and arguments
3. Execute the command using the command registry infrastructure
4. Capture the output/results
5. Return the results to the AI assistant

### Project Structure Changes

To improve organization and maintainability, we will restructure the command-related code:

```
src/
├── commands/           # New directory for all command-related code
│   ├── mod.rs          # Exports the CommandRegistry and CommandHandler trait
│   ├── registry.rs     # CommandRegistry implementation
│   ├── handler.rs      # CommandHandler trait definition
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
│   └── internal_command/  # New tool that uses the command registry
└── mod.rs
```

This structure parallels the existing `tools/` directory, creating a clear separation between tools (which are used by the AI) and commands (which are used by both users and the AI via the `internal_command` tool).

### Command Registry Pattern

To improve maintainability and reduce the reliance on match statements, we will introduce a new command registry pattern that directly integrates with the existing `ChatState` enum. The registry will be implemented as a singleton to avoid redundant initialization:

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
    
    /// Get the global instance of the command registry
    pub fn global() -> &'static CommandRegistry {
        static INSTANCE: OnceCell<CommandRegistry> = OnceCell::new();
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
    pub fn command_exists(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }
    
    /// Get all command names
    pub fn command_names(&self) -> Vec<&String> {
        self.commands.keys().collect()
    }
    
    /// Parse and execute a command string
    pub fn parse_and_execute(
        &self, 
        input: &str, 
        ctx: &ChatContext,
        tool_uses: Option<Vec<QueuedTool>>,
        pending_tool_index: Option<usize>,
    ) -> Result<ChatState> {
        let (name, args) = self.parse_command_string(input)?;
        
        if let Some(handler) = self.get(name) {
            handler.execute(args, ctx, tool_uses, pending_tool_index)
        } else {
            // If not a registered command, treat as a question to the AI
            Ok(ChatState::HandleInput {
                input: input.to_string(),
                tool_uses,
                pending_tool_index,
            })
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
    
    /// Returns a detailed description with examples for LLM tool descriptions
    fn llm_description(&self) -> String {
        // Default implementation returns the regular help text
        self.help()
    }
    
    /// Execute the command with the given arguments
    fn execute(
        &self, 
        args: Vec<&str>, 
        ctx: &ChatContext,
        tool_uses: Option<Vec<QueuedTool>>,
        pending_tool_index: Option<usize>,
    ) -> Result<ChatState>;
    
    /// Check if this command requires confirmation before execution
    fn requires_confirmation(&self, args: &[&str]) -> bool {
        false // Most commands don't require confirmation by default
    }
    
    /// Convert arguments to a Command enum
    fn to_command(&self, args: Vec<&str>) -> Result<Command> {
        // This method allows each command handler to parse its arguments
        // and return the appropriate Command enum instance
        unimplemented!("Command handlers must implement to_command")
    }
    
    /// Parse arguments for this command
    fn parse_args<'a>(&self, args: Vec<&'a str>) -> Result<Vec<&'a str>> {
        Ok(args)
    }
}

/// Function to convert a Command enum to its corresponding CommandHandler
pub fn command_to_handler(command: &Command) -> Option<&dyn CommandHandler> {
    let registry = CommandRegistry::global();
    match command {
        Command::Quit => registry.get("quit"),
        Command::Clear => registry.get("clear"),
        Command::Help => registry.get("help"),
        Command::Context { .. } => registry.get("context"),
        Command::Profile { .. } => registry.get("profile"),
        Command::Tools { .. } => registry.get("tools"),
        Command::Compact { .. } => registry.get("compact"),
        // Handle other command types...
        _ => None,
    }
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
    
    fn execute(
        &self, 
        _args: Vec<&str>, 
        _ctx: &ChatContext,
        _tool_uses: Option<Vec<QueuedTool>>,
        _pending_tool_index: Option<usize>,
    ) -> Result<ChatState> {
        // Return Exit state directly
        Ok(ChatState::Exit)
    }
    
    fn requires_confirmation(&self, _args: &[&str]) -> bool {
        true // Quitting should require confirmation
    }
    
    fn to_command(&self, _args: Vec<&str>) -> Result<Command> {
        // Convert to Command::Quit
        Ok(Command::Quit)
    }
}
```

Integration with the `InternalCommand` tool:

```rust
impl Tool for InternalCommand {
    fn validate(&self, ctx: &Context) -> Result<(), ToolResult> {
        // Validate command exists and is allowed
        let registry = CommandRegistry::global();
        if !registry.command_exists(&self.command) {
            return Err(ToolResult::error(
                self.tool_use_id.clone(),
                format!("Unknown command: {}", self.command),
            ));
        }
        Ok(())
    }
    
    fn requires_acceptance(&self, _ctx: &Context) -> bool {
        // Get the command handler
        let cmd = self.command.trim_start_matches('/');
        if let Some(handler) = CommandRegistry::global().get(cmd) {
            // Convert args to string slices for the handler
            let args: Vec<&str> = match &self.subcommand {
                Some(subcommand) => vec![subcommand.as_str()],
                None => vec![],
            };
            
            return handler.requires_confirmation(&args);
        }
        
        // For commands not in the registry, default to requiring confirmation
        true
    }
    
    // Other trait implementations...
}

impl InternalCommand {
    pub async fn invoke(&self, context: &Context, updates: &mut impl Write) -> Result<InvokeOutput> {
        // Format the command string for execution
        let command_str = self.format_command_string();
        let description = self.get_command_description();

        // Create a response with the command and description
        let response = format!("Executing command for you: `{}` - {}", command_str, description);

        // Get the command name and arguments
        let cmd = self.command.trim_start_matches('/');
        let args: Vec<&str> = match (&self.subcommand, &self.args) {
            (Some(subcommand), Some(args)) => {
                let mut result = vec![subcommand.as_str()];
                result.extend(args.iter().map(|s| s.as_str()));
                result
            },
            (Some(subcommand), None) => vec![subcommand.as_str()],
            (None, Some(args)) => args.iter().map(|s| s.as_str()).collect(),
            (None, None) => vec![],
        };

        // Get the command handler and convert to Command enum
        let parsed_command = if let Some(handler) = CommandRegistry::global().get(cmd) {
            handler.to_command(args)?
        } else {
            // Special case handling for commands not in the registry
            match cmd {
                "issue" => {
                    let prompt = if let Some(args) = &self.args {
                        if !args.is_empty() { Some(args.join(" ")) } else { None }
                    } else {
                        None
                    };
                    Command::Issue { prompt }
                },
                "editor" => {
                    let initial_text = if let Some(args) = &self.args {
                        if !args.is_empty() { Some(args.join(" ")) } else { None }
                    } else {
                        None
                    };
                    Command::PromptEditor { initial_text }
                },
                "usage" => Command::Usage,
                _ => return Err(eyre::eyre!("Unknown command: {}", self.command)),
            }
        };

        // Log the parsed command
        debug!("Parsed command: {:?}", parsed_command);

        // Return an InvokeOutput with the response and next state
        Ok(InvokeOutput {
            output: crate::tools::OutputKind::Text(response),
            next_state: Some(ChatState::ExecuteCommand {
                command: parsed_command,
                tool_uses: None,
                pending_tool_index: None,
            }),
        })
    }
    
    pub fn get_usage_description(&self) -> String {
        let registry = CommandRegistry::global();
        let mut description = String::from("Execute internal commands within the q chat system.\n\n");
        description.push_str("Available commands:\n");
        
        for command_name in registry.command_names() {
            if let Some(handler) = registry.get(command_name) {
                description.push_str(&format!(
                    "- {} - {}\n  Usage: {}\n\n",
                    command_name,
                    handler.description(),
                    handler.usage()
                ));
            }
        }
        
        description
    }
}
```

## Enhanced Security Considerations

To ensure security when allowing AI to execute commands:

1. **Default to Requiring Confirmation**: All commands executed through `internal_command` will require user confirmation by default if they are mutative, or will automatically proceed if read-only.
2. **Permission Persistence**: Users can choose to trust specific commands using the existing permission system
3. **Command Auditing**: All commands executed by the AI will be logged for audit purposes
4. **Scope Limitation**: Commands will only have access to the same resources as when executed directly by the user
5. **Input Sanitization**: All command arguments will be sanitized to prevent injection attacks
6. **Execution Context**: Commands will run in the same security context as the application

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

1. Create the basic tool structure in `src/tools/internal_command/`
2. Implement command parsing and validation
3. Implement execution for session management commands
4. Add unit tests for basic functionality

### Phase 2: Extended Command Support

1. Implement context management commands
2. Implement settings management commands
3. Implement tool management commands
4. Add comprehensive tests for all command types

### Phase 3: Integration and Refinement

1. Integrate with the AI assistant's response generation
2. Add natural language understanding for command intent
3. Implement confirmation flows for potentially destructive operations
4. Add telemetry to track usage patterns

### Phase 4: Command Registry Enhancements

1. Add `to_command` method to the CommandHandler trait
2. Implement `to_command` for all command handlers
3. Add `command_to_handler` function to convert Command enums to handlers
4. Update the internal_command tool to use these new methods
5. Add tests to verify bidirectional conversion between Commands and Handlers

# Drawbacks

[drawbacks]: #drawbacks

There are several potential drawbacks to this feature:

1. **Security Risks**: Allowing the AI to execute commands directly could introduce security vulnerabilities if not properly constrained.

2. **User Confusion**: Users might not understand what actions the AI is taking on their behalf, leading to confusion or unexpected behavior.

3. **Implementation Complexity**: The feature requires careful integration with the existing command infrastructure and robust error handling.

4. **Maintenance Burden**: As new commands are added to the system, the `internal_command` tool will need to be updated to support them.

5. **Potential for Misuse**: Users might become overly reliant on the AI executing commands, reducing their understanding of the underlying system.

# Rationale and alternatives

[rationale-and-alternatives]: #rationale-and-alternatives

## Why this design?

This design provides a balance between flexibility and security:

1. It leverages the existing command infrastructure rather than creating a parallel system
2. It provides a structured interface for the AI to interact with the system
3. It maintains clear boundaries around what commands can be executed
4. It captures output and errors for proper feedback to the user
5. It establishes a bidirectional relationship between Command enums and CommandHandlers

## Alternatives Considered

### Enhanced Command Suggestions

Instead of executing commands directly, enhance the suggestion system to provide more detailed guidance. This was rejected because it still requires manual user action.

### Custom Command Aliases

Implement a system of aliases for common commands. This was rejected because it doesn't address the core issue of natural language understanding.

### Guided Command Builder

Implement a step-by-step command builder UI. This was rejected due to increased complexity and potential disruption to the chat flow.

### Separate Command Parsing Logic

Maintain separate command parsing logic in the internal_command tool. This was rejected because it would lead to duplication and potential inconsistencies.

## Impact of Not Doing This

Without this feature:

1. Users will continue to experience friction when trying to use commands
2. The learning curve for new users will remain steeper
3. The AI assistant will appear less capable compared to competitors
4. User productivity will be limited by the need to manually execute commands

# Unresolved questions

[unresolved-questions]: #unresolved-questions

1. How should we handle ambiguous commands where the user's intent is unclear?
2. What level of confirmation should be required for potentially destructive operations?
3. How should we handle commands that require interactive input?
4. Should there be a way for users to disable this feature if they prefer to execute commands manually?
5. How will this feature interact with future enhancements to the command system?
6. Should the Command enum and CommandRegistry be merged in a future iteration?

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
9. **Unified Command System**: Merge the Command enum and CommandRegistry to create a more cohesive command system where each command type is directly associated with its handler.
