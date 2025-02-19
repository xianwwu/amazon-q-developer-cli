pub mod execute_bash;
pub mod fs_read;
pub mod fs_write;
pub mod use_aws;

use std::io::Stdout;
use std::path::Path;

use async_trait::async_trait;
use aws_smithy_types::{
    Document,
    Number as SmithyNumber,
};
use execute_bash::ExecuteBash;
use eyre::Result;
use fig_api_client::model::{
    ToolResult,
    ToolResultContentBlock,
    ToolResultStatus,
};
use fig_os_shim::Context;
use fs_read::FsRead;
use fs_write::FsWrite;
use serde::Deserialize;
use use_aws::UseAws;

use super::parser::ToolUse;

/// Represents an executable tool use.
#[async_trait]
pub trait Tool: std::fmt::Debug {
    // shouldn't be a method but traits are broken in rust
    /// The display name of a tool
    fn display_name(&self) -> String;
    /// Invokes the tool asynchronously
    async fn invoke(&self, context: &Context, updates: &mut Stdout) -> Result<InvokeOutput>;
    /// Queues up a tool's intention in a human readable format
    fn show_readable_intention(&self, updates: &mut Stdout) -> Result<()>;
    /// Validates the tool with the arguments supplied
    async fn validate(&mut self, ctx: &Context) -> Result<()>;
}

pub fn parse_tool(tool_use: ToolUse) -> Result<Box<dyn Tool>, ToolResult> {
    let map_err = |parse_error| ToolResult {
        tool_use_id: tool_use.id.clone(),
        content: vec![ToolResultContentBlock::Text(format!(
            "Serde failed to deserialize with the following error: {parse_error}"
        ))],
        status: ToolResultStatus::Error,
    };

    Ok(match tool_use.name.as_str() {
        "fs_read" => Box::new(serde_json::from_str::<FsRead>(&tool_use.args).map_err(map_err)?) as Box<dyn Tool>,
        "fs_write" => Box::new(serde_json::from_str::<FsWrite>(&tool_use.args).map_err(map_err)?) as Box<dyn Tool>,
        "execute_bash" => {
            Box::new(serde_json::from_str::<ExecuteBash>(&tool_use.args).map_err(map_err)?) as Box<dyn Tool>
        },
        "use_aws" => Box::new(serde_json::from_str::<UseAws>(&tool_use.args).map_err(map_err)?) as Box<dyn Tool>,
        unknown => {
            return Err(ToolResult {
                tool_use_id: tool_use.id,
                content: vec![ToolResultContentBlock::Text(format!(
                    "The tool, \"{unknown}\" is not supported by the client"
                ))],
                status: ToolResultStatus::Error,
            });
        },
    })
}

/// A tool specification to be sent to the model as part of a conversation. Maps to
/// [BedrockToolSpecification].
#[derive(Debug, Clone, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: InputSchema,
}

/// The schema specification describing a tool's fields.
#[derive(Debug, Clone, Deserialize)]
pub struct InputSchema(pub serde_json::Value);

/// The output received from invoking a [Tool].
#[derive(Debug, Default)]
pub struct InvokeOutput {
    pub output: OutputKind,
}

#[non_exhaustive]
#[derive(Debug)]
pub enum OutputKind {
    Text(String),
    Json(serde_json::Value),
}

impl Default for OutputKind {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

pub fn serde_value_to_document(value: serde_json::Value) -> Document {
    match value {
        serde_json::Value::Null => Document::Null,
        serde_json::Value::Bool(bool) => Document::Bool(bool),
        serde_json::Value::Number(number) => {
            if number.is_f64() {
                Document::Number(SmithyNumber::Float(number.as_f64().unwrap()))
            } else if number.as_i64().is_some_and(|n| n < 0) {
                Document::Number(SmithyNumber::NegInt(number.as_i64().unwrap()))
            } else {
                Document::Number(SmithyNumber::PosInt(number.as_u64().unwrap()))
            }
        },
        serde_json::Value::String(string) => Document::String(string),
        serde_json::Value::Array(vec) => {
            Document::Array(vec.clone().into_iter().map(serde_value_to_document).collect::<_>())
        },
        serde_json::Value::Object(map) => Document::Object(
            map.into_iter()
                .map(|(k, v)| (k, serde_value_to_document(v)))
                .collect::<_>(),
        ),
    }
}

/// Returns a display-friendly [String] of `path` relative to `cwd`, returning `path` if either
/// `cwd` or `path` is invalid UTF-8, or `path` is not prefixed by `cwd`.
fn relative_path(cwd: impl AsRef<Path>, path: impl AsRef<Path>) -> String {
    match (cwd.as_ref().to_str(), path.as_ref().to_str()) {
        (Some(cwd), Some(path)) => path.strip_prefix(cwd).unwrap_or_default().to_string(),
        _ => path.as_ref().to_string_lossy().to_string(),
    }
}
