use clap::Subcommand;
use crossterm::execute;
use crossterm::style::{
    self,
    Stylize,
};
use eyre::Result;

use crate::cli::chat::consts::MAX_NUMBER_OF_IMAGES_PER_REQUEST;
use crate::cli::chat::snapshots::SnapshotManager;
use crate::cli::chat::{
    ChatError,
    ChatSession,
    ChatState,
};
use crate::os::Os;

#[derive(Debug, PartialEq, Subcommand)]
pub enum SnapshotSubcommand {
    /// Initialize checkpointing
    Init,

    /// Revert to a specified checkpoint or the most recent if none specified
    Revert { snapshot: String },

    /// Create a checkpoint
    Create {
        message: String,
    },

    /// View all checkpoints
    Log,
}

impl SnapshotSubcommand {
    pub async fn execute(self, os: &Os, session: &mut ChatSession) -> Result<ChatState, ChatError> {
        match self {
            Self::Init => {
                // Handle case where snapshots are already being tracked
                // if session.snapshot_manager.is_some() {
                //     execute!(session.stderr, style::Print("Are you sure you want to reinitialize the shadow repo?
                // All history will be lost.\n".green()))?; }
                session.snapshot_manager = match SnapshotManager::init(os).await {
                    Ok(manager) => Some(manager),
                    Err(_) => return Err(ChatError::Custom("Could not initialize shadow repo".into())),
                };
                let Some(manager) = &mut session.snapshot_manager else {
                    return Err(ChatError::Custom(
                        "Snapshot manager was not initialized properly".into(),
                    ));
                };
                match manager.create_snapshot(os, "Initial snapshot").await {
                    Ok(id) => execute!(session.stderr, style::Print(format!("Created initial snapshot {id}\n").green()))?,     
                    Err(_) => return Err(ChatError::Custom("Could not create initial snapshot".into())),
                }
            },
            Self::Revert { snapshot } => {
                let Some(manager) = &mut session.snapshot_manager else {
                    return Err(ChatError::Custom(
                        "Snapshot manager does not exist; run /snapshot init to initialize".into(),
                    ));
                };
                match manager.restore(os, &snapshot).await {
                    Ok(id) => execute!(session.stderr, style::Print(format!("Restored snapshot {id}\n").green()))?,     
                    Err(_) => return Err(ChatError::Custom("Could not create a snapshot".into())),
                }
            },
            Self::Create { message } => {
                let Some(manager) = &mut session.snapshot_manager else {
                    return Err(ChatError::Custom(
                        "Snapshot manager does not exist; run /snapshot init to initialize".into(),
                    ));
                };
                match manager.create_snapshot(os, &message).await {
                    Ok(id) => execute!(session.stderr, style::Print(format!("Created snapshot {id}\n").green()))?,     
                    Err(_) => return Err(ChatError::Custom("Could not create a snapshot".into())),
                };
            },
            Self::Log => {
                println!("User wants to view all checkpoints");
            },
        };
        Ok(ChatState::PromptUser {
            skip_printing_tools: true,
        })
    }
}
