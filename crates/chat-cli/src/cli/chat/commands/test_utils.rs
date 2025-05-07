//! Test utilities for command tests

use std::collections::HashMap;

use crate::api_client::StreamingClient;
use crate::cli::chat::conversation_state::ConversationState;
use crate::cli::chat::input_source::InputSource;
use crate::cli::chat::tools::ToolPermissions;
use crate::cli::chat::util::shared_writer::SharedWriter;
use crate::cli::chat::{
    ChatContext,
    ChatError,
    ToolUseStatus,
};
use crate::platform::Context;
use crate::settings::{
    Settings,
    State,
};

/// Create a test chat context for unit tests
pub async fn create_test_chat_context() -> Result<ChatContext, ChatError> {
    // Create a context - Context::new() already returns an Arc<Context>
    let ctx = Context::new();
    let settings = Settings::new();
    let state = State::new();
    let output = SharedWriter::null();
    let input_source = InputSource::new_mock(vec![]);
    let interactive = true;
    let client = StreamingClient::mock(vec![]);

    // Create a tool config
    let tool_config = HashMap::new();

    // Create a conversation state
    let conversation_state = ConversationState::new(ctx.clone(), "test-conversation", tool_config, None, None).await;

    // Create the chat context
    let chat_context = ChatContext {
        ctx,
        settings,
        state,
        output,
        initial_input: None,
        input_source,
        interactive,
        client,
        terminal_width_provider: || Some(80),
        spinner: None,
        conversation_state,
        tool_permissions: ToolPermissions::new(10),
        tool_use_telemetry_events: HashMap::new(),
        tool_use_status: ToolUseStatus::Idle,
        tool_manager: crate::cli::chat::tool_manager::ToolManager::default(),
        failed_request_ids: Vec::new(),
        pending_prompts: std::collections::VecDeque::new(),
    };

    Ok(chat_context)
}

/// Create a test command context adapter for unit tests
pub async fn create_test_command_context(
    chat_context: &mut ChatContext,
) -> Result<crate::cli::chat::commands::CommandContextAdapter<'_>, ChatError> {
    Ok(crate::cli::chat::commands::CommandContextAdapter::new(
        &chat_context.ctx,
        &mut chat_context.output,
        &mut chat_context.conversation_state,
        &mut chat_context.tool_permissions,
        chat_context.interactive,
        &mut chat_context.input_source,
        &chat_context.settings,
    ))
}
