use tokio::io::{
    AsyncBufReadExt as _,
    AsyncWriteExt as _,
    Stdin,
    Stdout,
};
use tokio::process::Child;
use uuid::Uuid;

use super::base_protocol::JsonRpcMessage;
use super::{
    Transport,
    TransportError,
};
use crate::transport::base_protocol::{
    JsonRpcRequest,
    JsonRpcVersion,
};

#[derive(Debug)]
pub enum JsonRpcStdioTransport {
    Client { server: Child },
    Server { stdin: Stdin, stdout: Stdout },
}

impl JsonRpcStdioTransport {
    pub fn client(child_process: Child) -> Self {
        JsonRpcStdioTransport::Client { server: child_process }
    }

    pub fn server(stdin: Stdin, stdout: Stdout) -> Self {
        JsonRpcStdioTransport::Server { stdin, stdout }
    }
}

#[async_trait::async_trait]
impl Transport for JsonRpcStdioTransport {
    async fn init(&mut self) -> Result<JsonRpcMessage, TransportError> {
        let client_hello = JsonRpcRequest {
            jsonrpc: JsonRpcVersion::new(),
            id: Uuid::new_v4().as_u128(),
            method: "client_hello".to_owned(),
            params: None,
        };
        let msg = JsonRpcMessage::Request(client_hello);
        self.send(&msg).await?;
        Ok(self.listen().await?)
    }

    async fn send(&mut self, msg: &JsonRpcMessage) -> Result<(), TransportError> {
        match self {
            JsonRpcStdioTransport::Client { server } => {
                let mut serialized = serde_json::to_vec(msg)?;
                serialized.push(b'\n');
                let stdin = server
                    .stdin
                    .as_mut()
                    .ok_or(TransportError::Io("Process missing stdin".to_owned()))?;
                stdin
                    .write_all(&serialized)
                    .await
                    .map_err(|e| TransportError::Io(format!("Error writing to server: {:?}", e)))?;
                stdin
                    .flush()
                    .await
                    .map_err(|e| TransportError::Io(format!("Error writing to server: {:?}", e)))?;
                Ok(())
            },
            JsonRpcStdioTransport::Server { stdin, stdout } => {
                todo!()
            },
        }
    }

    async fn listen(&mut self) -> Result<JsonRpcMessage, TransportError> {
        match self {
            JsonRpcStdioTransport::Client { server } => {
                let stdout = server
                    .stdout
                    .as_mut()
                    .ok_or(TransportError::Io("Process missing stdout".to_owned()))?;
                let mut buf_reader = tokio::io::BufReader::new(stdout);
                let mut buffer = Vec::<u8>::new();
                match buf_reader.read_until(b'\n', &mut buffer).await {
                    Ok(0) => Err(TransportError::Io("Nothing was received from server".to_owned())),
                    Ok(_) => {
                        println!("received msg: {:?}", buffer.to_ascii_lowercase());
                        Ok(serde_json::from_slice::<JsonRpcMessage>(&buffer)?)
                    },
                    Err(e) => Err(TransportError::Io(format!("Error receiving from server: {:?}", e))),
                }
            },
            JsonRpcStdioTransport::Server { stdin, stdout } => {
                todo!();
            },
        }
    }
}
