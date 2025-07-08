#![allow(warnings)]

use std::time::{SystemTime, UNIX_EPOCH};

use std::io::Write;
use serde::{
    Deserialize,
    Serialize
};

use crossterm::{
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
/* You should use this tool when appropriate, even if it is not explicitly requested by the user. */

use crate::os::Os;

/*
Prompts that kinda work:

Make a simple todo list for writing a hello world program in C
Execute the steps for me and mark them off on the todo list after you complete each one (as you go).

Design a multi-file compiler for a small language. In each file, include minimal skeleton code for implementing the compiler.

Create a Python package layout with a blank main file and a blank utilities file. Start by making a todo list.

Design your own task with 4 simple steps. Make a todo list for it, and begin executing those steps.
*/

/*
Plan for user input:
- TODOs are automatically saved to todos/ directory
- Users can use /todos to view finished and in-progress todos
- Users can use /todos to select an unfinished todo to complete
    - Selected todo will be 
*/

// ########### REMOVE BEFORE PUSHING ##############
const CURRENT_TODO_STATE_PATH: &str = "/Users/kiranbug/.aws/amazonq/todos/CURRENT_STATE.txt";
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
    pub async fn load(os: &Os, path: &str) -> Result<Self> {
        if os.fs.exists(path) {
            let json_str = os.fs.read_to_string(path).await?;
            match serde_json::from_str::<Self>(&json_str) {
                Ok(state_struct) => Ok(state_struct),
                Err(_) => Ok(Self::default())
            }
        } else {
            Ok(Self::default())
        }
    }

    pub async fn save(&self, os: &Os, path: &str) -> Result<()> {
        if !os.fs.exists(path) {
            os.fs.create_new(path).await?;
        }
        let json_str = serde_json::to_string(self)?;
        os.fs.write(path, json_str).await?;
        Ok(())
    }

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

    fn queue_next_without_newline(output: &mut impl Write, task: String, completed: bool) -> Result<()> {
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

    pub async fn get_current_todo_path(os: &Os) -> Result<Option<String>> {
        let path = os.fs.read_to_string(CURRENT_TODO_STATE_PATH).await?;
        if path.len() > 0 {
            return Ok(Some(path));
        }
        Ok(None)
    }

    pub async fn set_current_todo_path(os: &Os, path: &str) -> Result<()> {
        os.fs.write(CURRENT_TODO_STATE_PATH, path).await?;
        Ok(())
    }

    pub fn generate_new_filename(prefix: &str, extension: &str) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();
        
        format!("{}{}{}", prefix, timestamp, extension)
    }
}


impl TodoInput {
    // ############# CLEANUP #############
    pub async fn invoke(&self, os: &Os, output: &mut impl Write) -> Result<InvokeOutput> {
        
        let state = match self {
            TodoInput::Create { tasks, task_description } => {
                println!("Create has been called!");
                let state = TodoState {
                    tasks: tasks.clone(),
                    completed: vec![false; tasks.len()],
                    task_description: task_description.clone(),
                    context: String::new(),
                };
                let path = TodoState::generate_new_filename("todo", ".json");
                println!("{:?}", path);
                state.save(os, &path).await?;
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
                state
            }
            TodoInput::Load { path } => { 
                let mut state = TodoState::load(os, &path).await?;
                TodoState::set_current_todo_path(os, path);
                state
            }  
        };
        state.display_list(output)?;
        output.flush()?;
        Ok(Default::default())
    }
    // ############# ------- #############

    pub fn queue_description(&self, os: &Os, output: &mut impl Write) -> Result<()> {
        Ok(())
    }

    pub async fn validate(&mut self, os: &Os) -> Result<()> {
        // match self {
        //     TodoInput::Create { tasks, ..} => {
        //         if tasks.len() == 0 || tasks.iter().any(|s| s.is_empty()) {
        //             bail!("Tasks must not be empty");
        //         }
        //     }
        //     TodoInput::Complete { completed_indices, ..} => {
        //         if 
                
        //         if completed_indices.iter().any(|i| i > state.)
        //     }
        // }
        Ok(())
    }
}
