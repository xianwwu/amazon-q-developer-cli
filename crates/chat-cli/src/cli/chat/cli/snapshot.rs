use std::io::Write;

use clap::Subcommand;
use crossterm::style::Stylize;
use crossterm::{
    execute,
    style,
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
    Revert { snapshot: String },

    /// Create a checkpoint
    Create { message: String },

    /// View all checkpoints
    List {
        #[arg(short)]
        verbose: bool,
    },

    /// Delete shadow repository
    Clean,
}

impl SnapshotSubcommand {
    pub async fn execute(self, os: &Os, session: &mut ChatSession) -> Result<ChatState, ChatError> {
        match self {
            Self::Init => {
                // Handle case where snapshots are already being tracked
                // if session.snapshot_manager.is_some() {
                //     execute!(session.stderr, style::Print("Are you sure you want to reinitialize the shadow repo?
                // All history will be lost.\n".blue()))?; }
                session.snapshot_manager = match SnapshotManager::init(os).await {
                    Ok(manager) => Some(manager),
                    Err(_) => return Err(ChatError::Custom("Could not initialize shadow repo".into())),
                };
                let Some(manager) = &mut session.snapshot_manager else {
                    return Err(ChatError::Custom(
                        "Snapshot manager was not initialized properly".into(),
                    ));
                };
                match manager.create_snapshot(os, "Initial snapshot", None).await {
                    Ok(id) => execute!(
                        session.stderr,
                        style::Print(format!("Created initial snapshot {id}\n").blue())
                    )?,
                    Err(_) => return Err(ChatError::Custom("Could not create initial snapshot".into())),
                }
            },
            Self::Revert { snapshot } => {
                let manager = Self::unpack_manager(session)?;
                match manager.restore(os, &snapshot).await {
                    Ok(id) => execute!(
                        session.stderr,
                        style::Print(format!("Restored snapshot {id}\n").blue())
                    )?,
                    Err(_) => return Err(ChatError::Custom("Could not create a snapshot".into())),
                }
            },
            Self::Create { message } => {
                let manager = Self::unpack_manager(session)?;
                match manager.create_snapshot(os, &message, None).await {
                    Ok(id) => execute!(session.stderr, style::Print(format!("Created snapshot {id}\n").blue()))?,
                    Err(_) => return Err(ChatError::Custom("Could not create a snapshot".into())),
                };
            },
            Self::List { verbose } => {
                // Explicitly unpack manager to avoid mutable reference in function call
                let Some(manager) = &mut session.snapshot_manager else {
                    return Err(ChatError::Custom(
                        "Snapshot manager does not exist; run /snapshot init to initialize".into(),
                    ));
                };
                match list_snapshots(manager, &mut session.stderr, verbose) {
                    Ok(_) => {},
                    Err(_) => return Err(ChatError::Custom("Could not list snapshots".into())),
                };
            },
            Self::Clean => {
                let manager = Self::unpack_manager(session)?;
                match manager.clean(os).await {
                    Ok(id) => execute!(
                        session.stderr,
                        style::Print(format!("Deleted shadow repository\n").blue())
                    )?,
                    Err(_) => return Err(ChatError::Custom("Could delete shadow repository".into())),
                };
            },
        };
        Ok(ChatState::PromptUser {
            skip_printing_tools: true,
        })
    }

    fn unpack_manager(session: &mut ChatSession) -> Result<&mut SnapshotManager, ChatError> {
        let Some(manager) = &mut session.snapshot_manager else {
            return Err(ChatError::Custom(
                "Snapshot manager does not exist; run /snapshot init to initialize".into(),
            ));
        };
        Ok(manager)
    }
}

pub fn list_snapshots(manager: &mut SnapshotManager, output: &mut impl Write, verbose: bool) -> Result<()> {
    let mut revwalk = manager.repo.revwalk()?;
    revwalk.push_head()?;

    for oid in revwalk {
        let oid = oid?;
        if let Some(snapshot) = manager.snapshot_map.get(&oid) {
            execute!(
                output,
                style::Print(format!("snapshot: {}\n", oid).blue()),
                style::Print(format!("Time:     {}\n", snapshot.timestamp)),
                style::Print(format!("Message:  {}\n", snapshot.message)),
            )?;
            if verbose {
                if let Some(r) = &snapshot.reason {
                    execute!(output, style::Print(format!("Reason:   {}\n", r)))?;
                };
            }
            execute!(output, style::Print("\n"))?;
        }
    }
    Ok(())
}
