mod client;
pub(crate) mod credentials;
pub mod prompt;
mod prompts;

use crate::settings::{AiCleanupThinkingLevel, AppSettings};

pub const MODELS: &[&str] = &[
    "gemini-3.5-flash-lite",
    "gemini-3.5-flash",
    "gemini-3.7-flash",
    "gemini-3.1-pro-preview",
];

pub fn thinking_level_name(level: AiCleanupThinkingLevel) -> &'static str {
    match level {
        AiCleanupThinkingLevel::Minimal => "minimal",
        AiCleanupThinkingLevel::Low => "low",
        AiCleanupThinkingLevel::Medium => "medium",
        AiCleanupThinkingLevel::High => "high",
    }
}

pub fn is_missing_api_key_error(error: &str) -> bool {
    error == credentials::MISSING_API_KEY_ERROR
}

pub fn validate_model_and_thinking(
    model: &str,
    thinking: AiCleanupThinkingLevel,
) -> Result<(), String> {
    if !MODELS.contains(&model) {
        return Err("Unsupported Gemini model".to_string());
    }
    if model == "gemini-3.1-pro-preview" && thinking == AiCleanupThinkingLevel::Minimal {
        return Err("Gemini Pro supports low, medium, or high thinking".to_string());
    }
    Ok(())
}

pub async fn clean(input: &str, settings: &AppSettings) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("No text was provided".to_string());
    }
    validate_model_and_thinking(
        &settings.ai_cleanup_model,
        settings.ai_cleanup_thinking_level,
    )?;
    let api_key = credentials::load(settings)?;

    client::generate(
        &api_key,
        &settings.ai_cleanup_model,
        settings.ai_cleanup_thinking_level,
        prompt::SYSTEM_PROMPT,
        &prompt::build_user_content(input, settings),
    )
    .await
}

pub async fn edit(
    selected_text: &str,
    instruction: &str,
    settings: &AppSettings,
) -> Result<String, String> {
    if selected_text.trim().is_empty() {
        return Err("No text was selected".to_string());
    }
    if instruction.trim().is_empty() {
        return Err("No edit instruction was provided".to_string());
    }
    validate_model_and_thinking(
        &settings.ai_cleanup_model,
        settings.ai_cleanup_thinking_level,
    )?;
    let api_key = credentials::load(settings)?;

    client::generate(
        &api_key,
        &settings.ai_cleanup_model,
        settings.ai_cleanup_thinking_level,
        prompt::EDIT_SYSTEM_PROMPT,
        &prompt::build_edit_user_content(selected_text, instruction.trim()),
    )
    .await
}

pub async fn execute_with_page_context(
    instruction: &str,
    snapshot: Option<&crate::context::types::ContextSnapshot>,
    settings: &AppSettings,
) -> Result<String, String> {
    let instruction = instruction.trim();
    if instruction.is_empty() {
        return Err("No voice instruction was provided".to_string());
    }
    validate_model_and_thinking(
        &settings.ai_cleanup_model,
        settings.ai_cleanup_thinking_level,
    )?;
    let api_key = credentials::load(settings)?;

    let page_context = snapshot.map(page_context_for_prompt);
    let system_prompt = prompts::hey_superflow::superflow_system_prompt(
        &settings.hey_superflow_tone,
        &superflow_personal_data(settings),
    );
    client::generate(
        &api_key,
        &settings.ai_cleanup_model,
        settings.ai_cleanup_thinking_level,
        &system_prompt,
        &prompt::build_edit_user_content(page_context.as_deref().unwrap_or(""), instruction),
    )
    .await
}

fn page_context_for_prompt(snapshot: &crate::context::types::ContextSnapshot) -> String {
    format!(
        "Application: {}\nSurface: {}\nTitle: {}\nURL: {}\n\nVisible page content:\n{}",
        snapshot.app_name,
        snapshot.surface.as_str(),
        snapshot.title.as_deref().unwrap_or("unknown"),
        snapshot.url.as_deref().unwrap_or("unknown"),
        snapshot.focused_text.as_deref().unwrap_or("unavailable")
    )
}

/// Compose the personal-data "memory" block for Hey Superflow: the user's
/// local specification plus any saved AI-cleanup contexts. Both are small,
/// local, and surfaced in settings, so they are safe to include by default.
fn superflow_personal_data(settings: &AppSettings) -> String {
    let mut data = String::new();
    let spec = settings.user_specification.trim();
    if !spec.is_empty() {
        data.push_str(spec);
    }
    for context in &settings.ai_cleanup_contexts {
        let context = context.trim();
        if context.is_empty() {
            continue;
        }
        if !data.is_empty() {
            data.push('\n');
        }
        data.push_str(context);
    }
    data
}

#[cfg(test)]
mod page_context_tests {
    use super::*;
    use crate::context::types::{ContextSnapshot, Surface};

    #[test]
    fn packages_real_page_context_separately_from_the_instruction() {
        let snapshot = ContextSnapshot {
            surface: Surface::Gmail,
            app_name: "Google Chrome".to_string(),
            bundle_id: Some("com.google.Chrome".to_string()),
            url: Some("https://mail.google.com/".to_string()),
            title: Some("New sign-in to your OpenAI account".to_string()),
            focused_text: Some("If this was you, no action is needed.".to_string()),
            captured_at_ms: 1,
        };
        let context = page_context_for_prompt(&snapshot);
        assert!(context.contains("Surface: gmail"));
        assert!(context.contains("New sign-in to your OpenAI account"));
        assert!(context.contains("If this was you, no action is needed."));
    }
}

pub(crate) async fn generate_with_system_prompt(
    system_prompt: &str,
    user_content: &str,
    settings: &AppSettings,
) -> Result<String, String> {
    validate_model_and_thinking(
        &settings.ai_cleanup_model,
        settings.ai_cleanup_thinking_level,
    )?;
    let api_key = credentials::load(settings)?;
    client::generate(
        &api_key,
        &settings.ai_cleanup_model,
        settings.ai_cleanup_thinking_level,
        system_prompt,
        user_content,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pro_rejects_minimal_thinking() {
        assert!(validate_model_and_thinking(
            "gemini-3.1-pro-preview",
            AiCleanupThinkingLevel::Minimal
        )
        .is_err());
        assert!(
            validate_model_and_thinking("gemini-3.1-pro-preview", AiCleanupThinkingLevel::Low)
                .is_ok()
        );
    }

    #[test]
    fn voice_instruction_uses_the_edit_prompt_input_shape_without_source_text() {
        assert_eq!(
            prompt::build_edit_user_content("", "Draft an email to Emma."),
            "<selected-text>\n\n</selected-text>\n\n<edit-instruction>\nDraft an email to Emma.\n</edit-instruction>"
        );
    }
}
