- Feature Name: rule_files_support
- Start Date: 2025-04-08

# Summary

[summary]: #summary

This RFC proposes adding support for `.rule` files to the Amazon Q Developer CLI, similar to how they function in the Amazon Q VSCode extension. Rule files provide a way to define contextual guidance for the AI assistant using natural language instructions, allowing users to customize Amazon Q's behavior across different projects and contexts without modifying code.

# Motivation

[motivation]: #motivation

Currently, the Amazon Q CLI lacks a flexible mechanism for users to provide persistent contextual guidance to the AI assistant. While the CLI supports profiles for configuration settings and context files for providing additional information, it doesn't have a way for users to define rules that influence how the AI interprets and responds to queries in specific situations.

The Amazon Q VSCode extension already supports `.rule` files, which are automatically loaded and applied to conversations. These files allow users to:

1. Define domain-specific knowledge and preferences
2. Establish project-specific conventions and guidelines
3. Configure how Amazon Q should behave in certain contexts
4. Provide additional reference materials for specific topics

By adding support for `.rule` files to the CLI, we can:

1. Create consistency between the CLI and VSCode extension experiences
2. Enable more customizable and context-aware AI interactions
3. Allow teams to share standardized rules across projects
4. Provide a mechanism for users to improve Amazon Q's responses without code changes

# Guide-level explanation

[guide-level-explanation]: #guide-level-explanation

## What are Rule Files?

Rule files are plain text files with a `.rule` extension that contain natural language instructions for Amazon Q. These instructions tell the AI assistant how to behave in certain contexts, what additional information to consider, or what guidelines to follow when responding to user queries.

For example, a rule file might instruct Amazon Q to:

- Include specific documentation when discussing certain topics
- Follow team-specific coding conventions when generating code
- Adhere to company security guidelines when suggesting solutions
- Reference particular resources for domain-specific questions

## How Rule Files Work

Rule files are written in natural language that Amazon Q can understand and apply. When a user interacts with Amazon Q, the assistant automatically considers all applicable rule files based on:

1. The current workspace/project
2. Global user-defined rules
3. The specific topic being discussed

Unlike rigid configuration files, rule files leverage Amazon Q's natural language understanding to interpret and apply guidance in a flexible, context-sensitive manner.

## Example Rule Files

### Project-Specific Coding Standards

```
# coding-standards.rule

When generating or reviewing code for this project:
1. Follow the Google Java Style Guide for Java code
2. Use 2-space indentation for all languages
3. Prefer functional programming patterns over imperative ones
4. Always include comprehensive error handling
5. Add detailed comments for complex algorithms
```

### Security Guidelines

```
# security.rule

When suggesting solutions that involve authentication or data handling:
1. Never store credentials in code or configuration files
2. Always use environment variables or AWS Secrets Manager for sensitive information
3. Ensure all API endpoints use HTTPS
4. Implement proper input validation to prevent injection attacks
5. Follow the principle of least privilege for IAM roles and permissions
```

### Documentation References

```
# documentation.rule

When discussing AWS services, include relevant links to the official AWS documentation.
For questions about our internal libraries, reference the documentation at ~/docs/internal-libs/.
```

## Using Rule Files with the CLI

Users can place rule files in several locations:

1. **Project-specific rules**: `.amazonq/rules/` in the project directory
2. **Global rules**: `~/.aws/amazonq/rules/`
3. **Profile-specific rules**: `~/.aws/amazonq/profiles/<profile-name>/rules/`

The CLI will automatically load and apply these rules based on the current context and conversation topic.

Users can also manage rules using new CLI commands:

```
# List active rules
/rules list

# Enable/disable specific rules
/rules enable <rule-name>
/rules disable <rule-name>

# Create a new rule
/rules create <rule-name>

# Edit an existing rule
/rules edit <rule-name>
```

# Reference-level explanation

[reference-level-explanation]: #reference-level-explanation

## Rule File Discovery and Loading

The CLI will discover and load rule files from multiple locations, with the following precedence (highest to lowest):

1. Project-specific rules: `.amazonq/rules/*.rule` in the current working directory
2. Profile-specific rules: `~/.aws/amazonq/profiles/<current-profile>/rules/*.rule`
3. Global rules: `~/.aws/amazonq/rules/*.rule`
4. Environment variable path: `$AMAZONQ_RULES_PATH/*.rule` (if set)

