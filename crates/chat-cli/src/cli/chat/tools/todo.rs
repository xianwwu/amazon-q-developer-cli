#![allow(warnings)]

use core::task;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use std::io::Write;
use serde::{
    Deserialize,
    Serialize
};

use crossterm::{
    execute,
    queue,
    style,
};

use eyre::{
    Result,
    bail,
};
use uuid::timestamp::context;

use super::{
    InvokeOutput,
    MAX_TOOL_RESPONSE_SIZE,
    OutputKind,
};

use crate::os::Os;

/*
Demo prompts:
Create a Python package layout with a blank main file and a blank utilities file. Start by making a todo list.
Design your own super simple programming task with 4 steps. Make a todo list for the task, and begin executing those steps.
*/

// ########### HARDCODED VALUES ##############
pub const CURRENT_TODO_STATE_PATH: &str = ".aws/amazonq/todos/CURRENT_STATE.txt";
pub const TODO_STATE_FOLDER_PATH: &str = ".aws/amazonq/todos";
// ########### --------------------- ##############

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "command")]
pub enum TodoInput {

    #[serde(rename = "create")]
    Create { 
        tasks: Vec<String>,
        task_description: String,
    },
    
    #[serde(rename = "complete")]
    Complete { 
        completed_indices: Vec<usize>,
        context_update: String,
    },

    #[serde(rename = "load")]
    Load {
        path: String,
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct TodoState {
    pub tasks: Vec<String>,
    pub completed: Vec<bool>,
    pub task_description: String,
    pub context: String,
}

impl TodoState {

    /// Loads a TodoState from the given path
    pub async fn load(os: &Os, path: &str) -> Result<Self> {
        if os.fs.exists(path) {
            let json_str = os.fs.read_to_string(path).await?;
            match serde_json::from_str::<Self>(&json_str) {
                Ok(state_struct) => Ok(state_struct),
                Err(_) => bail!("File is not a valid TodoState"),
            }
        } else {
            bail!("File does not exist");
        }
    }

    /// Saves this TodoState to the given path
    pub async fn save(&self, os: &Os, path: &str) -> Result<()> {
        if !os.fs.exists(path) {
            os.fs.create_new(path).await?;
        }
        let json_str = serde_json::to_string(self)?;
        os.fs.write(path, json_str).await?;
        Ok(())
    }

    /// Displays the TodoState as a to-do list
    pub fn display_list(&self, output: &mut impl Write) -> Result<()> {
        queue!(
            output,
            style::Print("TODO:\n"),
        )?;
        for (index, (task, completed)) in self.tasks.iter().zip(self.completed.iter()).enumerate() {
            TodoState::queue_next_without_newline(output, task.clone(), *completed)?;
            if index < self.tasks.len() - 1 {
                queue!(output, style::Print("\n"))?;
            }
        }
        Ok(())
    }

    /// Displays a single empty or marked off to-do list task depending on 
    /// the completion status
    fn queue_next_without_newline(
        output: &mut impl Write, 
        task: String, 
        completed: bool) -> Result<()> {
        if completed {
            queue!(
                output, 
                style::SetAttribute(style::Attribute::Italic),
                style::SetForegroundColor(style::Color::Green),
                style::Print(" ■ "),
                style::SetForegroundColor(style::Color::DarkGrey),
                // style::SetAttribute(style::Attribute::CrossedOut),
                style::Print(format!("{}", task)),
                // style::SetAttribute(style::Attribute::NotCrossedOut),
                style::SetAttribute(style::Attribute::NoItalic),
            )?;
        } else {
            queue!(
                output, 
                style::SetForegroundColor(style::Color::Reset),
                style::Print(format!(" ☐ {}", task)),
            )?;
        }
        Ok(())
    }

    /// Gets the current to-do list path from the fixed state file
    pub async fn get_current_todo_path(os: &Os) -> Result<Option<String>> {
        let temp = build_path(os, CURRENT_TODO_STATE_PATH, "")?;
        let path = os.fs.read_to_string(
            build_path(os, CURRENT_TODO_STATE_PATH, "")?
        ).await?;
        if path.len() > 0 {
            return Ok(Some(path));
        }
        Ok(None)
    }

    /// Sets the current to-do list path in the fixed state file
    pub async fn set_current_todo_path(os: &Os, path: &str) -> Result<()> {;
        os.fs.write(
            build_path(os, CURRENT_TODO_STATE_PATH, "")?, 
            path
        ).await?;
        Ok(())
    }

    /// Generates a new unique filename to be used for new to-do lists
    pub fn generate_new_filename(prefix: &str, extension: &str) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();
        
        format!("{}{}{}", prefix, timestamp, extension)
    }
}


impl TodoInput {
    pub async fn invoke(&self, os: &Os, output: &mut impl Write) -> Result<InvokeOutput> {
        let state = match self {
            TodoInput::Create { tasks, task_description } => {
                let state = TodoState {
                    tasks: tasks.clone(),
                    completed: vec![false; tasks.len()],
                    task_description: task_description.clone(),
                    context: String::new(),
                };
                let path = build_path(
                    os, 
                    TODO_STATE_FOLDER_PATH, 
                    &TodoState::generate_new_filename("todo", ".json")
                )?;
                state.save(os, &path.to_string_lossy()).await?;
                TodoState::set_current_todo_path(os, &path.to_string_lossy()).await?;
                state
            },
            TodoInput::Complete { completed_indices, context_update} => {
                let current_path = match TodoState::get_current_todo_path(os).await? {
                    Some(path) => path,
                    None => bail!("No todo list is currently loaded"),
                };
                let mut state = TodoState::load(os, &current_path).await?;
                completed_indices.iter().for_each(|i| {
                    state.completed[*i] = true;
                });
                state.context = context_update.clone();
                state.save(os, &current_path).await?;
                state
            },
            TodoInput::Load { path } => { 
                let mut state = TodoState::load(os, &path).await?;
                TodoState::set_current_todo_path(os, path).await?;
                state
            }  
        };
        state.display_list(output)?;
        output.flush()?;

        Ok(Default::default())
    }

