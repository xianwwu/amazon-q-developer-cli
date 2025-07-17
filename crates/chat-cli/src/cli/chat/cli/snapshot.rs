use clap::Subcommand;
use crossterm::execute;
use crossterm::style::{
    self,
    Stylize,
};
use eyre::Result;

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
    Revert { snapshot: Option<String> },

    /// Create a checkpoint
    Create {
        #[arg(short, long)]
        message: String,
    },

    /// View all checkpoints
    Log,
}

impl SnapshotSubcommand {
    pub async fn execute(self, os: &Os, session: &mut ChatSession) -> Result<ChatState, ChatError> {
        match self {
            Self::Init => {
                match SnapshotManager::init(os).await {
                    Ok(_) => execute!(session.stderr, style::Print("Initialized shadow repo\n".green()))?,
                    Err(_) => return Err(ChatError::Custom("Could not initialize shadow repo".into())),
                };
            },
            Self::Revert { snapshot } => {
                println!(
                    "User wants to revert to checkpoint: {}",
                    snapshot.unwrap_or("None".to_string())
                );
            },
            Self::Create { message } => {
                println!("User wants to create a checkpoint with message: {}", message);
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
