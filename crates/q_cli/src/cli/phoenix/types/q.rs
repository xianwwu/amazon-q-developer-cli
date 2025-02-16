use fig_api_client::model::{
    AssistantResponseMessage,
    ChatMessage,
    ConversationState as FigConversationState,
    Tool,
    ToolInputSchema,
    ToolSpecification,
    UserInputMessage,
    UserInputMessageContext,
};
use tracing::error;

use crate::cli::phoenix::ToolConfiguration;
use crate::cli::phoenix::tools::{
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

    pub fn append_new_user_message(&mut self, input: String) {
        if self.next_message.is_some() {
            error!("Replacing the next_message with a new message with input: {}", input);
        }
        let user_input_message_context = UserInputMessageContext {
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
                    ChatMessage::AssistantResponseMessage(assistant_response_message) => None,
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
pub type ToolResultStatus = fig_api_client::model::ToolResultStatus;

impl From<InvokeOutput> for ToolResultContentBlock {
    fn from(value: InvokeOutput) -> Self {
        match value.output {
            crate::cli::phoenix::tools::OutputKind::Text(text) => Self::Text(text),
            crate::cli::phoenix::tools::OutputKind::Json(value) => Self::Json(serde_value_to_document(value)),
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
