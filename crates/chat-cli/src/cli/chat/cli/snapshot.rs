use std::io::Write;
use std::str::FromStr;

use clap::Subcommand;
use crossterm::style::Stylize;
use crossterm::{
    execute,
    style,
};
use eyre::{
    bail, Result
};

use crate::cli::chat::snapshot::SnapshotManager;
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
    Restore { index: String },

    /// Create a checkpoint
    // Create { message: String },

    /// View all checkpoints
    List {
        #[arg(short, long)]
        limit: Option<usize>,
    },

    /// Delete shadow repository
    Clean,

    /// Display more information about a turn-level snapshot
    Expand { index: usize },
}

impl SnapshotSubcommand {
    pub async fn execute(self, os: &Os, session: &mut ChatSession) -> Result<ChatState, ChatError> {
        match self {
            Self::Init => {
                // Handle case where snapshots are already being tracked
                // if session.conversation.snapshot_manager.is_some() {
                //     execute!(session.stderr, style::Print("Are you sure you want to reinitialize the shadow repo?
                // All history will be lost.\n".blue()))?; }
                if session.conversation.snapshot_manager.is_some() {
                    return Err(ChatError::Custom(
                        "Snapshot manager already exists".into(),
                    ));
                }
                let history_index = session.conversation.get_history_len();
                session.conversation.snapshot_manager = match SnapshotManager::init() {
                    Ok(manager) => Some(manager),
                    Err(_) => return Err(ChatError::Custom("Could not initialize shadow repo".into())),
                };
                let Some(manager) = &mut session.conversation.snapshot_manager else {
                    return Err(ChatError::Custom(
                        "Snapshot manager could not be initialized".into(),
                    ));
                };
                match manager.create_snapshot(os, "Initial snapshot", true, history_index).await {
                    Ok(_) => {
                        execute!(
                            session.stderr,
                            style::Print(format!("Created initial snapshot: 1\n").blue().bold())
                        )?;
                    },
                    Err(e) => {
                        return Err(ChatError::Custom(
                            format!("Could not create initial snapshot: {}", e).into(),
                        ));
                    },
                }
            },
            Self::Restore { index } => {
                // Extract the snapshot manager from the conversation temporarily
                let mut manager = match session.conversation.snapshot_manager.take() {
                    Some(manager) => manager,
                    None => {
                        return Err(ChatError::Custom(
                            "Snapshot manager does not exist; run /snapshot init to initialize".into(),
                        ));
                    },
                };
                let index_obj = Index::from_str(&index)?;
                let (outer, inner) = match index_obj {
                    Index::Single(i) => (i, None),
                    Index::Nested(i, j) => (i, Some(j))
                };
                let result = manager.restore(os, &mut session.conversation, outer, inner).await;

                // Put the snapshot manager back into the conversation
                session.conversation.snapshot_manager = Some(manager);

                match result {
                    Ok(_) => execute!(
                        session.stderr,
                        style::Print(format!("Restored snapshot: {index}\n").blue())
                    )?,
                    Err(e) => return Err(ChatError::Custom(format!("Could not restore snapshot: {}", e).into())),
                }
            },
            // Self::Create { message } => {
            //     let Some(manager) = &mut session.conversation.snapshot_manager else {
            //         return Err(ChatError::Custom(
            //             "Snapshot manager does not exist; run /snapshot init to initialize".into(),
            //         ));
            //     };
            //     match manager.create_snapshot(os, &message, true).await {
            //         Ok(id) => execute!(session.stderr, style::Print(format!("Created snapshot {id}\n").blue()))?,
            //         Err(_) => return Err(ChatError::Custom("Could not create a snapshot".into())),
            //     };
            // },
            Self::List { limit } => {
                let Some(manager) = &session.conversation.snapshot_manager else {
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
                        style::Print(format!("Deleted shadow repository\n").blue().bold())
                    )?,
                    Err(e) => {
                        return Err(ChatError::Custom(
                            format!("Could not delete shadow repository: {e}").into(),
                        ));
                    },
                };
                session.conversation.snapshot_manager = None;
            },
            Self::Expand { index }=> {
                let Some(manager) = &session.conversation.snapshot_manager else {
                    return Err(ChatError::Custom(
                        "Snapshot manager does not exist; run /snapshot init to initialize".into(),
                    ));
                };
                match expand_snapshot(manager, &mut session.stderr, index) {
                    Ok(_) => {},
                    Err(_) => return Err(ChatError::Custom(format!("Could not expand snapshot with index {index}").into())),
                };
            }
        };
        Ok(ChatState::PromptUser {
            skip_printing_tools: true,
        })
    }
}

