use clap::Args;
use crate::os::Os;
use crossterm::{
    cursor,
    execute,
    queue,
    style,
};

use crate::cli::chat::{
    ChatError,
    ChatSession,
    ChatState,
};

const TODO_INTERNAL_PROMPT: &str = "This is the internal prompt that will be 
sent to create the todo list";

#[derive(Debug, Clone, PartialEq, Eq, Default, Args)]
pub struct TodoArgs {
    // Task/prompt to generate TODO list for
    task_prompt: String,
}

impl TodoArgs {
    pub async fn execute(self, os: &Os, session: &mut ChatSession) -> Result<ChatState, ChatError> {
        execute!(
            session.stderr,
            style::Print("Dummy? Who you callin' a dummy?\n")
        )?;
        Ok(ChatState::PromptUser { skip_printing_tools: true })
    }

    // pub async fn create_todo_request(os: &Os) {

    // }
}
/*
async fn generate_todo(os: &Os, prompt: &str) {
    // Create todo request using string above
    // This will be a conversation state method
}

*/