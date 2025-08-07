use amzn_codewhisperer_client::types::Model;
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

use crate::api_client::Endpoint;
use crate::cli::chat::{
    ChatError,
    ChatSession,
    ChatState,
};
use crate::os::Os;
#[deny(missing_docs)]
#[derive(Debug, PartialEq, Args)]
pub struct ModelArgs;

impl ModelArgs {
    pub async fn execute(self, os: &Os, session: &mut ChatSession) -> Result<ChatState, ChatError> {
        Ok(select_model(os, session).await?.unwrap_or(ChatState::PromptUser {
            skip_printing_tools: false,
        }))
    }
}

pub async fn select_model(os: &Os, session: &mut ChatSession) -> Result<Option<ChatState>, ChatError> {
    queue!(session.stderr, style::Print("\n"))?;

    // Fetch available models from service
    let (models, _default_model) = get_available_models(os).await?;

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
        .map(|model| {
            let display_name = model.model_name().unwrap_or(model.model_id());

            if Some(model.model_id()) == active_model_id {
                format!("{} (active)", display_name)
            } else {
                display_name.to_owned()
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
        let model_id_str = selected.model_id.clone();
        session.conversation.model = Some(model_id_str.clone());
        let display_name = selected.model_name().unwrap_or(selected.model_id());

        queue!(
            session.stderr,
            style::Print("\n"),
            style::Print(format!(" Using {}\n\n", display_name)),
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

/// Get available models with caching support
pub async fn get_available_models(os: &Os) -> Result<(Vec<Model>, Model), ChatError> {
    let endpoint = Endpoint::configured_value(&os.database);
    let region = endpoint.region().as_ref();

    os.client
        .get_available_models(region)
        .await
        .map_err(|e| ChatError::Custom(format!("Failed to fetch available models: {}", e).into()))
}

/// Returns the context window length in tokens for the given model_id.
/// Uses cached model data when available
pub async fn context_window_tokens(model_id: Option<&str>, os: &Os) -> usize {
    const DEFAULT_CONTEXT_WINDOW_LENGTH: usize = 200_000;

    // If no model_id provided, return default
    let Some(model_id) = model_id else {
        return DEFAULT_CONTEXT_WINDOW_LENGTH;
    };

    // Try to get from cached models first
    let (models, _) = match get_available_models(os).await {
        Ok(models) => models,
        Err(_) => {
            // If we can't get models, return default
            return DEFAULT_CONTEXT_WINDOW_LENGTH;
        },
    };

    models
        .iter()
        .find(|m| m.model_id() == model_id)
        .and_then(|m| m.token_limits())
        .and_then(|limits| limits.max_input_tokens())
        .map(|tokens| tokens as usize)
        .unwrap_or(DEFAULT_CONTEXT_WINDOW_LENGTH)
}