pub fn list_snapshots(manager: &SnapshotManager, output: &mut impl Write, limit: Option<usize>) -> Result<()> {
    if manager.snapshot_count > 0 {
        execute!(output, style::Print("Current checkpoints:\n"))?;
    } else {
        execute!(output, style::Print("No checkpoints to show!:\n"))?;
        return Ok(());
    }
    for (i, snapshot) in manager
        .snapshot_table
        .iter()
        .enumerate()
        .take(limit.unwrap_or(manager.snapshot_count))
    {
        execute!(
            output,
            style::Print(format!("[{}]", i + 1).blue()),
            style::Print(format!(" {} - {}\n", snapshot.timestamp, snapshot.message))
        )?
    }
    Ok(())
}

pub fn expand_snapshot(manager: &SnapshotManager, output: &mut impl Write, index: usize) -> Result<()> {
    let snapshot = match manager.snapshot_table.get(index - 1) {
        Some(s) => s,
        None => bail!("Invalid checkpoint index"),
    };

    execute!(output, style::Print(format!("[{}] {}\n", index.to_string().blue(), snapshot.message)))?;
    for (i, tool) in snapshot.tool_snapshots.iter().enumerate() {
        let bullet = if i == snapshot.tool_snapshots.len() {
            " ├─"
        } else {
            " └─"
        };
        execute!(
            output,
            style::Print(format!("{} [{}.{}] ", bullet, index, i + 1).blue()),
            style::Print(&tool.message),
            style::Print("\n"),
        )?;
    }
    Ok(())
}

enum Index {
    Single(usize),
    Nested(usize, usize),
}

/// Generated by Q
impl FromStr for Index {
    type Err = ChatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((outer, inner)) = s.split_once('.') {
            let outer_idx = outer.parse::<usize>()
                .map_err(|_| ChatError::Custom(format!("Invalid checkpoint idx: {outer}").into()))?;
            let inner_idx = inner.parse::<usize>()
                .map_err(|_| ChatError::Custom(format!("Invalid checkpoint idx: {outer}").into()))?;
            
            // Convert from 1-based to 0-based indexing
            Ok(Self::Nested(outer_idx - 1, inner_idx - 1))
        } else {
            let idx = s.parse::<usize>()
                .map_err(|_| ChatError::Custom(format!("Invalid checkpoint idx: {s}").into()))?;
            
            // Convert from 1-based to 0-based indexing
            Ok(Self::Single(idx - 1))
        }
    }
}

// Available commands:
// /snapshot list                 - List all turn-level checkpoints
// /snapshot expand <id>          - Show tool-level checkpoints within a turn
// /snapshot restore <id>         - Restore to a specific checkpoint
// /snapshot diff <id>            - Show changes between current state and checkpoint
//
// Current checkpoints:
// [1] 2025-07-22 20:45 - Created initial project structure (+3 files)
// [2] 2025-07-22 20:48 - Added authentication module (+2 files, modified 1)
// [3] 2025-07-22 20:52 - Fixed API endpoint bugs (modified 2 files) ← CURRENT
//
// > /snapshot expand 2
// Added authentication module
// ├─ [2.1] fs_write: Created auth.py (+1 file)
// ├─ [2.2] fs_write: Created auth_test.py (+1 file)
// └─ [2.3] fs_write: Updated app.py to import auth module (modified 1 file)
//
// > /snapshot diff 1
// Comparing current state with checkpoint [1]:
// + auth.py (added)
// + auth_test.py (added)
// ~ app.py (modified)
// - import os, sys
// + import os, sys, auth
// ...
//
// > /snapshot restore 1
//
//
//
