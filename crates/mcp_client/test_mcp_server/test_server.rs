use mcp_client::server::{
    self,
    PreServerRequestHandler,
    Response,
    ServerError,
    ServerRequestHandler,
};
use mcp_client::transport::{
    JsonRpcRequest,
    JsonRpcResponse,
    JsonRpcStdioTransport,
};

#[derive(Default)]
struct Handler {
    pending_request: Option<Box<dyn Fn(u64) -> Option<JsonRpcRequest> + Send + Sync>>,
}

impl PreServerRequestHandler for Handler {
    fn register_pending_request_callback(
        &mut self,
        cb: impl Fn(u64) -> Option<JsonRpcRequest> + Send + Sync + 'static,
    ) {
        self.pending_request = Some(Box::new(cb));
    }
}

#[async_trait::async_trait]
impl ServerRequestHandler for Handler {
    async fn handle_initialize(&self, params: Option<serde_json::Value>) -> Result<Response, ServerError> {
        // For test, we are just going to repeat what we received back to the client for it to be
        // verified
        Ok(params)
    }

    async fn handle_incoming(&self, method: &str, params: Option<serde_json::Value>) -> Result<Response, ServerError> {
        match method {
            "notification/initialized" => Ok(None),
            _ => Err(ServerError::MissingMethod),
        }
    }

    async fn handle_response(&self, resp: JsonRpcResponse) -> Result<(), ServerError> {
        let JsonRpcResponse { id, result, error, .. } = resp;
        let pending = self.pending_request.as_ref().and_then(|f| f(id));
        Ok(())
    }

    async fn handle_shutdown(&self) -> Result<(), ServerError> {
        todo!();
    }
}

#[tokio::main]
async fn main() {
    let handler = Handler::default();
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let test_server =
        server::Server::<JsonRpcStdioTransport, _>::new(handler, stdin, stdout).expect("Failed to create server");
    let _ = test_server.init().expect("Test server failed to init").await;
}