Rules from all these locations will be combined, with project-specific rules taking precedence over conflicting global rules.

## Rule File Processing

Rule files will be processed as follows:

1. **Discovery**: At startup, the CLI will scan the above locations for `.rule` files
2. **Parsing**: Each file will be read and its contents stored as natural language instructions
3. **Indexing**: Rules will be indexed by filename and content for efficient retrieval
4. **Application**: During conversations, relevant rules will be included in the context sent to the AI

The CLI will use a lightweight semantic matching algorithm to determine which rules are relevant to the current conversation topic, rather than including all rules in every request.

## Integration with Existing Commands

### Profile Integration

Rule files will be integrated with the existing profile system:

```rust
pub struct Profile {
    // Existing fields
    pub name: String,
    pub model: String,
    pub region: String,
    // New field
    pub rules_dir: Option<PathBuf>,
}
```

When switching profiles with `/profile set <name>`, the CLI will automatically load rules from that profile's rules directory.

### Context Integration

Rule files will complement the existing context system:

```rust
pub struct Context {
    // Existing fields
    pub files: Vec<ContextFile>,
    pub history: Vec<Message>,
    // New field
    pub active_rules: Vec<Rule>,
}

pub struct Rule {
    pub name: String,
    pub path: PathBuf,
    pub content: String,
    pub enabled: bool,
}
```

When adding context with `/context add <file>`, users can also specify rule files:

```
/context add --rule coding-standards.rule
```

## New Commands

### Rules Management Commands

```rust
pub enum RulesCommand {
    List,
    Enable(String),
    Disable(String),
    Create(String),
    Edit(String),
    Delete(String),
}

impl RulesCommand {
    pub fn execute(&self, ctx: &mut Context) -> Result<()> {
        match self {
            RulesCommand::List => self.list_rules(ctx),
            RulesCommand::Enable(name) => self.enable_rule(ctx, name),
            RulesCommand::Disable(name) => self.disable_rule(ctx, name),
            RulesCommand::Create(name) => self.create_rule(ctx, name),
            RulesCommand::Edit(name) => self.edit_rule(ctx, name),
            RulesCommand::Delete(name) => self.delete_rule(ctx, name),
        }
    }
    
    // Implementation methods...
}
```

## Rule Application in AI Requests

When sending requests to the AI service, the CLI will include relevant rules in the context:

```rust
pub struct AIRequest {
    // Existing fields
    pub query: String,
    pub context_files: Vec<ContextFile>,
    pub conversation_history: Vec<Message>,
    // New field
    pub active_rules: Vec<String>,
}
```

The AI service will process these rules as natural language instructions that guide its responses.

## Implementation Plan

### Phase 1: Basic Rule Support

1. Add rule file discovery and loading from all locations
2. Implement basic rule application in AI requests
3. Add `/rules list` command to show active rules
4. Add `/rules enable` and `/rules disable` commands

### Phase 2: Enhanced Rule Management

1. Implement `/rules create` and `/rules edit` commands
2. Add rule file validation to ensure proper formatting
3. Implement rule relevance scoring to prioritize rules
4. Add profile-specific rule directories

### Phase 3: Advanced Features

1. Implement rule sharing and synchronization
2. Add support for rule templates
3. Implement rule effectiveness metrics
4. Add support for rule dependencies and composition

## Performance Considerations

To ensure that rule files don't negatively impact performance:

1. Rules will be loaded asynchronously during startup
2. Only relevant rules will be included in AI requests
3. Rules will be cached in memory to avoid repeated disk access
4. Large rule files will be chunked and summarized

## Security Considerations

Rule files introduce several security considerations:

1. **Content Validation**: Rules should be validated to prevent injection attacks
2. **Permission Management**: Access to rule files should be properly controlled
3. **Data Privacy**: Rules should not contain sensitive information
4. **Execution Boundaries**: Rules should not be able to execute arbitrary code

# Drawbacks

[drawbacks]: #drawbacks

There are several potential drawbacks to implementing rule files:

1. **Complexity**: Adding another configuration mechanism increases the overall complexity of the system.

2. **Inconsistent Application**: Natural language rules may be interpreted differently in different contexts, leading to inconsistent behavior.

3. **Performance Impact**: Loading and processing multiple rule files could impact startup time and response latency.

