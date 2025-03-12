use std::process::Stdio;

use serde::{
    Deserialize,
    Serialize,
};
use thiserror::Error;

use crate::protocol::{
    Protocol,
    ProtocolError,
};
use crate::transport::stdio::JsonRpcStdioTransport;
use crate::transport::{
    self,
    Transport,
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
    ProtocolError(#[from] ProtocolError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
}

pub struct Client<T: Transport> {
    protocol: Protocol<T>,
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
        let transport = transport::stdio::JsonRpcStdioTransport::client(child);
        let protocol = Protocol::new(transport, timeout);
        Ok(Self { protocol })
    }
}

impl<T> Client<T>
where
    T: Transport,
{
    pub async fn init(&mut self) -> Result<ToolSpec, ClientError> {
        todo!();
    }

    pub async fn request(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, ClientError> {
        let resp = self.protocol.request(method, params).await?;
        Ok(serde_json::to_value(resp)?)
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
        let output = client.request("some_method", None).await.unwrap();

        println!("output is {:?}", output);
    }
}
