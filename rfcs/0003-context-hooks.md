- Feature Name: `context_hooks`
- Start Date: 2025-04-02

# Summary

[summary]: #summary

Implement "context hooks" which allow users or MCP servers to pass context either at the start of a conversation or alongside each prompt in Amazon Q CLI. This feature enables automated context injection without requiring manual `/context` commands.

# Motivation

[motivation]: #motivation

Currently, users must manually add context to Amazon Q CLI conversations using the `/context` command. This approach has several limitations:

1. It requires explicit user action for each piece of context
2. It doesn't support automated workflows where context should be dynamically injected
3. It doesn't integrate well with external tools or systems that could provide valuable context
4. It creates friction when users want to maintain consistent context across multiple conversations

By implementing context hooks, we can address these limitations and enable more powerful use cases:

- Automatically include relevant project information at the start of conversations
- Dynamically inject git status, environment variables, or other contextual information with each prompt
- Allow IDE integrations to provide code context without manual steps
- Enable MCP servers to provide organization-specific context to guide responses
- Support workflow automation where context is programmatically determined

# Guide-level explanation

[guide-level-explanation]: #guide-level-explanation

## What are Context Hooks?

Context hooks are configurable functions that automatically provide relevant context to Amazon Q CLI conversations. They come in two varieties:

1. **Conversation Start Hooks**: Run once at the beginning of a conversation to provide initial context
2. **Per-Prompt Hooks**: Run alongside each prompt to provide up-to-date context

## Configuration

Context hooks are configured within the existing Amazon Q CLI configuration structure to maintain consistency and leverage the established profile system. The configuration is stored in:

1. **Global Configuration**: `~/.aws/amazonq/global_config.json` for system-wide hooks
2. **Profile Configuration**: `~/.aws/amazonq/profiles/<profile-name>/context.json` for profile-specific hooks

This approach ensures that hooks are properly integrated with the existing context management system and respect profile settings.

Example global configuration:

```json
{
  "context": {
    "hooks": {
      "conversation_start": [
        {
          "name": "project-info",
          "type": "script",
          "path": "~/.aws/amazonq/hooks/project-info.sh",
          "enabled": true
        },
        {
          "name": "company-guidelines",
          "type": "http",
          "url": "https://internal-server.example.com/context-api",
          "headers": {
            "Authorization": "Bearer ${CONTEXT_API_TOKEN}"
          },
          "enabled": true
        }
      ],
      "per_prompt": [
        {
          "name": "environment-vars",
          "type": "inline",
          "command": "env | grep AWS_",
          "enabled": false
        }
      ]
    }
  }
}
```

Example profile-specific configuration:

```json
{
  "hooks": {
    "conversation_start": [],
    "per_prompt": [
      {
        "name": "git-status",
        "type": "script",
        "path": "~/.aws/amazonq/hooks/git-status.sh",
        "enabled": true
      }
    ]
  }
}
```

### Simplified Command Syntax

For ease of use, hooks can also be configured using a simplified prefix notation in the command field:

```json
{
  "hooks": {
    "conversation_start": [
      {
        "name": "project-info",
        "command": "!~/.aws/amazonq/hooks/project-info.sh",
        "enabled": true
      },
      {
        "name": "company-guidelines",
        "command": "#context-api https://internal-server.example.com/context-api",
        "headers": {
          "Authorization": "Bearer ${CONTEXT_API_TOKEN}"
        },
        "enabled": true
      }
    ],
    "per_prompt": [
      {
        "name": "git-status",
        "command": "!git status --short",
        "enabled": true
      }
    ]
  }
}
```

Where:
- `!` prefix indicates a shell command
- `/` prefix indicates an internal command
- `#` prefix indicates a tool call

This simplified syntax makes it easier to read and write hook configurations, especially for common use cases.

## Hook Types

### Script Hooks

Script hooks execute a local script that outputs context information:

```bash
#!/bin/bash
# ~/.config/amazon-q/hooks/project-info.sh

echo "# Project Information"
echo "Project: $(basename $(pwd))"
echo "Language: $(cat .tool-versions 2>/dev/null || echo "unknown")"
echo "Framework: $([ -f package.json ] && echo "Node.js" || echo "unknown")"
```

### HTTP Hooks

HTTP hooks make a request to an API endpoint that returns context information:

```
GET https://internal-server.example.com/context-api
Authorization: Bearer abc123

Response:
# Company Guidelines
- Follow the AWS Well-Architected Framework
- Use infrastructure as code
- Implement least privilege access
```

### Inline Hooks

Inline hooks execute a shell command directly:

