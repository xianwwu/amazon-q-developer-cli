pub mod base_protocol;
pub mod stdio;

use std::fmt::Debug;

use base_protocol::JsonRpcMessage;
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum TransportError {
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("IO error: {0}")]
    Stdio(String),
    #[error("{0}")]
    Custom(String),
    #[error(transparent)]
    RecvError(#[from] tokio::sync::broadcast::error::RecvError),
}

impl From<serde_json::Error> for TransportError {
    fn from(err: serde_json::Error) -> Self {
        TransportError::Serialization(err.to_string())
    }
}

impl From<std::io::Error> for TransportError {
    fn from(err: std::io::Error) -> Self {
        TransportError::Stdio(err.to_string())
    }
}

#[async_trait::async_trait]
pub trait Transport: Send + Sync + Debug + 'static {
    /// Sends a message over the transport layer.
    async fn send(&self, msg: &JsonRpcMessage) -> Result<(), TransportError>;
    /// Listens to awaits for a response.
    async fn listen(&self) -> Result<JsonRpcMessage, TransportError>;
}
