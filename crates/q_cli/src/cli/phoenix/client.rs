use std::sync::{
    Arc,
    Mutex,
};

use aws_sdk_bedrockruntime::Client as BedrockClient;
use aws_sdk_bedrockruntime::error::{
    DisplayErrorContext,
    SdkError,
};
use aws_sdk_bedrockruntime::operation::converse_stream::ConverseStreamOutput as BedrockConverseStreamResponse;
use aws_sdk_bedrockruntime::types::ConverseStreamOutput;
use aws_sdk_bedrockruntime::types::error::ConverseStreamOutputError;
use aws_smithy_types::event_stream::RawMessage;
use aws_types::sdk_config::StalledStreamProtectionConfig;
use eyre::{
    Result,
    bail,
};
use tracing::debug;

use super::Message;
use super::tools::ToolConfig;

const CLAUDE_REGION: &str = "us-west-2";

/// A client for calling the Bedrock ConverseStream API.
// #[derive(Debug)]
// pub struct Client {
//     client: BedrockClient,
//     model_id: String,
//     system_prompt: String,
//     tool_config: ToolConfig,
// }

#[derive(Debug)]
pub struct Client(inner::Inner);

mod inner {
    use std::sync::{
        Arc,
        Mutex,
    };

    use aws_sdk_bedrockruntime::Client as BedrockClient;

    use super::ConverseStreamResponse;
    use crate::cli::phoenix::tools::ToolConfig;

    #[derive(Debug)]
    pub enum Inner {
        Real {
            client: BedrockClient,
            model_id: String,
            system_prompt: String,
            tool_config: ToolConfig,
        },
        Fake {
            responses: Arc<Mutex<std::vec::IntoIter<ConverseStreamResponse>>>,
        },
    }
}

impl Client {
    pub async fn new(model_id: String, system_prompt: String, tool_config: ToolConfig) -> Self {
        let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .stalled_stream_protection(
                StalledStreamProtectionConfig::enabled()
                    .grace_period(std::time::Duration::from_secs(100))
                    .build(),
            )
            .region(CLAUDE_REGION)
            .load()
            .await;
        let client = BedrockClient::new(&sdk_config);
        Self(inner::Inner::Real {
            client,
            model_id,
            system_prompt,
            tool_config,
        })
    }

    pub fn new_mock(responses: Vec<ConverseStreamResponse>) -> Self {
        Self(inner::Inner::Fake {
            responses: Arc::new(Mutex::new(responses.into_iter())),
        })
    }

    pub async fn send_messages(&self, messages: Vec<Message>) -> Result<ConverseStreamResponse> {
        debug!(?messages, "Sending messages");
        let messages = messages.into_iter().map(Into::into).collect();
        match &self.0 {
            inner::Inner::Real {
                client,
                model_id,
                system_prompt,
                tool_config,
            } => Ok(ConverseStreamResponse(StreamResponse::Bedrock(
                client
                    .converse_stream()
                    .model_id(model_id)
                    .system(aws_sdk_bedrockruntime::types::SystemContentBlock::Text(
                        system_prompt.clone(),
                    ))
                    .set_messages(Some(messages))
                    .tool_config(tool_config.clone().into())
                    .send()
                    .await?,
            ))),
            inner::Inner::Fake { responses } => Ok(responses.lock().unwrap().next().unwrap()),
        }
    }
}

/// Represents a stream of event blocks that constitute a message in a Bedrock conversation.
///
/// Corresponds to the return of the `ConverseStream` Bedrock API.
#[derive(Debug)]
pub struct ConverseStreamResponse(StreamResponse);

impl ConverseStreamResponse {
    pub async fn recv(
        &mut self,
    ) -> Result<Option<ConverseStreamOutput>, SdkError<ConverseStreamOutputError, RawMessage>> {
        match &mut self.0 {
            StreamResponse::Bedrock(converse_stream_output) => Ok(converse_stream_output.stream.recv().await?),
            StreamResponse::Fake(vec) => todo!(),
        }
    }
}

#[derive(Debug)]
enum StreamResponse {
    Bedrock(BedrockConverseStreamResponse),
    Fake(Vec<ConverseStreamOutput>),
}

#[derive(Debug)]
pub struct Error(SdkError<ConverseStreamOutputError, RawMessage>);

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", DisplayErrorContext(&self.0))?;
        Ok(())
    }
}

// #[derive(Debug)]
// enum ResponseStreamEvent {
//     /// <p>The messages output content block delta.</p>
//     ContentBlockDelta(crate::types::ContentBlockDeltaEvent),
//     /// <p>Start information for a content block.</p>
//     ContentBlockStart(crate::types::ContentBlockStartEvent),
//     /// <p>Stop information for a content block.</p>
//     ContentBlockStop(crate::types::ContentBlockStopEvent),
//     /// <p>Message start information.</p>
//     MessageStart(crate::types::MessageStartEvent),
//     /// <p>Message stop information.</p>
//     MessageStop(crate::types::MessageStopEvent),
//     /// <p>Metadata for the converse output stream.</p>
//     Metadata(crate::types::ConverseStreamMetadataEvent),
// }
