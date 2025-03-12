pub mod base_protocol;
pub mod stdio;

use std::fmt::Debug;

use base_protocol::JsonRpcMessage;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
    #[error("{0}")]
    Io(String),
}

#[async_trait::async_trait]
pub trait Transport: Send + Sync + Debug {
    /// Method for init handshake as per https://spec.modelcontextprotocol.io/specification/2024-11-05/basic/lifecycle/.
    async fn init(&mut self) -> Result<JsonRpcMessage, TransportError>;
    /// Sends a message over the transport layer.
    async fn send(&mut self, msg: &JsonRpcMessage) -> Result<(), TransportError>;
    /// Listens to Awaits for a response.
    async fn listen(&mut self) -> Result<JsonRpcMessage, TransportError>;
}