```
env | grep AWS_

# Output:
AWS_PROFILE=development
AWS_REGION=us-west-2
```

## Using Context Hooks

Once configured, context hooks run automatically:

1. When starting a new conversation with `q chat`, conversation start hooks run
2. When sending a prompt, per-prompt hooks run

Users can see which hooks are active with the `/hooks` command:

```
> /hooks
Active context hooks:
- project-info (conversation start)
- git-status (per prompt)
```

Users can temporarily disable hooks with the `/hooks disable` command:

```
> /hooks disable git-status
Disabled hook: git-status
```

And re-enable them with `/hooks enable`:

```
> /hooks enable git-status
Enabled hook: git-status
```

### Context Visibility

Users can view how hooks contribute to the context with the `/context show` command:

```
> /context show
===============================
Current context:

global:
    .amazonq/rules/**/*.md
    README.md
    AmazonQ.md

profile:
    default

Expanded files (2):
    /Users/user/project/README.md
    /Users/user/project/AmazonQ.md

Context hooks (2):
    project-info (conversation start)
    git-status (per prompt)
```

Additional context visibility commands include:

- `/context show --hooks` - Show only hook contributions
- `/context show --previous` - Show context from previous prompt
- `/context show --always` - Always show context after each prompt

This visibility helps users understand what context is being provided to the LLM, making the system more transparent and easier to debug.

# Reference-level explanation

[reference-level-explanation]: #reference-level-explanation

## Architecture

The context hooks system consists of several components:

1. **Hook Configuration Manager**: Loads and validates hook configurations
2. **Hook Executor**: Executes hooks and captures their output
3. **Context Manager Integration**: Integrates hook output with the existing context system
4. **Hook Registry**: Maintains the state of enabled/disabled hooks

### Hook Configuration Manager

The Hook Configuration Manager is responsible for loading hook configurations from the YAML file and validating them:

```rust
pub struct HookConfig {
    pub name: String,
    pub hook_type: HookType,
    pub enabled: bool,
    // Common fields
    pub timeout_ms: Option<u64>,
    pub max_output_size: Option<usize>,
    // Type-specific fields
    pub path: Option<String>,        // For script hooks
    pub command: Option<String>,     // For inline hooks
    pub url: Option<String>,         // For HTTP hooks
    pub headers: Option<HashMap<String, String>>, // For HTTP hooks
}

pub enum HookType {
    Script,
    Http,
    Inline,
}

pub struct HookConfigManager {
    config_path: PathBuf,
    conversation_start_hooks: Vec<HookConfig>,
    per_prompt_hooks: Vec<HookConfig>,
}

impl HookConfigManager {
    pub fn new() -> Result<Self> {
        // Find and load config file
        // Parse YAML into HookConfig structs
        // Validate configurations
    }
    
    pub fn get_conversation_start_hooks(&self) -> &[HookConfig] {
        &self.conversation_start_hooks
    }
    
    pub fn get_per_prompt_hooks(&self) -> &[HookConfig] {
        &self.per_prompt_hooks
    }
}
```

### Hook Executor

The Hook Executor runs hooks and captures their output:

```rust
pub struct HookExecutor {
    config_manager: Arc<HookConfigManager>,
    hook_registry: Arc<HookRegistry>,
}

impl HookExecutor {
    pub fn new(
        config_manager: Arc<HookConfigManager>,
        hook_registry: Arc<HookRegistry>,
    ) -> Self {
        Self {
            config_manager,
            hook_registry,
        }
    }
    
    pub async fn execute_conversation_start_hooks(&self) -> Result<Vec<ContextEntry>> {
        // Get enabled conversation start hooks
        // Execute each hook
        // Format output as context entries
    }
    
    pub async fn execute_per_prompt_hooks(&self) -> Result<Vec<ContextEntry>> {
        // Get enabled per-prompt hooks
        // Execute each hook
        // Format output as context entries
    }
    
    async fn execute_hook(&self, hook: &HookConfig) -> Result<String> {
        match hook.hook_type {
            HookType::Script => self.execute_script_hook(hook).await,
            HookType::Http => self.execute_http_hook(hook).await,
            HookType::Inline => self.execute_inline_hook(hook).await,
        }
    }
    
    async fn execute_script_hook(&self, hook: &HookConfig) -> Result<String> {
        // Execute script at hook.path
        // Capture stdout
        // Handle errors and timeouts
    }
    
    async fn execute_http_hook(&self, hook: &HookConfig) -> Result<String> {
        // Make HTTP request to hook.url with headers
        // Capture response body
        // Handle errors and timeouts
    }
    
    async fn execute_inline_hook(&self, hook: &HookConfig) -> Result<String> {
        // Execute hook.command in shell
        // Capture stdout
        // Handle errors and timeouts
    }
}
```

