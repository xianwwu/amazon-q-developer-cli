use clap::Subcommand;
use crossterm::style::Stylize;
use crossterm::{
    execute,
    style,
};
use eyre::Result;

use crate::cli::chat::checkpoint::CheckpointManager;
use crate::cli::chat::{
    ChatError,
    ChatSession,
    ChatState,
};
use crate::os::Os;

#[derive(Debug, PartialEq, Subcommand)]
pub enum CheckpointSubcommand {
    /// Revert to a specified checkpoint or the most recent if none specified
    Restore { index: usize },
}

impl CheckpointSubcommand {
    pub async fn execute(self, os: &Os, session: &mut ChatSession) -> Result<ChatState, ChatError> {
        match self {
            Self::Restore { index } => {
                let mut manager_option = CheckpointManager::load_manager(os).await;
                if let Ok(manager) = &mut manager_option {
                    let result = manager.restore(os, index).await;
                    match result {
                        Ok(_) => execute!(
                            session.stderr,
                            style::Print(format!("Restored snapshot: {index}\n").blue())
                        )?,
                        Err(e) => return Err(ChatError::Custom(format!("Could not restore snapshot: {}", e).into())),
                    }
                } else {
                    return Err(ChatError::Custom(
                        format!("Snapshot manager could not be loaded").into(),
                    ));
                }
            },
        }
        Ok(ChatState::PromptUser {
            skip_printing_tools: true,
        })
    }
}
