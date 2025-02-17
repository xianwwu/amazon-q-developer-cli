use fig_api_client::model::{
    ChatMessage,
    ConversationState as FigConversationState,
    Tool,
    ToolInputSchema,
    ToolSpecification,
    UserInputMessage,
    UserInputMessageContext,
};
use fig_settings::history::History;
use tracing::error;

use super::{
    build_env_state,
    build_git_state,
    build_shell_state,
    input_to_modifiers,
};
use crate::cli::chat::ToolConfiguration;
use crate::cli::chat::tools::{
    InputSchema,
    InvokeOutput,
    serde_value_to_document,
};

#[derive(Debug, Clone)]
pub struct ConversationState {
    pub conversation_id: Option<String>,
    pub next_message: Option<Message>,
    pub history: Vec<Message>,
    tool_results: Vec<ToolResult>,
    tools: Vec<Tool>,
}

impl ConversationState {
    pub fn new(tool_config: ToolConfiguration) -> Self {
        Self {
            conversation_id: None,
            next_message: None,
            history: Vec::new(),
            tool_results: Vec::new(),
            tools: tool_config
                .tools
                .into_values()
                .map(|v| {
                    Tool::ToolSpecification(ToolSpecification {
                        name: v.name,
                        description: v.description,
                        input_schema: v.input_schema.into(),
                    })
                })
                .collect(),
        }
    }

    pub async fn append_new_user_message(&mut self, input: String) {
        if self.next_message.is_some() {
            error!("Replacing the next_message with a new message with input: {}", input);
        }

        let (ctx, input) = input_to_modifiers(input);
        let history = History::new();

        let mut user_input_message_context = UserInputMessageContext {
            shell_state: Some(build_shell_state(ctx.history, &history)),
            env_state: Some(build_env_state(&ctx)),
            tool_results: if self.tool_results.is_empty() {
                None
            } else {
                Some(std::mem::take(&mut self.tool_results))
            },
            tools: if self.tools.is_empty() {
                None
            } else {
                Some(self.tools.clone())
            },
            ..Default::default()
        };

        if ctx.git {
            if let Ok(git_state) = build_git_state(None).await {
                user_input_message_context.git_state = Some(git_state);
            }
        }

        let msg = Message(ChatMessage::UserInputMessage(UserInputMessage {
            content: input,
            user_input_message_context: Some(user_input_message_context),
            user_intent: None,
        }));
        self.next_message = Some(msg);
    }

    pub fn push_assistant_message(&mut self, message: Message) {
        self.history.push(message);
    }

    pub fn add_tool_results(&mut self, mut results: Vec<ToolResult>) {
        self.tool_results.append(&mut results);
    }
}

impl From<ConversationState> for FigConversationState {
    fn from(value: ConversationState) -> Self {
        Self {
            conversation_id: value.conversation_id,
            user_input_message: value
                .next_message
                .and_then(|m| match m.0 {
                    ChatMessage::AssistantResponseMessage(_) => None,
                    ChatMessage::UserInputMessage(user_input_message) => Some(user_input_message),
                })
                .expect("no user input message available"),
            history: Some(value.history.into_iter().map(|m| m.0).collect()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Message(pub ChatMessage);

pub type ToolResult = fig_api_client::model::ToolResult;
pub type ToolResultContentBlock = fig_api_client::model::ToolResultContentBlock;
#[allow(dead_code)]
pub type ToolResultStatus = fig_api_client::model::ToolResultStatus;

impl From<InvokeOutput> for ToolResultContentBlock {
    fn from(value: InvokeOutput) -> Self {
        match value.output {
            crate::cli::chat::tools::OutputKind::Text(text) => Self::Text(text),
            crate::cli::chat::tools::OutputKind::Json(value) => Self::Json(serde_value_to_document(value)),
        }
    }
}

impl From<InputSchema> for ToolInputSchema {
    fn from(value: InputSchema) -> Self {
        Self {
            json: Some(serde_value_to_document(value.0)),
        }
    }
}
