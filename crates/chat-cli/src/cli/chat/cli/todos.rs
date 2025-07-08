#![allow(warnings)]

use clap::Subcommand;
use std::{io, path::PathBuf};
use crate::{cli::chat::tools::todo::TodoState, os::Os};
use crossterm::{
    execute,
    style::{self, Stylize},
};

use crate::cli::chat::{
    ChatError,
    ChatSession,
    ChatState,
};

use eyre::{
    Result,
    bail,
};

use dialoguer::{
    FuzzySelect
};

const TODO_INTERNAL_PROMPT: &str = "This is the internal prompt that will be 
sent to create the todo list";

// ########### REMOVE BEFORE PUSHING ##############
const TODO_STATE_FOLDER_PATH: &str = "/Users/kiranbug/.aws/amazonq/todos"; // temporary path where we store state files
// ########### --------------------- ##############


#[derive(Debug, PartialEq, Subcommand)]
pub enum TodoSubcommand {
    // Task/prompt to generate TODO list for
    Show,
    ClearFinished,
    Select,
}

pub struct TodoDisplayEntry {
    pub num_completed: usize,
    pub num_tasks: usize,
    pub description: String,
    pub path: PathBuf,
}

impl std::fmt::Display for TodoDisplayEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.num_completed == self.num_tasks {
            write!(f, "{} {}", 
                "✓".green().bold(), 
                self.description.clone(),
            )
        } else {
            write!(f, "{} {} ({}/{})", 
                "✗".red().bold(), 
                self.description.clone(),
                self.num_completed,
                self.num_tasks
            )
        }
    }
}

impl TodoSubcommand {
    pub async fn execute(self, os: &mut Os, session: &mut ChatSession) -> Result<ChatState, ChatError> {
        
        execute!(
            session.stderr,
            style::Print("Dummy? Who you callin' a dummy?\n")
        )?;

        match self {
            Self::Show => {
                match self.get_descriptions_and_statuses(os).await {
                    Ok(entries) => {
                        for e in entries {
                            execute!(
                                session.stderr,
                                style::Print(e),
                                style::Print("\n"),
                            );
                        }
                    }
                    Err(e) => { 
                        execute!(
                            session.stderr,
                            style::Print("Could not show to-do lists"),
                        ); 
                    }
                }
            },
            Self::ClearFinished => {
                ();
            },
            Self::Select => {
                match self.get_descriptions_and_statuses(os).await {
                    Ok(entries) => {
                        let selection_index = FuzzySelect::new()
                            .items(&entries)
                            .interact()
                            .unwrap(); // FIX THIS
                        return session.resume_todo(os, entries[selection_index].path.clone()).await;
                    }
                    Err(e) => println!("{:?}", e),
                };
            },
        };
        Ok(ChatState::PromptUser { skip_printing_tools: true })
    }

    async fn get_descriptions_and_statuses(self, os: &Os) -> Result<Vec<TodoDisplayEntry>> {
        let mut out = Vec::new();
        let mut entries = os.fs.read_dir(TODO_STATE_FOLDER_PATH).await?;

        while let Some(entry) = entries.next_entry().await? {
            let contents = os.fs.read_to_string(entry.path()).await?;
            let temp_struct = match serde_json::from_str::<TodoState>(&contents) {
                Ok(state) => state,
                Err(_) => continue,
            };
            out.push(TodoDisplayEntry {
                num_completed: temp_struct.completed.iter().filter(|b| **b).count(),
                num_tasks: temp_struct.completed.len(),
                description: temp_struct.task_description,
                path: entry.path(),
            });
        }
        Ok(out)
    }

}