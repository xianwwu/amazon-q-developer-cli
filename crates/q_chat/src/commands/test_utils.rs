//! Test utilities for command tests

use std::collections::HashMap;

use eyre::Result;
use fig_api_client::StreamingClient;
use fig_os_shim::Context;
use fig_settings::{
    Settings,
    State,
};

use crate::conversation_state::ConversationState;
use crate::input_source::InputSource;
use crate::tools::ToolPermissions;
use crate::util::shared_writer::SharedWriter;
use crate::{
    ChatContext,
    ToolUseStatus,
};

/// Create a test chat context for unit tests
pub async fn create_test_chat_context() -> Result<ChatContext> {
    // Create a context - Context::new_fake() already returns an Arc<Context>
    let ctx = Context::new_fake();
    let settings = Settings::new_fake();
    let state = State::new_fake();
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
        tool_manager: crate::tool_manager::ToolManager::default(),
        failed_request_ids: Vec::new(),
        pending_prompts: std::collections::VecDeque::new(),
    };

    Ok(chat_context)
}

/// Create a test command context adapter for unit tests
pub async fn create_test_command_context(
    chat_context: &mut ChatContext,
) -> Result<crate::commands::CommandContextAdapter<'_>> {
    Ok(crate::commands::CommandContextAdapter::new(
        &chat_context.ctx,
        &mut chat_context.output,
        &mut chat_context.conversation_state,
        &mut chat_context.tool_permissions,
        chat_context.interactive,
        &mut chat_context.input_source,
        &chat_context.settings,
    ))
}