### Context Manager Integration

The Context Manager Integration connects hook output to the existing context system:

```rust
pub struct ContextManagerIntegration {
    hook_executor: Arc<HookExecutor>,
    context_manager: Arc<ContextManager>,
}

impl ContextManagerIntegration {
    pub fn new(
        hook_executor: Arc<HookExecutor>,
        context_manager: Arc<ContextManager>,
    ) -> Self {
        Self {
            hook_executor,
            context_manager,
        }
    }
    
    pub async fn apply_conversation_start_hooks(&self) -> Result<()> {
        let context_entries = self.hook_executor.execute_conversation_start_hooks().await?;
        for entry in context_entries {
            self.context_manager.add_context(entry)?;
        }
        Ok(())
    }
    
    pub async fn apply_per_prompt_hooks(&self) -> Result<()> {
        let context_entries = self.hook_executor.execute_per_prompt_hooks().await?;
        for entry in context_entries {
            self.context_manager.add_context(entry)?;
        }
        Ok(())
    }
}
```

### Hook Registry

The Hook Registry maintains the state of enabled/disabled hooks:

```rust
pub struct HookRegistry {
    disabled_hooks: RwLock<HashSet<String>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self {
            disabled_hooks: RwLock::new(HashSet::new()),
        }
    }
    
    pub fn disable_hook(&self, name: &str) -> Result<()> {
        let mut disabled_hooks = self.disabled_hooks.write().unwrap();
        disabled_hooks.insert(name.to_string());
        Ok(())
    }
    
    pub fn enable_hook(&self, name: &str) -> Result<()> {
        let mut disabled_hooks = self.disabled_hooks.write().unwrap();
        disabled_hooks.remove(name);
        Ok(())
    }
    
    pub fn is_hook_enabled(&self, name: &str) -> bool {
        let disabled_hooks = self.disabled_hooks.read().unwrap();
        !disabled_hooks.contains(name)
    }
}
```

## Integration with Chat Command

The context hooks system integrates with the existing chat command:

```rust
impl ChatCommand {
    pub async fn run(&self) -> Result<()> {
        // Initialize components
        let config_manager = Arc::new(HookConfigManager::new()?);
        let hook_registry = Arc::new(HookRegistry::new());
        let hook_executor = Arc::new(HookExecutor::new(config_manager.clone(), hook_registry.clone()));
        let context_integration = ContextManagerIntegration::new(hook_executor.clone(), self.context_manager.clone());
        
        // Apply conversation start hooks
        context_integration.apply_conversation_start_hooks().await?;
        
        // Start chat loop
        loop {
            // Get user input
            let input = self.get_user_input().await?;
            
            // Check for hook commands
            if input.starts_with("/hooks") {
                self.handle_hook_command(input, &hook_registry).await?;
                continue;
            }
            
            // Apply per-prompt hooks before sending prompt
            context_integration.apply_per_prompt_hooks().await?;
            
            // Send prompt to LLM
            // ...
        }
    }
    
    async fn handle_hook_command(&self, input: String, hook_registry: &Arc<HookRegistry>) -> Result<()> {
        // Parse command: /hooks, /hooks disable <name>, /hooks enable <name>
        // Call appropriate hook_registry methods
        // Display feedback to user
    }
}
```

## Security Considerations

1. **Script Execution**: Scripts run with the same permissions as the Amazon Q CLI process
2. **HTTP Requests**: Sensitive information in headers (like tokens) should be stored securely
3. **Output Validation**: Hook output should be validated to prevent injection attacks
4. **Resource Limits**: Hooks should have timeouts and output size limits

## Performance Considerations

Context hooks can impact performance in several ways:

1. **Startup Time**: Conversation start hooks run before the first prompt, potentially increasing startup time
2. **Prompt Latency**: Per-prompt hooks run before each prompt, potentially increasing response time
3. **Background Execution**: To mitigate performance impact, hooks can run in the background:
   - Conversation start hooks run asynchronously during chat initialization
   - Per-prompt hooks run in parallel with a configurable timeout
4. **Caching**: Hook outputs can be cached with configurable TTL to avoid redundant executions

## MCP Integration

The context hooks system addresses several limitations with the current MCP protocol:

1. **Dynamic Context**: MCP servers can now provide context dynamically through HTTP hooks
2. **Per-Prompt Updates**: Context can be updated with each prompt without requiring client-side changes
3. **Protocol Extension**: This provides a standardized way for MCP servers to inject context without protocol changes
4. **Organizational Policies**: Companies can implement centralized context policies through MCP-provided hooks

