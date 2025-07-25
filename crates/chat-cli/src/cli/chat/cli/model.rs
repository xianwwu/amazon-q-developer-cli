use clap::Args;
use crossterm::style::{
    self,
    Color,
};
use crossterm::{
    execute,
    queue,
};
use dialoguer::Select;

use crate::auth::builder_id::{
    BuilderIdToken,
    TokenType,
};
use crate::cli::chat::{
    ChatError,
    ChatSession,
    ChatState,
};
use crate::os::Os;

// pub struct ModelOption {
//     pub name: &'static str,
//     pub model_id: &'static str,
// }

// pub const MODEL_OPTIONS: [ModelOption; 2] = [
//     ModelOption {
//         name: "claude-4-sonnet",
//         model_id: "CLAUDE_SONNET_4_20250514_V1_0",
//     },
//     ModelOption {
//         name: "claude-3.7-sonnet",
//         model_id: "CLAUDE_3_7_SONNET_20250219_V1_0",
//     },
// ];

#[deny(missing_docs)]
#[derive(Debug, PartialEq, Args)]
pub struct ModelArgs;

impl ModelArgs {
    pub async fn execute(self, os: &mut Os, session: &mut ChatSession) -> Result<ChatState, ChatError> {
        Ok(select_model(os, session).await?.unwrap_or(ChatState::PromptUser {
            skip_printing_tools: false,
        }))
    }
}

pub async fn select_model(os: &mut Os, session: &mut ChatSession) -> Result<Option<ChatState>, ChatError> {
    queue!(session.stderr, style::Print("\n"))?;

    // Fetch available models from service
    let (models, _default_model) = os
        .client
        .list_available_models()
        .await
        .map_err(|e| ChatError::Custom(format!("Failed to fetch available models: {}", e).into()))?;

    if models.is_empty() {
        queue!(
            session.stderr,
            style::SetForegroundColor(Color::Red),
            style::Print("No models available\n"),
            style::ResetColor
        )?;
        return Ok(None);
    }

    let active_model_id = session.conversation.model.as_deref();

    let labels: Vec<String> = models
        .iter()
        .map(|opt| {
            if Some(opt.model_id.as_str()) == active_model_id {
                format!("{} (active)", opt.model_id)
            } else {
                opt.model_id.to_owned()
            }
        })
        .collect();

    let selection: Option<_> = match Select::with_theme(&crate::util::dialoguer_theme())
        .with_prompt("Select a model for this chat session")
        .items(&labels)
        .default(0)
        .interact_on_opt(&dialoguer::console::Term::stdout())
    {
        Ok(sel) => {
            let _ = crossterm::execute!(
                std::io::stdout(),
                crossterm::style::SetForegroundColor(crossterm::style::Color::Magenta)
            );
            sel
        },
        // Ctrl‑C -> Err(Interrupted)
        Err(dialoguer::Error::IO(ref e)) if e.kind() == std::io::ErrorKind::Interrupted => return Ok(None),
        Err(e) => return Err(ChatError::Custom(format!("Failed to choose model: {e}").into())),
    };

    queue!(session.stderr, style::ResetColor)?;

    if let Some(index) = selection {
        let selected = &models[index];
        let model_id_str = selected.model_id.to_string();
        session.conversation.model = Some(model_id_str);

        queue!(
            session.stderr,
            style::Print("\n"),
            style::Print(format!(" Using {}\n\n", selected.model_id)),
            style::ResetColor,
            style::SetForegroundColor(Color::Reset),
            style::SetBackgroundColor(Color::Reset),
        )?;
    }

    execute!(session.stderr, style::ResetColor)?;

    Ok(Some(ChatState::PromptUser {
        skip_printing_tools: false,
    }))
}

/// Returns Claude 3.7 for: Amazon IDC users, FRA region users
/// Returns Claude 4.0 for: Builder ID users, other regions
pub async fn default_model_id(os: &Os) -> String {
    if let Ok((_, Some(default_model))) = os.client.list_available_models().await {
        return default_model.model_id().to_string();
    }
    // Check FRA region first
    if let Ok(Some(profile)) = os.database.get_auth_profile() {
        if profile.arn.split(':').nth(3) == Some("eu-central-1") {
            return "CLAUDE_3_7_SONNET_20250219_V1_0".to_string();
        }
    }

    // Check if Amazon IDC user
    if let Ok(Some(token)) = BuilderIdToken::load(&os.database).await {
        if matches!(token.token_type(), TokenType::IamIdentityCenter) && token.is_amzn_user() {
            return "CLAUDE_3_7_SONNET_20250219_V1_0".to_string();
        }
    }

    // Default to 4.0
    "CLAUDE_SONNET_4_20250514_V1_0".to_string()
}
