use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;
use tokio::time;
use uuid::Uuid;

use crate::transport::base_protocol::{
    JsonRpcMessage,
    JsonRpcNotification,
    JsonRpcRequest,
    JsonRpcVersion,
};
use crate::transport::stdio::JsonRpcStdioTransport;
use crate::transport::{
    self,
    Transport,
    TransportError,
};

pub type ToolSpec = serde_json::Value;
pub type StdioTransport = JsonRpcStdioTransport;

#[derive(Debug, Deserialize)]
pub struct ClientConfig {
    pub bin_path: String,
    pub args: Vec<String>,
    pub timeout: u64,
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error(transparent)]
    TransportError(#[from] TransportError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
    #[error(transparent)]
    RuntimeError(#[from] tokio::time::error::Elapsed),
    #[error("Unexpected msg type encountered")]
    UnexpectedMsgType,
}

pub struct Client<T: Transport> {
    transport: Arc<T>,
    timeout: u64,
}

impl Client<StdioTransport> {
    pub fn from_config(config: ClientConfig) -> Result<Self, ClientError> {
        let ClientConfig {
            bin_path,
            args,
            timeout,
        } = config;
        let child = tokio::process::Command::new(bin_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .args(args)
            .spawn()?;
        let transport = Arc::new(transport::stdio::JsonRpcStdioTransport::client(child)?);
        Ok(Self { transport, timeout })
    }
}

impl<T> Client<T>
where
    T: Transport,
{
    pub fn init(&mut self) -> Result<(), ClientError> {
        let transport_ref = self.transport.clone();
        tokio::spawn(async move { 
            loop {
                match transport_ref.listen().await {
                    Ok(msg) => {
                        match msg {
                            JsonRpcMessage::Request(req) => {},
                            JsonRpcMessage::Notification(notif) => {},
                            JsonRpcMessage::Response(_) => { /* noop since direct response is handled inside the request api */ }
                        }
                    }
                    Err(_) => { todo!() }
                }
            } 
        });

        Ok(())
    }

    pub async fn request(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, ClientError> {
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
            return Err(ClientError::UnexpectedMsgType);
        };
        Ok(serde_json::to_value(resp)?)
    }

    pub async fn notify(&mut self, method: &str, params: Option<serde_json::Value>) -> Result<(), ClientError> {
        let notification = JsonRpcNotification {
            jsonrpc: JsonRpcVersion::new(),
            method: method.to_owned(),
            params,
        };
        let msg = JsonRpcMessage::Notification(notification);
        Ok(time::timeout(Duration::from_secs(self.timeout), self.transport.send(&msg)).await??)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    const TEST_BIN_OUT_DIR: &str = "target/debug";
    const TEST_SERVER_NAME: &str = "test_mcp_server";

    fn get_workspace_root() -> PathBuf {
        let output = std::process::Command::new("cargo")
            .args(["metadata", "--format-version=1", "--no-deps"])
            .output()
            .expect("Failed to execute cargo metadata");

        let metadata: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("Failed to parse cargo metadata");

        let workspace_root = metadata["workspace_root"]
            .as_str()
            .expect("Failed to find workspace_root in metadata");

        PathBuf::from(workspace_root)
    }

    #[tokio::test]
    async fn test_client_overall() {
        std::process::Command::new("cargo")
            .args(["build", "--bin", TEST_SERVER_NAME])
            .status()
            .expect("Failed to build binary");
        let mut bin_path = get_workspace_root();
        bin_path.push(TEST_BIN_OUT_DIR);
        bin_path.push(TEST_SERVER_NAME);
        println!("bin path: {}", bin_path.to_str().unwrap_or("no path found"));

        let client_config = ClientConfig {
            bin_path: bin_path.to_str().unwrap().to_string(),
            args: ["1".to_owned()].to_vec(),
            timeout: 60,
        };
        let mut client = Client::<StdioTransport>::from_config(client_config).unwrap();
        client.init();
        let output = client.request("some_method", None).await.unwrap();

        println!("output is {:?}", output);
    }
}
