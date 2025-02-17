pub mod aws_tool;
pub mod execute_bash;
pub mod fs_read;
pub mod fs_write;

use async_trait::async_trait;
use aws_sdk_bedrockruntime::types::{
    Tool as BedrockTool,
    ToolInputSchema as BedrockToolInputSchema,
    ToolResultContentBlock,
    ToolSpecification as BedrockToolSpecification,
};
use aws_smithy_types::{
    Document,
    Number as SmithyNumber,
};
use aws_tool::UseAws;
use execute_bash::ExecuteBash;
use eyre::Result;
use fig_os_shim::Context;
use fs_read::FsRead;
use fs_write::FsWrite;
use serde::Deserialize;

pub use super::Error;

/// Represents an executable tool use.
#[async_trait]
pub trait Tool: std::fmt::Debug + std::fmt::Display {
    // shouldn't be a method but traits are broken in rust
    /// The display name of a tool
    fn display_name(&self) -> String;
    /// Invokes the tool asynchronously
    async fn invoke(&self, context: &Context) -> Result<InvokeOutput, Error>;
}

pub fn parse_tool(name: &str, value: serde_json::Value) -> Result<Box<dyn Tool>, Error> {
    let tool = match name {
        "fs_read" => Box::new(serde_json::from_value::<FsRead>(value)?) as Box<dyn Tool>,
        "fs_write" => Box::new(serde_json::from_value::<FsWrite>(value)?) as Box<dyn Tool>,
        "execute_bash" => Box::new(serde_json::from_value::<ExecuteBash>(value)?) as Box<dyn Tool>,
        "use_aws_read_only" => Box::new(serde_json::from_value::<UseAws>(value)?) as Box<dyn Tool>,
        unknown => {
            return Err(Error::UnknownToolUse {
                tool_name: unknown.to_string(),
            });
        },
    };

    Ok(tool)
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
pub struct InputSchema(pub serde_json::Value);

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
