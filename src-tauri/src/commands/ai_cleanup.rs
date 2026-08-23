use crate::ai_cleanup;
use crate::managers::history::{AiCleanupHistoryEntry, HistoryManager};
use crate::settings::{self, AiCleanupThinkingLevel};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

const MAX_CUSTOM_INSTRUCTION_CHARS: usize = 4_000;
const MAX_CONTEXTS: usize = 12;
const MAX_CONTEXT_CHARS: usize = 12_000;

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct AiCleanupConfiguration {
    pub enabled: bool,
    pub auto_enabled: bool,
    pub model: String,
    pub thinking_level: AiCleanupThinkingLevel,
    pub custom_instruction: String,
    pub contexts: Vec<String>,
}

#[tauri::command]
#[specta::specta]
pub fn update_ai_cleanup_configuration(
    app: AppHandle,
    configuration: AiCleanupConfiguration,
) -> Result<(), String> {
    ai_cleanup::validate_model_and_thinking(&configuration.model, configuration.thinking_level)?;
    if configuration.custom_instruction.chars().count() > MAX_CUSTOM_INSTRUCTION_CHARS {
        return Err("Custom instruction is too long".to_string());
    }
    if configuration.contexts.len() > MAX_CONTEXTS
        || configuration
            .contexts
            .iter()
            .any(|context| context.chars().count() > MAX_CONTEXT_CHARS)
    {
        return Err("Context exceeds the supported limit".to_string());
    }

    let mut current = settings::get_settings(&app);
    let enabled_changed = current.ai_cleanup_enabled != configuration.enabled;
    if enabled_changed {
        if let Some(binding) = current.bindings.get("ai_cleanup").cloned() {
            if configuration.enabled {
                crate::shortcut::register_shortcut(&app, binding)?;
            } else {
                crate::shortcut::unregister_shortcut(&app, binding)?;
            }
        }
    }

    current.ai_cleanup_enabled = configuration.enabled;
    current.auto_ai_cleanup_enabled = configuration.auto_enabled;
    current.ai_cleanup_model = configuration.model;
    current.ai_cleanup_thinking_level = configuration.thinking_level;
    current.ai_cleanup_custom_instruction = configuration.custom_instruction;
    current.ai_cleanup_contexts = configuration.contexts;
    settings::write_settings(&app, current.clone());

    if enabled_changed {
        crate::secure_input::reconcile_fallback(&app);
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn get_ai_cleanup_history(
    app: AppHandle,
    limit: usize,
) -> Result<Vec<AiCleanupHistoryEntry>, String> {
    app.state::<Arc<HistoryManager>>()
        .get_ai_cleanup_history(limit)
        .map_err(|error| format!("Failed to load AI cleanup history: {error}"))
}

#[tauri::command]
#[specta::specta]
pub fn set_gemini_api_key(api_key: String) -> Result<(), String> {
    ai_cleanup::credentials::save(&api_key)
}

#[tauri::command]
#[specta::specta]
pub fn is_gemini_api_configured() -> Result<bool, String> {
    ai_cleanup::credentials::is_configured()
}