    pub fn queue_description(&self, os: &Os, output: &mut impl Write) -> Result<()> {
        Ok(())
    }

    pub async fn validate(&mut self, os: &Os) -> Result<()> {
        match self {
            TodoInput::Create { tasks, task_description } => {
                if tasks.len() == 0 {
                    bail!("No tasks were provided");
                } else if task_description.is_empty() {
                    bail!("No task description was provided");
                }
            }
            TodoInput::Complete { completed_indices, context_update } => {
                let current_path = match TodoState::get_current_todo_path(os).await? {
                    Some(path) => path,
                    None => bail!("No todo list is currently loaded"),
                };
                let mut state = TodoState::load(os, &current_path).await?;
                if completed_indices.iter().any(|i| *i > state.completed.len()) {
                    bail!("Completed index is out of bounds");
                }
            }
            TodoInput::Load { path } => {
                if !os.fs.exists(&path) {
                    bail!("Path does not exist");
                } else if let Ok(state) = TodoState::load(os, &path).await {
                    if state.tasks.len() == 0 {
                        bail!("Loaded todo list is empty");
                    }
                } else {
                    bail!("Could not load todo list");
                }
            }
        }
        Ok(())
    }
}

/// Builds an absolute paths from the two given parts
pub fn build_path(os: &Os, part1: &str, part2: &str) -> Result<PathBuf> {
    if let Some(home_dir) = os.env.home() {
        let mut path = PathBuf::new();
        path.push(home_dir);
        path.push(part1);
       
        // Only push part2 if it's non-empty to avoid trailing slashes on files
        if part2.len() > 0 {
            path.push(part2);
        }
        Ok(path)
    } else {
        bail!("Could not determine home directory");
    }
}