# Drawbacks

[drawbacks]: #drawbacks

1. **Security Risks**: Executing arbitrary scripts and making HTTP requests introduces security risks
2. **Performance Impact**: Running hooks with each prompt could slow down the conversation flow
3. **Complexity**: Adds another configuration system that users need to understand
4. **Maintenance Burden**: More code to maintain and test
5. **Potential for Abuse**: Could be used to inject misleading context

# Rationale and alternatives

[rationale-and-alternatives]: #rationale-and-alternatives

## Why this design?

1. **Flexibility**: Supports multiple hook types (script, HTTP, inline) to cover various use cases
2. **Integration**: Builds on the existing context system rather than replacing it
3. **User Control**: Allows users to enable/disable hooks as needed
4. **Separation of Concerns**: Clearly separates conversation start hooks from per-prompt hooks

## Alternatives considered

### 1. Context Plugins

Instead of hooks, implement a plugin system where plugins can provide context:

```rust
trait ContextPlugin {
    fn get_conversation_start_context(&self) -> Result<String>;
    fn get_per_prompt_context(&self) -> Result<String>;
}
```

**Rationale for not choosing**: More complex to implement and would require a plugin loading system. Hooks are simpler and more aligned with the command-line nature of the tool.

### 2. Context Templates

Define context templates that can include dynamic elements:

```yaml
templates:
  project-info: |
    # Project: {{exec "basename $(pwd)"}}
    # Language: {{exec "cat .tool-versions 2>/dev/null || echo 'unknown'"}}
```

**Rationale for not choosing**: While powerful, this approach mixes template syntax with context content, which could be confusing. The hook system keeps the execution logic separate from the context content.

### 3. Environment Variables Only

Use environment variables to control context injection:

```bash
export Q_CONTEXT_HOOKS="project-info,git-status"
```

**Rationale for not choosing**: Limited flexibility compared to a configuration file. Doesn't support different hook types or per-hook configuration.

## Impact of not doing this

Without context hooks:

1. Users will continue to manually add context, leading to inconsistent usage
2. Integration with external tools and systems will remain limited
3. Automated workflows involving Amazon Q CLI will be harder to implement
4. The CLI will be less powerful for advanced users and organizations

# Unresolved questions

[unresolved-questions]: #unresolved-questions

1. **Hook Ordering**: Should hooks run in a specific order? Should users be able to specify priority?
2. **Error Handling**: How should errors in hook execution be presented to users?
3. **Context Deduplication**: How should duplicate context from different hooks be handled?
4. **Hook Dependencies**: Should hooks be able to depend on other hooks?
5. **Hook Output Format**: Should hooks output plain text, or should they support structured formats like JSON?
6. **Hook Versioning**: How will we handle backward compatibility as the hook system evolves?

# Future possibilities

[future-possibilities]: #future-possibilities

1. **Hook Marketplace**: A central repository where users can discover and share useful hooks
2. **Hook Templates**: Pre-defined templates for common use cases (git info, AWS environment, etc.)
3. **Conditional Hooks**: Hooks that only run under certain conditions (e.g., in specific directories)
4. **Hook Chaining**: Allow hooks to be chained together, with the output of one hook feeding into another
5. **Interactive Hooks**: Hooks that can prompt the user for input
6. **Hook Scheduling**: Hooks that run on a schedule rather than at conversation start or per prompt
7. **IDE Integration**: Deeper integration with IDEs to provide rich code context
8. **Team Hooks**: Organization-wide hooks that can be centrally managed
9. **Context Visualization**: UI to show which hooks contributed which context
10. **Hook Analytics**: Track which hooks are most useful for improving responses

## Use Case Examples

### Project Information

```bash
#!/bin/bash
# ~/.config/amazon-q/hooks/project-info.sh

echo "# Project Information"
echo "Project: $(basename $(pwd))"
echo "Language: $(cat .tool-versions 2>/dev/null || echo "unknown")"
echo "Framework: $([ -f package.json ] && echo "Node.js" || echo "unknown")"
```

### Git Status

```bash
#!/bin/bash
# ~/.config/amazon-q/hooks/git-status.sh

if git rev-parse --is-inside-work-tree &>/dev/null; then
  echo "# Git Status"
  echo "Branch: $(git branch --show-current)"
  echo "```"
  git status --short
  echo "```"
fi
```

### Personalization

```bash
#!/bin/bash
# ~/.config/amazon-q/hooks/personalization.sh

q-personalization --list-memories
```
