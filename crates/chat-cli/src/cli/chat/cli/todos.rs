use clap::Subcommand;
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

use eyre::Result;

use dialoguer::{
    FuzzySelect
};

#[derive(Debug, PartialEq, Subcommand)]
pub enum TodoSubcommand {
    Show,
    ClearFinished,
    Select,
}

/// Used for displaying completed and in-progress todo lists
pub struct TodoDisplayEntry {
    pub num_completed: usize,
    pub num_tasks: usize,
    pub description: String,
    pub id: String,
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
                match self.get_descriptions_and_statuses(os) {
                    Ok(entries) => {
                        if entries.len() == 0 {
                            execute!(
                                session.stderr,
                                style::Print("No to-do lists to show"),
                            )?;
                        }
                        for e in entries {
                            execute!(
                                session.stderr,
                                style::Print(e),
                                style::Print("\n"),
                            )?;
                        }
                    }
                    Err(_) => { 
                        execute!(
                            session.stderr,
                            style::Print("Could not show to-do lists"),
                        )?; 
                    }
                }
            },
            Self::ClearFinished => {
                ();
            },
            Self::Select => {
                match self.get_descriptions_and_statuses(os) {
                    Ok(entries) => {
                        if entries.len() == 0 {
                            execute!(
                                session.stderr,
                                style::Print("No to-do lists to show"),
                            )?;
                        } else {
                            let selection = FuzzySelect::new()
                                .with_prompt("Select task:")
                                .items(&entries)
                                .report(false)
                                .interact_opt()
                                .unwrap_or(None); 

                            if let Some(index) = selection {
                                if index < entries.len() {
                                    execute!(
                                        session.stderr,
                                        style::Print("⟳ Resuming: ".magenta()),
                                        style::Print(format!("{}\n", entries[index].description.clone())),
                                    )?;
                                    return session.resume_todo(os, &entries[index].id).await;
                                }
                            }
                        }
                    }
                    // %%% FIX %%%
                    Err(e) => println!("{:?}", e),
                    // %%% --- %%%
                };
            },
        };
        Ok(ChatState::PromptUser { skip_printing_tools: true })
    }

    /// Convert all to-do list state files to displayable entries
    fn get_descriptions_and_statuses(self, os: &Os) -> Result<Vec<TodoDisplayEntry>> {
        let mut out = Vec::new();
        let entries = os.database.get_all_todos()?;
        for (id, value) in entries.iter() {
            let temp_struct = match value.as_str() {
                Some(s) => { match serde_json::from_str::<TodoState>(s) {
                    Ok(state) => state,
                    Err(_) => continue,
                }},
                None => continue,
            };
            // For some reason this doesn't work
            // Has to do with the Value::String wrapping in os.database.all_entries() rather than Value::from_str()
            // let temp_struct = match serde_json::from_value::<TodoState>(value.clone()) {
            //     Ok(state) => state,
            //     Err(_) => continue,
            // };

            out.push(TodoDisplayEntry {
                num_completed: temp_struct.completed.iter().filter(|b| **b).count(),
                num_tasks: temp_struct.completed.len(),
                description: prewrap(&temp_struct.task_description),
                id: id.to_string(),
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
    
    for (_, word) in words.iter().enumerate() {
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