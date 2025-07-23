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
    Restore { snapshot: usize },

    /// Create a checkpoint
    Create { message: String },

    /// View all checkpoints
    List {
        #[arg(short, long)]
        limit: Option<usize>,
    },

    /// Delete shadow repository
    Clean,
}

impl SnapshotSubcommand {
    pub async fn execute(self, os: &Os, session: &mut ChatSession) -> Result<ChatState, ChatError> {
        match self {
            Self::Init => {
                // Handle case where snapshots are already being tracked
                // if session.conversation.snapshot_manager.is_some() {
                //     execute!(session.stderr, style::Print("Are you sure you want to reinitialize the shadow repo?
                // All history will be lost.\n".blue()))?; }
                session.conversation.snapshot_manager = match SnapshotManager::init() {
                    Ok(manager) => Some(manager),
                    Err(_) => return Err(ChatError::Custom("Could not initialize shadow repo".into())),
                };
                let Some(manager) = &mut session.conversation.snapshot_manager else {
                    return Err(ChatError::Custom(
                        "Snapshot manager was not initialized properly".into(),
                    ));
                };
                match manager.create_snapshot(os, "Initial snapshot").await {
                    Ok(oid) => {
                        execute!(
                            session.stderr,
                            style::Print(format!("Created initial snapshot: {oid}\n").blue())
                        )?;
                    },
                    Err(e) => return Err(ChatError::Custom(format!("Could not create initial snapshot: {}", e).into())),
                }
            },
            Self::Restore { snapshot } => {
                // Extract the snapshot manager from the conversation temporarily
                let mut manager = match session.conversation.snapshot_manager.take() {
                    Some(manager) => manager,
                    None => return Err(ChatError::Custom(
                        "Snapshot manager does not exist; run /snapshot init to initialize".into(),
                    )),
                };
            
                let result = manager.restore(os, &mut session.conversation, snapshot).await;
                
                // Put the snapshot manager back into the conversation
                session.conversation.snapshot_manager = Some(manager);
                
                match result {
                    Ok(id) => execute!(
                        session.stderr,
                        style::Print(format!("Restored snapshot: {id}\n").blue())
                    )?,
                    Err(e) => return Err(ChatError::Custom(format!("Could not restore snapshot: {}", e).into())),
                }
            },
            Self::Create { message } => {
                let Some(manager) = &mut session.conversation.snapshot_manager else {
                    return Err(ChatError::Custom(
                        "Snapshot manager does not exist; run /snapshot init to initialize".into(),
                    ));
                };
                match manager.create_snapshot(os, &message).await {
                    Ok(id) => execute!(session.stderr, style::Print(format!("Created snapshot {id}\n").blue()))?,
                    Err(_) => return Err(ChatError::Custom("Could not create a snapshot".into())),
                };
            },
            Self::List { limit } => {
                let Some(manager) = &mut session.conversation.snapshot_manager else {
                    return Err(ChatError::Custom(
                        "Snapshot manager does not exist; run /snapshot init to initialize".into(),
                    ));
                };
                match list_snapshots(manager, &mut session.stderr, limit) {
                    Ok(_) => {},
                    Err(_) => return Err(ChatError::Custom("Could not list snapshots".into())),
                };
            },
            Self::Clean => {
                match SnapshotManager::clean_all(os).await {
                    Ok(_) => execute!(
                        session.stderr,
                        style::Print(format!("Deleted shadow repository\n").blue())
                    )?,
                    Err(e) => {
                        return Err(ChatError::Custom(
                            format!("Could not delete shadow repository: {e}").into(),
                        ));
                    },
                };
                session.conversation.snapshot_manager = None;
            },
        };
        Ok(ChatState::PromptUser {
            skip_printing_tools: true,
        })
    }
}

pub fn list_snapshots(manager: &mut SnapshotManager, output: &mut impl Write, limit: Option<usize>) -> Result<()> {
    // let mut revwalk = manager.repo.revwalk()?;
    // revwalk.push_head()?;

    // let revwalk: Vec<Result<Oid, git2::Error>> = if let Some(limit) = limit {
    //     revwalk.take(limit).collect()
    // } else {
    //     revwalk.collect()
    // };

    // for oid in revwalk {
    //     let oid = oid?;
    //     if let Some(snapshot) = manager.snapshot_map.get(&oid) {
    //         execute!(
    //             output,
    //             style::Print(format!("snapshot:  {}\n", oid).blue()),
    //             style::Print(format!("Time:      {}\n", snapshot.timestamp)),
    //             style::Print(format!("{}\n\n", snapshot.message)),
    //         )?;
    //     }
    // }
    Ok(())
}
