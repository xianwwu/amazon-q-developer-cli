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
};

use super::{
    InvokeOutput,
    MAX_TOOL_RESPONSE_SIZE,
    OutputKind,
};

use crate::os::Os;

const TODO_STATE_PATH: &str = "todo_state.json";
/*
Prompts that kinda work:

Make a simple todo list for writing a hello world program in C
Execute the steps for me and mark them off on the todo list after you complete each one (as you go).

Design a multi-file compiler for a small language. In each file, include minimal skeleton code for implementing the compiler.
 */

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "command")]

pub enum TodoInput {

    #[serde(rename = "create")]
    Create { tasks: Vec<String> },

    // #[serde(rename = "add")]
    // Add { new_task: String },
    
    #[serde(rename = "complete")]
    Complete { completed_indices: Vec<usize> },
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TodoState {
    pub tasks: Vec<String>,
    pub completed: Vec<bool>,
}

impl TodoState {
    pub async fn load(os: &Os) -> Result<Self> {
        if os.fs.exists(TODO_STATE_PATH) {
            let json_str = os.fs.read_to_string(TODO_STATE_PATH).await?;
            match serde_json::from_str::<Self>(&json_str) {
                Ok(state_struct) => Ok(state_struct),
                Err(_) => Ok(Self::default())
            }
        } else {
            Ok(Self::default())
        }
    }

    pub async fn save(&self, os: &Os) -> Result<()> {
        if !os.fs.exists(TODO_STATE_PATH) {
            os.fs.create_new(TODO_STATE_PATH).await?;
        }
        let json_str = serde_json::to_string(self)?;
        os.fs.write(TODO_STATE_PATH, json_str).await?;
        Ok(())
    }

    pub fn display_list(&self, output: &mut impl Write) -> Result<()> {
        queue!(
            output,
            style::Print("TODO:\n"),
        )?;
        for (task, completed) in self.tasks.iter().zip(self.completed.iter()) {
            TodoState::queue_next(output, task.clone(), *completed)?;
        }
        Ok(())
    }

    fn queue_next(output: &mut impl Write, task: String, completed: bool) -> Result<()> {
        if completed {
            queue!(
                output, 
                style::SetAttribute(style::Attribute::Italic),
                style::SetForegroundColor(style::Color::Green),
                style::Print("  ■ "),
                style::SetForegroundColor(style::Color::DarkGrey),
                // style::SetAttribute(style::Attribute::CrossedOut),
                style::Print(format!("{}\n", task)),
                // style::SetAttribute(style::Attribute::NotCrossedOut),
                style::SetAttribute(style::Attribute::NoItalic),
            )?;
        } else {
            queue!(
                output, 
                style::SetForegroundColor(style::Color::Reset),
                style::Print(format!("  ☐ {}\n", task)),
            )?;
        }
        Ok(())
    }
}


impl TodoInput {
    pub async fn invoke(&self, os: &Os, output: &mut impl Write) -> Result<InvokeOutput> {
        let mut state = TodoState::load(os).await?;
        match self {
            TodoInput::Create { tasks } => {
                state.tasks = tasks.clone();
                state.completed = vec![false; state.tasks.len()];
            },
            TodoInput::Complete { completed_indices } => {
                completed_indices.iter().for_each(|i| {
                    if *i > state.completed.len() {
                        return ();
                    }
                    state.completed[*i as usize] = true;
                });
            }
        };
        state.display_list(output)?;
        output.flush()?;
        state.save(os).await?;
        Ok(Default::default())
        // execute!(
        //     output,
        //     style::Print("Q requested to use the TODO tool!"),
        // )?;
        // panic!("panicked");
    }

    pub fn queue_description(&self, os: &Os, output: &mut impl Write) -> Result<()> {
        Ok(())
    }

    pub async fn validate(&mut self, os: &Os) -> Result<()> {
        Ok(())
    }
}
