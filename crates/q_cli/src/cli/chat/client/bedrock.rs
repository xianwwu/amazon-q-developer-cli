use aws_sdk_bedrockruntime::Client as BedrockClient;
use aws_sdk_bedrockruntime::error::{
    DisplayErrorContext,
    SdkError,
};
use aws_sdk_bedrockruntime::operation::converse_stream::ConverseStreamOutput as BedrockConverseStreamResponse;
use aws_sdk_bedrockruntime::types::error::ConverseStreamOutputError;
use aws_sdk_bedrockruntime::types::{
    Tool as BedrockTool,
    ToolConfiguration as BedrockToolConfiguration,
};
use aws_smithy_types::event_stream::RawMessage;
use aws_types::sdk_config::StalledStreamProtectionConfig;
use eyre::{
    OptionExt,
    Result,
};
use fig_os_shim::Context;
use tracing::debug;

use super::super::types::ConversationState;
use crate::cli::chat::ToolConfiguration;
use crate::cli::chat::types::{
    ContentBlock,
    ConversationRole,
    Message,
    ToolResult,
};

const MODEL_ID: &str = "anthropic.claude-3-5-sonnet-20241022-v2:0";
const CLAUDE_REGION: &str = "us-west-2";

#[derive(Debug)]
pub struct Client {
    client: BedrockClient,
    model_id: String,
    system_prompt: String,
    tool_config: BedrockToolConfiguration,
}

impl Client {
    pub async fn new(ctx: &Context, tool_config: ToolConfiguration) -> Result<Self> {
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

        let tool_index = BedrockToolConfiguration::builder()
            .set_tools(Some(
                tool_config
                    .tools
                    .into_values()
                    .map(aws_sdk_bedrockruntime::types::Tool::from)
                    .collect(),
            ))
            .build()
            .unwrap();

        Ok(Self {
            client,
            model_id: MODEL_ID.to_string(),
            system_prompt: create_system_prompt(ctx).unwrap(),
            tool_config: tool_index,
        })
    }

    pub async fn send_messages(&self, conversation_state: &mut ConversationState) -> Result<SendMessageOutput> {
        debug!(?conversation_state, "Sending messages");
        if conversation_state.tool_results.is_empty() {
            let next_user_msg = conversation_state
                .next_message
                .take()
                .ok_or_eyre("no user message is available to send")?;
            conversation_state.messages.push(next_user_msg);
        } else {
            let tool_results = std::mem::take(&mut conversation_state.tool_results);
            conversation_state.messages.push(Message::new(
                ConversationRole::User,
                tool_results
                    .into_iter()
                    .map(|r| {
                        ContentBlock::ToolResult(ToolResult {
                            tool_use_id: r.tool_use_id,
                            content: r.content,
                            status: r.status,
                        })
                    })
                    .collect(),
            ));
        }
        Ok(self
            .client
            .converse_stream()
            .model_id(self.model_id.clone())
            .system(aws_sdk_bedrockruntime::types::SystemContentBlock::Text(
                self.system_prompt.clone(),
            ))
            .set_messages(Some(
                conversation_state
                    .messages
                    .clone()
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            ))
            .tool_config(self.tool_config.clone())
            .send()
            .await?)
    }
}

impl From<ToolConfiguration> for BedrockToolConfiguration {
    fn from(value: ToolConfiguration) -> Self {
        Self::builder()
            .set_tools(Some(
                value
                    .tools
                    .into_iter()
                    .map(|(_, v)| BedrockTool::ToolSpec(v.into()))
                    .collect(),
            ))
            .build()
            .expect("building ToolConfiguration should not fail")
    }
}

/// Creates a system prompt with context about the user's environment.
fn create_system_prompt(ctx: &Context) -> Result<String> {
    let cwd = ctx.env().current_dir()?;
    let cwd = cwd.to_string_lossy();
    let os = ctx.platform().os();
    let system_prompt = format!(
        r#"You are an expert programmer and CLI chat assistant. You are given a list of tools to use to answer a given prompt.

You should only respond to tasks related to coding. You must never make assumptions about the user's environment. If you need more information,
you MUST make a tool use request.

When you execute a tool, do not assume that the user can see the output directly. You should either show the command output, or explain what the
output contains in a friendly format.

Context about the user's environment is provided below:
- Current working directory: {}
- Operating system: {}
"#,
        cwd, os
    );

    Ok(system_prompt)
}

/// Represents a stream of event blocks that constitute a message in a Bedrock conversation.
///
/// Corresponds to the return of the `ConverseStream` Bedrock API.
pub type SendMessageOutput = BedrockConverseStreamResponse;

#[derive(Debug)]
pub struct Error(SdkError<ConverseStreamOutputError, RawMessage>);

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", DisplayErrorContext(&self.0))?;
        Ok(())
    }
}
