use std::io::Write;
use std::str::FromStr;

use clap::Subcommand;
use crossterm::style::Stylize;
use crossterm::{
    execute,
    style,
};
use eyre::Result;

use crate::cli::chat::checkpoint::{
    CheckpointManager,
    Tag,
};
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
    /// View all turn-level checkpoints
    List {
        #[arg(short, long)]
        limit: Option<usize>,
    },

    /// Display more information about a turn-level checkpoint
    Expand { tag: String },
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
                            style::Print(format!("Restored checkpoint: {tag}\n").blue().bold())
                        )?,
                        Err(e) => return Err(ChatError::Custom(format!("Could not restore checkpoint: {}", e).into())),
                    }
                } else {
                    return Err(ChatError::Custom(
                        format!("Checkpoint manager could not be loaded").into(),
                    ));
                }
            },
            Self::List { limit } => match print_all_checkpoints(os, &mut session.stderr, limit).await {
                Ok(_) => (),
                Err(e) => {
                    return Err(ChatError::Custom(
                        format!("Could not display all checkpoints: {e}").into(),
                    ));
                },
            },
            Self::Expand { tag } => match expand_checkpoint(os, &mut session.stderr, tag.clone()).await {
                Ok(_) => (),
                Err(e) => {
                    return Err(ChatError::Custom(
                        format!("Could not expandn checkpoint with tag {}: {e}", tag).into(),
                    ));
                },
            },
        }
        Ok(ChatState::PromptUser {
            skip_printing_tools: true,
        })
    }
}

async fn print_all_checkpoints(os: &Os, output: &mut impl Write, limit: Option<usize>) -> Result<()> {
    let mut num_printed = 0;
    if let Ok(manager) = CheckpointManager::load_manager(os).await {
        for checkpoint in manager.checkpoints {
            if checkpoint.tag.is_none() {
                continue;
            }
            match checkpoint.tag.unwrap() {
                Tag::TurnLevel(i) => {
                    execute!(
                        output,
                        style::Print(format!("[{}]", i).blue()),
                        style::Print(format!(
                            " {} - {}\n",
                            checkpoint.timestamp.format("%Y-%m-%d %H:%M:%S"),
                            checkpoint.summary
                        )),
                    )?;
                    num_printed += 1;
                    if limit.is_some() && num_printed > limit.unwrap() {
                        break;
                    }
                },
                Tag::ToolLevel(..) => (),
            };
        }
    } else {
        execute!(output, style::Print("Checkpoints could not be loaded."))?;
    }
    Ok(())
}

async fn expand_checkpoint(os: &Os, output: &mut impl Write, tag: String) -> Result<()> {
    if let Ok(manager) = CheckpointManager::load_manager(os).await {
        let checkpoint_index = match manager.tag_to_index.get(&Tag::from_str(&tag)?) {
            Some(i) => i,
            None => {
                execute!(output, style::Print(format!("Checkpoint with tag '{tag}' does not exist! Use /checkpoint list to see available checkpoints\n").blue()))?;
                return Ok(());
            },
        };
        let checkpoint = &manager.checkpoints[*checkpoint_index];
        execute!(
            output,
            style::Print(format!("[{}]", checkpoint.tag.as_ref().unwrap()).blue()),
            style::Print(format!(
                " {} - {}\n",
                checkpoint.timestamp.format("%Y-%m-%d %H:%M:%S"),
                checkpoint.summary
            )),
        )?;

        // If the user tries to expand a tool-level checkpoint, return early
        if !checkpoint.tag.as_ref().unwrap().is_turn() {
            return Ok(());
        } else {
            let mut display_vec = Vec::new();
            for i in (0..*checkpoint_index).rev() {
                let checkpoint = &manager.checkpoints[i];
                if checkpoint.tag.is_none() {
                    continue;
                }
                let tag = checkpoint.tag.as_ref().unwrap();
                if tag.is_turn() {
                    break;
                }

                // Since we're iterating backwards, append to display_vec backwards
                display_vec.push(format!("{}\n", checkpoint.summary).reset());
                display_vec.push(format!("{}: ", checkpoint.tool_name).magenta());
                display_vec.push(format!("[{}] ", tag).blue());
                display_vec.push(" └─ ".to_string().blue());
            }

            for elem in display_vec.iter().rev() {
                execute!(output, style::Print(elem))?;
            }
        }
    } else {
        execute!(output, style::Print("Checkpoints could not be loaded."))?;
    }
    Ok(())
}
