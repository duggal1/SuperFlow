mod client;
pub(crate) mod credentials;
pub mod prompt;

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

pub async fn execute(instruction: &str, settings: &AppSettings) -> Result<String, String> {
    let instruction = instruction.trim();
    if instruction.is_empty() {
        return Err("No voice instruction was provided".to_string());
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
        &prompt::build_edit_user_content("", instruction),
    )
    .await
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
