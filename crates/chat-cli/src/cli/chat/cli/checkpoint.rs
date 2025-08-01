use std::io::Write;
use std::str::FromStr;

use clap::Subcommand;
use crossterm::style::{
    StyledContent,
    Stylize,
};
use crossterm::{
    execute,
    style,
};
use dialoguer::FuzzySelect;
use eyre::{
    OptionExt,
    Result,
    bail,
};

use crate::cli::chat::checkpoint::{
    Checkpoint,
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
    Restore { tag: Option<String> },
    /// View all turn-level checkpoints
    List {
        #[arg(short, long)]
        limit: Option<usize>,
    },

    /// Display more information about a turn-level checkpoint
    Expand { tag: String },
}

pub struct CheckpointDisplayEntry {
    pub tag: Tag,
    pub display_parts: Vec<StyledContent<String>>,
}

impl TryFrom<&Checkpoint> for CheckpointDisplayEntry {
    type Error = eyre::Report;

    fn try_from(value: &Checkpoint) -> std::result::Result<Self, Self::Error> {
        let tag = value
            .tag
            .clone()
            .ok_or_eyre("Untagged checkpoints cannot be converted to display entries.")?;
        let mut parts = Vec::new();
        if tag.is_turn() {
            parts.push(format!("[{tag}] ",).blue());
            parts.push(format!("{} - {}", value.timestamp.format("%Y-%m-%d %H:%M:%S"), value.summary).reset());
        } else {
            parts.push(format!("[{tag}] ",).blue());
            parts.push(format!("{}: ", value.tool_name).magenta());
            parts.push(format!("{}", value.summary).reset());
        }

        Ok(Self {
            tag,
            display_parts: parts,
        })
    }
}

impl std::fmt::Display for CheckpointDisplayEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for part in self.display_parts.iter() {
            write!(f, "{}", part)?;
        }
        Ok(())
    }
}

impl CheckpointSubcommand {
    pub async fn execute(self, os: &Os, session: &mut ChatSession) -> Result<ChatState, ChatError> {
        match self {
            Self::Restore { tag } => {
                if let Ok(manager) = &mut CheckpointManager::load_manager(os).await {
                    let tag = if let Some(tag) = tag {
                        tag
                    } else {
                        // If the user doesn't provide a tag, allow them to fuzzy select a checkpoint
                        let display_entries = match gather_all_turn_checkpoints(os).await {
                            Ok(entries) => entries,
                            Err(e) => {
                                return Err(ChatError::Custom(format!("Error getting checkpoints: {e}\n").into()));
                            },
                        };
                        if let Some(index) =
                            fuzzy_select_checkpoints(&display_entries, "Select a checkpoint to restore:")
                        {
                            if index < display_entries.len() {
                                display_entries[index].tag.to_string()
                            } else {
                                return Err(ChatError::Custom(
                                    format!("Selecting checkpoint with index {index} failed\n").into(),
                                ));
                            }
                        } else {
                            return Ok(ChatState::PromptUser { skip_printing_tools: true })
                        }
                    };
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
    let display_entries = gather_all_turn_checkpoints(os).await?;
    for entry in display_entries.iter().take(limit.unwrap_or(display_entries.len())) {
        execute!(output, style::Print(entry), style::Print("\n"))?;
    }
    Ok(())
}

async fn gather_all_turn_checkpoints(os: &Os) -> Result<Vec<CheckpointDisplayEntry>> {
    let mut displays = Vec::new();
    if let Ok(manager) = CheckpointManager::load_manager(os).await {
        for checkpoint in manager.checkpoints {
            if checkpoint.tag.is_none() {
                continue;
            }
            match checkpoint.tag.clone().unwrap() {
                Tag::TurnLevel(_) => {
                    displays.push(CheckpointDisplayEntry::try_from(&checkpoint).unwrap());
                },
                Tag::ToolLevel(..) => (),
            };
        }
    } else {
        bail!("Checkpoints could not be loaded.\n");
    }
    Ok(displays)
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
        let display_entry = CheckpointDisplayEntry::try_from(checkpoint)?;
        execute!(output, style::Print(display_entry), style::Print("\n"))?;

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
                if checkpoint.tag.as_ref().unwrap().is_turn() {
                    break;
                }
                display_vec.push(CheckpointDisplayEntry::try_from(&manager.checkpoints[i])?);
            }

            for entry in display_vec.iter().rev() {
                execute!(
                    output,
                    style::Print(" └─ ".blue()),
                    style::Print(entry),
                    style::Print("\n")
                )?;
            }
        }
    } else {
        execute!(output, style::Print("Checkpoints could not be loaded."))?;
    }
    Ok(())
}

fn fuzzy_select_checkpoints(entries: &Vec<CheckpointDisplayEntry>, prompt_str: &str) -> Option<usize> {
    FuzzySelect::new()
        .with_prompt(prompt_str)
        .items(&entries)
        .report(false)
        .interact_opt()
        .unwrap_or(None)
}
