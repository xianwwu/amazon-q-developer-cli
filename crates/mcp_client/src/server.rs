use std::future::Future;
use std::sync::Arc;

use tokio::io::{
    Stdin,
    Stdout,
};
use tokio::task::JoinHandle;

use crate::client::StdioTransport;
use crate::transport::base_protocol::{
    JsonRpcError,
    JsonRpcMessage,
    JsonRpcRequest,
    JsonRpcResponse,
};
use crate::transport::stdio::JsonRpcStdioTransport;
use crate::transport::{
    Transport,
    TransportError,
};

pub type Request = serde_json::Value;
pub type Response = Option<serde_json::Value>;

#[async_trait::async_trait]
pub trait ServerRequestHandler: Send + Sync + Clone + 'static {
    async fn handle_request(&self, method: &str, params: Option<serde_json::Value>) -> Result<Response, ServerError>;
}

pub struct Server<T: Transport, H: ServerRequestHandler> {
    transport: Arc<T>,
    handler: H,
    listener: Option<JoinHandle<Result<(), ServerError>>>,
}

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error(transparent)]
    TransportError(#[from] TransportError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
    #[error("Unexpected msg type encountered")]
    UnexpectedMsgType,
    #[error("{0}")]
    NegotiationError(String),
    #[error("Failed to obtain request method")]
    MissingMethod,
    #[error(transparent)]
    TokioJoinError(#[from] tokio::task::JoinError)
}

impl<H> Server<StdioTransport, H>
where
    H: ServerRequestHandler,
{
    pub fn new(handler: H, stdin: Stdin, stdout: Stdout) -> Result<Self, ServerError> {
        let transport = JsonRpcStdioTransport::server(stdin, stdout)?;
        Ok(Self {
            transport: Arc::new(transport),
            handler,
            listener: None,
        })
    }
}

impl<T, H> Clone for Server<T, H> 
where T: Transport, H: ServerRequestHandler
{
    fn clone(&self) -> Self {
        Self {
            transport: self.transport.clone(),
            handler: self.handler.clone(),
            listener: None
        }
    }
}

impl<T, H> Server<T, H>
where
    T: Transport,
    H: ServerRequestHandler,
{
    pub async fn init(&mut self) -> Result<(), ServerError> {
        let server_clone = self.clone();
        let listener = tokio::spawn(async move {
            loop {
                match server_clone.transport.listen().await {
                    Ok(msg) => {
                        match msg {
                            JsonRpcMessage::Request(req) => {
                                let jsonrpc = req.jsonrpc.clone();
                                let id = req.id;
                                let resp = server_clone.handle_request(req).await.map_or_else(
                                    // TODO: handle error generation
                                    |error| {
                                        let err = JsonRpcError {
                                            code: 0,
                                            message: error.to_string(),
                                            data: None,
                                        };
                                        let resp = JsonRpcResponse {
                                            jsonrpc: jsonrpc.clone(),
                                            id,
                                            result: None,
                                            error: Some(err),
                                        }; 
                                        JsonRpcMessage::Response(resp)
                                    },
                                    |result| {
                                        let resp = JsonRpcResponse {
                                            jsonrpc: jsonrpc.clone(),
                                            id,
                                            result,
                                            error: None,
                                        };
                                        JsonRpcMessage::Response(resp)
                                    },
                                );
                                let _ = server_clone.transport.send(&resp).await;
                            },
                            JsonRpcMessage::Notification(_notif) => {},
                            JsonRpcMessage::Response(_) => { /* noop since direct response is handled inside the request api */
                            },
                        }
                    },
                    Err(_e) => {
                        // TODO: error handling
                    },
                }
            }
        });
        self.listener.replace(listener);
        Ok(())
    }

    async fn handle_request(&self, request: JsonRpcRequest) -> Result<Response, ServerError> {
        let JsonRpcRequest { ref method, params, .. } = request;
        self.handler.handle_request(method, params).await
    }
}

impl<T, H> Future for Server<T, H>
where
    T: Transport,
    H: ServerRequestHandler,
{
    type Output = Result<(), ServerError>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        // SAFETY: we're not moving any pinned fields out of self
        let self_mut = unsafe { self.as_mut().get_unchecked_mut() };
        let Some(listener) = self_mut.listener.take() else {
            return std::task::Poll::Ready(Ok(()));
        };
        let listener = std::pin::pin!(listener);
        listener.poll(cx)?
    }
}
