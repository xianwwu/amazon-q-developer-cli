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

use crate::cli::chat::tools::todo::{
    build_path,
    TODO_STATE_FOLDER_PATH,
};

use dialoguer::{
    FuzzySelect
};

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
        match self {
            Self::Show => {
                match self.get_descriptions_and_statuses(os).await {
                    Ok(entries) => {
                        if entries.len() == 0 {
                            execute!(
                                session.stderr,
                                style::Print("No to-do lists to show"),
                            );
                        }
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
                        if entries.len() == 0 {
                            execute!(
                                session.stderr,
                                style::Print("No to-do lists to show"),
                            );
                        } else {
                            let selection = FuzzySelect::new()
                                .with_prompt("Select task:")
                                .items(&entries)
                                .report(false)
                                .interact_opt()
                                .unwrap_or(None); // FIX: workaround for ^C during selection

                            if let Some(index) = selection {
                                if index < entries.len() {
                                    execute!(
                                        session.stderr,
                                        style::Print("⟳ Resuming: ".magenta()),
                                        style::Print(format!("{}\n", entries[index].description.clone())),
                                    );
                                    return session.resume_todo(os, entries[index].path.clone()).await;
                                }
                            }
                        }
                    }
                    Err(e) => println!("{:?}", e),
                };
            },
        };
        Ok(ChatState::PromptUser { skip_printing_tools: true })
    }

    async fn get_descriptions_and_statuses(self, os: &Os) -> Result<Vec<TodoDisplayEntry>> {
        let mut out = Vec::new();
        let mut entries = os.fs.read_dir(
            build_path(os, TODO_STATE_FOLDER_PATH, "")?
        ).await?;

        while let Some(entry) = entries.next_entry().await? {
            let contents = os.fs.read_to_string(entry.path()).await?;
            let temp_struct = match serde_json::from_str::<TodoState>(&contents) {
                Ok(state) => state,
                Err(_) => continue,
            };
            out.push( TodoDisplayEntry {
                num_completed: temp_struct.completed.iter().filter(|b| **b).count(),
                num_tasks: temp_struct.completed.len(),
                description: prewrap(&temp_struct.task_description),
                path: entry.path(),
            });
        }
        Ok(out)
    }

}


const MAX_LINE_LENGTH: usize = 80;

// FIX: Hacky workaround for cleanly wrapping lines
/// Insert newlines every n characters, not within a word and not at the end.
/// 
/// Generated by Q
fn prewrap(text: &str) -> String {
    if text.is_empty() || MAX_LINE_LENGTH == 0 {
        return text.to_string();
    }
    
    let mut result = String::new();
    let mut current_line_length = 0;
    let words: Vec<&str> = text.split_whitespace().collect();
    
    for (i, word) in words.iter().enumerate() {
        let word_length = word.len();
        
        // If adding this word would exceed the line length and we're not at the start of a line
        if current_line_length > 0 && current_line_length + 1 + word_length > MAX_LINE_LENGTH {
            result.push('\n');
            result.push_str(&" ".repeat("> ".len()));
            current_line_length = 0;
        }
        
        // Add space before word if not at start of line
        if current_line_length > 0 {
            result.push(' ');
            current_line_length += 1;
        }
        
        result.push_str(word);
        current_line_length += word_length;
    }
    
    result
}