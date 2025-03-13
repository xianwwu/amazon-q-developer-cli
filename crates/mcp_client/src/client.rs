use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use nix::sys::signal::Signal;
use nix::unistd::Pid;
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
    pub tool_name: String,
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
    #[error("{0}")]
    NegotiationError(String),
    #[error("Failed to obtain process id")]
    MissingProcessId,
}

pub struct Client<T: Transport> {
    tool_name: String,
    transport: Arc<T>,
    timeout: u64,
    server_process_id: u32,
}

impl Client<StdioTransport> {
    pub fn from_config(config: ClientConfig) -> Result<Self, ClientError> {
        let ClientConfig {
            tool_name,
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
        let server_process_id = child.id().ok_or(ClientError::MissingProcessId)?;
        let transport = Arc::new(transport::stdio::JsonRpcStdioTransport::client(child)?);
        Ok(Self {
            tool_name,
            transport,
            timeout,
            server_process_id,
        })
    }
}

impl<T> Client<T>
where
    T: Transport,
{
    /// Exchange of information specified as per https://spec.modelcontextprotocol.io/specification/2024-11-05/basic/lifecycle/#initialization
    ///
    /// Also done is the spawn of a background task that constantly listens for incoming messages
    /// from the server.
    pub async fn init(&mut self) -> Result<(), ClientError> {
        let transport_ref = self.transport.clone();
        let tool_name = self.tool_name.clone();

        tokio::spawn(async move {
            loop {
                match transport_ref.listen().await {
                    Ok(msg) => {
                        match msg {
                            JsonRpcMessage::Request(req) => {},
                            JsonRpcMessage::Notification(notif) => {},
                            JsonRpcMessage::Response(_) => { /* noop since direct response is handled inside the request api */
                            },
                        }
                    },
                    Err(e) => {
                        tracing::error!("Background listening thread for client {}: {:?}", tool_name, e);
                    },
                }
            }
        });

        // TODO: construct the init params
        let init_params = None;
        let server_capabilities = self.request("initialize", init_params).await?;
        if let Err(e) = self.examine_server_capabilities(&server_capabilities) {
            #[allow(clippy::map_err_ignore)]
            let pid = Pid::from_raw(
                self.server_process_id
                    .try_into()
                    .map_err(|_| ClientError::MissingProcessId)?,
            );
            let _ = nix::sys::signal::kill(pid, Signal::SIGTERM);
            return Err(ClientError::NegotiationError(format!(
                "Client {} has failed to negotiate server capabilities with server: {:?}",
                self.tool_name, e
            )));
        }
        self.notify("initialized", None).await?;

        Ok(())
    }

    /// Sends a request to the server asociated.
    /// This call will yield until a response is received.
    pub async fn request(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, ClientError> {
        let request = JsonRpcRequest {
            jsonrpc: JsonRpcVersion::default(),
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

    /// Sends a notification to the server associated.
    /// Notifications are requests that expect no responses.
    pub async fn notify(&mut self, method: &str, params: Option<serde_json::Value>) -> Result<(), ClientError> {
        let notification = JsonRpcNotification {
            jsonrpc: JsonRpcVersion::default(),
            method: method.to_owned(),
            params,
        };
        let msg = JsonRpcMessage::Notification(notification);
        Ok(time::timeout(Duration::from_secs(self.timeout), self.transport.send(&msg)).await??)
    }

    fn examine_server_capabilities(&self, ser_cap: &serde_json::Value) -> Result<(), ClientError> {
        // Check the jrpc version.
        // Currently we are only proceeding if the versions are EXACTLY the same.
        let jrpc_version = ser_cap
            .get("jsonrpc")
            .map(|v| {
                v.to_string()
                    .trim_matches('"')
                    .replace("\\\"", "\"")
                    .split(".")
                    .map(|n| n.parse::<u32>())
                    .collect::<Vec<Result<u32, _>>>()
            })
            .ok_or(ClientError::NegotiationError("Missing jsonrpc from server".to_owned()))?;
        let client_jrpc_version = JsonRpcVersion::default().as_u32_vec();
        for (sv, cv) in jrpc_version.iter().zip(client_jrpc_version.iter()) {
            let sv = sv
                .as_ref()
                .map_err(|e| ClientError::NegotiationError(format!("Failed to parse server jrpc version: {:?}", e)))?;
            if sv != cv {
                return Err(ClientError::NegotiationError(
                    "Incompatible jrpc version between server and client".to_owned(),
                ));
            }
        }
        Ok(())
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
            tool_name: "test_tool".to_owned(),
            bin_path: bin_path.to_str().unwrap().to_string(),
            args: ["1".to_owned()].to_vec(),
            timeout: 60,
        };
        let mut client = Client::<StdioTransport>::from_config(client_config).expect("Failed to create client");
        client.init().await.expect("Client init failed");
        let output = client.request("some_method", None).await.unwrap();

        println!("output is {:?}", output);
    }
}