4. **Maintenance Burden**: Users may create complex rule systems that are difficult to maintain and debug.

5. **Security Risks**: Improperly validated rules could potentially be used for prompt injection attacks.

6. **User Confusion**: Having multiple ways to configure behavior (profiles, context, rules) may confuse users about which approach to use.

7. **Testing Challenges**: It may be difficult to test how rules interact with each other and with different user queries.

# Rationale and alternatives

[rationale-and-alternatives]: #rationale-and-alternatives

## Why this design?

This design was chosen for several reasons:

1. **Consistency with VSCode**: It maintains consistency with the existing Amazon Q VSCode extension, providing a familiar experience for users.

2. **Natural Language Flexibility**: Using natural language for rules leverages Amazon Q's core strengths and provides maximum flexibility.

3. **Multiple Sources**: Supporting rules from multiple locations (project, profile, global) allows for layered customization.

4. **Progressive Enhancement**: The design allows for simple rules initially, with the potential for more complex features in the future.

## Alternatives Considered

### Structured Configuration Files

Instead of natural language rules, we could use structured configuration files (JSON, YAML, etc.) to define behavior:

```yaml
rules:
  - name: coding-standards
    topics: [code-generation, code-review]
    instructions:
      - Follow Google Java Style Guide
      - Use 2-space indentation
      - Prefer functional programming
```

This approach would be more precise but less flexible and would require users to learn a specific schema.

### Extended Profile Configuration

We could extend the existing profile system to include rule-like behavior:

```json
{
  "name": "default",
  "model": "anthropic.claude-3-sonnet-20240229-v1:0",
  "region": "us-east-1",
  "behavior": {
    "coding-standards": "Follow Google Java Style Guide...",
    "security": "Never store credentials in code..."
  }
}
```

This would simplify the configuration system but would limit rules to profile-level settings rather than allowing project-specific rules.

### Plugin System

We could implement a more formal plugin system that allows for programmatic customization:

```javascript
// coding-standards.js
module.exports = {
  name: "coding-standards",
  apply: (context, query) => {
    if (query.includes("code") || query.includes("function")) {
      return "Follow Google Java Style Guide...";
    }
    return null;
  }
};
```

This would be more powerful but would require users to write code and would introduce additional security concerns.

## Impact of Not Doing This

If we don't implement rule files:

1. The CLI experience will remain inconsistent with the VSCode extension
2. Users will have limited ability to customize Amazon Q's behavior
3. Teams will lack a standardized way to share guidance across projects
4. Users will need to repeatedly provide the same context in conversations

# Unresolved questions

[unresolved-questions]: #unresolved-questions

1. **Rule Conflicts**: How should conflicts between rules from different sources be resolved?

2. **Rule Validation**: What validation should be performed on rule files to ensure they are effective and secure?

3. **Rule Sharing**: How can teams effectively share and synchronize rules across projects and team members?

4. **Rule Versioning**: Should we implement versioning for rules to track changes over time?

5. **Rule Effectiveness**: How can we measure and improve the effectiveness of rules?

6. **Rule Size Limits**: What limits should be placed on rule file size and complexity?

7. **Rule Dependencies**: Should rules be able to reference or depend on other rules?

8. **Rule Templates**: Should we provide templates or examples for common rule types?

# Future possibilities

[future-possibilities]: #future-possibilities

1. **Rule Marketplace**: Create a marketplace or repository where users can share and discover useful rules.

2. **AI-Generated Rules**: Allow Amazon Q to suggest rules based on observed usage patterns.

3. **Rule Testing Framework**: Develop tools to test and validate rules before applying them.

4. **Visual Rule Editor**: Create a visual interface for creating and editing rules.

5. **Rule Analytics**: Provide analytics on rule usage and effectiveness.

6. **Conditional Rules**: Support rules that only apply under specific conditions or for specific commands.

7. **Rule Composition**: Allow rules to be composed from smaller, reusable components.

8. **Dynamic Rules**: Support rules that can adapt based on conversation history or project context.

9. **Rule Export/Import**: Enable easy export and import of rules between environments.

10. **Integration with Other Tools**: Allow rules to integrate with other development tools and workflows.

11. **Rule-Based Workflows**: Support defining entire workflows or sequences of operations using rules.

12. **Multi-Modal Rules**: Extend rules to support multi-modal interactions (text, code, images).