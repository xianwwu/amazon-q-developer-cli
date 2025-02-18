use eyre::Result;
use fig_api_client::StreamingClient;
use fig_api_client::clients::SendMessageOutput;
use fig_os_shim::Context;

use crate::cli::chat::ToolConfiguration;
use crate::cli::chat::conversation_state::ConversationState;

#[derive(Debug)]
pub struct Client(StreamingClient);

impl Client {
    pub async fn new(_: &Context, _tool_config: ToolConfiguration) -> Result<Self> {
        Ok(Self(StreamingClient::new().await?))
    }

    pub async fn send_messages(&self, conversation_state: &mut ConversationState) -> Result<SendMessageOutput> {
        Ok(self.0.send_message(conversation_state.clone().into()).await?)
    }
}

#[derive(Debug)]
pub struct Error;

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}
