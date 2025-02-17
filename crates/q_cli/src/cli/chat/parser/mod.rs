pub mod bedrock;
pub use bedrock::*;

use super::tools::Tool;

// pub mod q;
// pub use q::*;

/// Represents a tool use requested by the assistant.
#[derive(Debug)]
pub struct ToolUse {
    /// Corresponds to the `"toolUseId"` returned by the model.
    #[allow(dead_code)]
    pub tool_use_id: String,
    pub tool: Box<dyn Tool>,
}
