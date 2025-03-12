use std::time::Duration;

use thiserror::Error;
use tokio::time;
use uuid::Uuid;

use crate::transport::base_protocol::{
    JsonRpcMessage,
    JsonRpcNotification,
    JsonRpcRequest,
    JsonRpcResponse,
    JsonRpcVersion,
};
use crate::transport::{
    Transport,
    TransportError,
};

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error(transparent)]
    TransportError(#[from] TransportError),
    #[error(transparent)]
    RuntimeError(#[from] tokio::time::error::Elapsed),
    #[error("Unexpected msg type encountered")]
    UnexpectedMsgType,
}

#[derive(Debug)]
pub struct Protocol<T: Transport> {
    transport: T,
    timeout: u64,
}

impl<T> Protocol<T>
where
    T: Transport,
{
    pub fn new(transport: T, timeout: u64) -> Self {
        Self { transport, timeout }
    }

    pub async fn request(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<JsonRpcResponse, ProtocolError> {
        let request = JsonRpcRequest {
            jsonrpc: JsonRpcVersion::new(),
            id: Uuid::new_v4().as_u128(),
            method: method.to_owned(),
            params,
        };
        let msg = JsonRpcMessage::Request(request);
        time::timeout(Duration::from_secs(self.timeout), self.transport.send(&msg)).await??;
        let resp = time::timeout(Duration::from_secs(self.timeout), self.transport.listen()).await??;
        let JsonRpcMessage::Response(resp) = resp else {
            return Err(ProtocolError::UnexpectedMsgType);
        };
        Ok(resp)
    }

    pub async fn notify(&mut self, method: &str, params: Option<serde_json::Value>) -> Result<(), ProtocolError> {
        let notification = JsonRpcNotification {
            jsonrpc: JsonRpcVersion::new(),
            method: method.to_owned(),
            params,
        };
        let msg = JsonRpcMessage::Notification(notification);
        Ok(time::timeout(Duration::from_secs(self.timeout), self.transport.send(&msg)).await??)
    }
}
