use std::borrow::Cow;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Api(#[from] fig_api_client::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Client(#[from] super::client::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    SystemTime(#[from] std::time::SystemTimeError),
    #[error("{0}")]
    InvalidToolUse(Cow<'static, str>),
    #[error("An error occurred running the tool: {0}")]
    ToolInvocation(Cow<'static, str>),
    #[error("Model requested the use of an unknown tool: {tool_name}")]
    UnknownToolUse { tool_name: String },
    #[error("{0}")]
    Custom(Cow<'static, str>),
}
