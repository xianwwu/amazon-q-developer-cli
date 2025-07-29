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
    /// Revert to a specified checkpoint
    Restore { tag: String },
    // /// View all checkpoints
    // List {
    //     #[arg(short, long)]
    //     limit: Option<usize>,
    // },

    // /// Display more information about a turn-level snapshot
    // Expand { index: usize },
}

impl CheckpointSubcommand {
    pub async fn execute(self, os: &Os, session: &mut ChatSession) -> Result<ChatState, ChatError> {
        match self {
            Self::Restore { tag } => {
                let mut manager_option = CheckpointManager::load_manager(os).await;
                if let Ok(manager) = &mut manager_option {
                    let result = manager
                        .restore_checkpoint(os, &mut session.conversation, tag.clone())
                        .await;
                    match result {
                        Ok(_) => execute!(
                            session.stderr,
                            style::Print(format!("Restored snapshot: {tag}\n").blue())
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

// Turn 1, then turn 2 with 3 tool uses
// turn 1 -> 2.1 -> 2.2 -> 2.3 -> turn 2 CP
