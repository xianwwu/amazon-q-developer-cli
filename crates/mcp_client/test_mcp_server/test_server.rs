use mcp_client::server::{
    self,
    Response,
    ServerError,
    ServerRequestHandler,
};
use mcp_client::transport::base_protocol::{
    JsonRpcMessage,
    JsonRpcResponse,
};
use mcp_client::transport::stdio::JsonRpcStdioTransport;

#[derive(Clone)]
struct Handler;

#[async_trait::async_trait]
impl ServerRequestHandler for Handler {
    async fn handle_request(&self, method: &str, params: Option<serde_json::Value>) -> Result<Response, ServerError> {
        match method {
            "initialize" => {
                let resp = JsonRpcResponse {
                    id: 0,
                    // TODO: fill this in
                    result: None,
                    error: None,
                    ..Default::default()
                };
                let msg = JsonRpcMessage::Response(resp);
                Ok(Some(serde_json::to_value(msg).expect("Failed to convert msg to value")))
            },
            "some_method" => {
                let resp = JsonRpcResponse {
                    id: 0,
                    // TODO: fill this in
                    result: None,
                    error: None,
                    ..Default::default()
                };
                let msg = JsonRpcMessage::Response(resp);
                Ok(Some(serde_json::to_value(msg).expect("Failed to convert msg to value")))
            },
            _ => Err(ServerError::MissingMethod),
        }
    }
}

#[tokio::main]
async fn main() {
    let handler = Handler;
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let mut test_server =
        server::Server::<JsonRpcStdioTransport, _>::new(handler, stdin, stdout).expect("Failed to create server");
    test_server.init().await.expect("Test server failed to init");
    let _ = test_server.await;
}
