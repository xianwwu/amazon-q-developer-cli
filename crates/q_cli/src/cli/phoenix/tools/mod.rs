pub mod custom;
pub mod execute_bash;
pub mod filesystem_read;
pub mod filesystem_write;
pub mod use_aws_read_only;

use std::borrow::Cow;
use std::collections::{
    HashMap,
    VecDeque,
};
use std::fs::Metadata;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use async_trait::async_trait;
use aws_sdk_bedrockruntime::types::{
    Tool as BedrockTool,
    ToolConfiguration as BedrockToolConfiguration,
    ToolInputSchema as BedrockToolInputSchema,
    ToolResultContentBlock,
    ToolSpecification as BedrockToolSpecification,
};
use aws_smithy_types::{
    Document,
    Number as SmithyNumber,
};
use bstr::ByteSlice;
use execute_bash::{
    ExecuteBash,
    execute_bash,
};
use eyre::{
    Result,
    bail,
};
use fig_os_shim::{
    Context,
    ContextArcProvider,
};
use filesystem_read::{
    FileSystemRead,
    filesystem_read,
};
use filesystem_write::{
    FileSystemWrite,
    filesystem_write,
};
use nix::NixPath;
use nix::unistd::{
    geteuid,
    getuid,
};
use serde::Deserialize;
use tracing::{
    debug,
    error,
    info,
    warn,
};

pub use super::Error;

/// Represents an executable tool use.
#[async_trait]
pub trait Tool: std::fmt::Debug + std::fmt::Display {
    async fn invoke(&self) -> Result<InvokeOutput, Error>;
    fn requires_consent(&self) -> bool {
        false
    }
}

pub fn new_tool<C: ContextArcProvider>(
    ctx: C,
    name: &str,
    value: serde_json::Value,
) -> Result<Box<dyn Tool + Sync>, Error> {
    let tool = match name {
        "filesystem_read" => Box::new(FileSystemRead::from_value(ctx.context_arc(), value)?) as Box<dyn Tool + Sync>,
        "filesystem_write" => Box::new(FileSystemWrite::from_value(ctx.context_arc(), value)?) as Box<dyn Tool + Sync>,
        "execute_bash" => Box::new(ExecuteBash::from_value(ctx.context_arc(), value)?) as Box<dyn Tool + Sync>,
        custom_name => {
            return Err(Error::Custom(
                format!("custom tools are not supported: model request tool {}", custom_name).into(),
            ));
        },
    };
    Ok(tool)
}

pub fn load_tool_config() -> ToolConfig {
    let fs_read = filesystem_read();
    let fs_write = filesystem_write();
    let execute_bash = execute_bash();
    ToolConfig(HashMap::from([
        (fs_read.name.clone(), fs_read),
        (fs_write.name.clone(), fs_write),
        (execute_bash.name.clone(), execute_bash),
    ]))
}

#[derive(Debug, Clone)]
pub struct ToolConfig(HashMap<String, ToolSpec>);

impl ToolConfig {
    pub fn get_by_name(&self, tool_name: impl AsRef<str>) -> Option<&ToolSpec> {
        self.0.get(tool_name.as_ref())
    }
}

impl From<ToolConfig> for BedrockToolConfiguration {
    fn from(value: ToolConfig) -> Self {
        BedrockToolConfiguration::builder()
            .set_tools(Some(value.0.values().cloned().map(Into::into).collect::<_>()))
            .build()
            .expect("building the tool configuration should not fail with tools set")
    }
}

#[derive(Debug)]
pub enum BuiltinToolName {
    FileSystemRead,
    FileSystemWrite,
    ExecuteBash,
}
impl BuiltinToolName {
    const fn name(&self) -> &'static str {
        match self {
            BuiltinToolName::FileSystemRead => "filesystem_read",
            BuiltinToolName::FileSystemWrite => "filesystem_write",
            BuiltinToolName::ExecuteBash => "execute_bash",
        }
    }
}

impl std::fmt::Display for BuiltinToolName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// A tool specification to be sent to the model as part of a conversation. Maps to
/// [BedrockToolSpecification].
#[derive(Debug, Clone, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: InputSchema,
}

impl From<ToolSpec> for BedrockTool {
    fn from(value: ToolSpec) -> Self {
        BedrockTool::ToolSpec(value.into())
    }
}

#[allow(clippy::fallible_impl_from)]
impl From<ToolSpec> for BedrockToolSpecification {
    fn from(value: ToolSpec) -> Self {
        BedrockToolSpecification::builder()
            .name(value.name)
            .description(value.description)
            .input_schema(value.input_schema.into())
            .build()
            .unwrap()
    }
}

/// The schema specification describing a tool's fields. Maps to [BedrockToolInputSchema].
#[derive(Debug, Clone, Deserialize)]
pub struct InputSchema(serde_json::Value);

impl From<InputSchema> for BedrockToolInputSchema {
    fn from(value: InputSchema) -> Self {
        BedrockToolInputSchema::Json(serde_value_to_document(value.0))
    }
}

/// The output received from invoking a [Tool].
#[derive(Debug, Default)]
pub struct InvokeOutput {
    pub output: OutputKind,
}

impl InvokeOutput {
    fn text(&self) -> Option<&str> {
        match &self.output {
            OutputKind::Text(text) => Some(text),
            _ => None,
        }
    }

    fn json(&self) -> Option<&serde_json::Value> {
        match &self.output {
            OutputKind::Json(value) => Some(value),
            _ => None,
        }
    }
}

impl From<InvokeOutput> for ToolResultContentBlock {
    fn from(value: InvokeOutput) -> Self {
        match value.output {
            OutputKind::Text(text) => ToolResultContentBlock::Text(text),
            OutputKind::Json(value) => ToolResultContentBlock::Json(serde_value_to_document(value)),
        }
    }
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
