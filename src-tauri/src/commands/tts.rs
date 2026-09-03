use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::managers::tts::{TtsDownloadProgress, TtsStatus, TtsVoice};

#[tauri::command]
#[specta::specta]
pub async fn tts_status(
    app_handle: AppHandle,
    tts_manager: State<'_, Arc<crate::managers::tts::TtsManager>>,
) -> Result<TtsStatus, String> {
    Ok(tts_manager.status(&app_handle))
}

#[tauri::command]
#[specta::specta]
pub async fn tts_download_model(
    app_handle: AppHandle,
    tts_manager: State<'_, Arc<crate::managers::tts::TtsManager>>,
) -> Result<(), String> {
    tts_manager
        .download_model(app_handle)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn tts_synthesize(
    app_handle: AppHandle,
    tts_manager: State<'_, Arc<crate::managers::tts::TtsManager>>,
    text: String,
) -> Result<String, String> {
    let manager = tts_manager.inner().clone();
    let handle = app_handle.clone();
    // Synthesis is blocking (spawns crispasr subprocess) — keep it off the async runtime
    let path = tokio::task::spawn_blocking(move || manager.synthesize(&handle, text))
        .await
        .map_err(|e| format!("synthesize task failed: {e}"))?
        .map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

/// Pocket-TTS preset voices (Kyutai catalog) pulled from the real backend.
#[tauri::command]
#[specta::specta]
pub async fn tts_voices() -> Result<Vec<TtsVoice>, String> {
    Ok(crate::managers::tts::TtsManager::voices())
}

/// Currently selected Pocket-TTS voice id (persisted locally).
#[tauri::command]
#[specta::specta]
pub async fn tts_selected_voice(
    tts_manager: State<'_, Arc<crate::managers::tts::TtsManager>>,
) -> Result<String, String> {
    Ok(tts_manager.selected_voice())
}

/// Persist the selected Pocket-TTS voice. Synthesis uses it from here.
#[tauri::command]
#[specta::specta]
pub async fn tts_set_voice(
    tts_manager: State<'_, Arc<crate::managers::tts::TtsManager>>,
    voice: String,
) -> Result<(), String> {
    tts_manager.set_voice(voice).map_err(|e| e.to_string())
}

/// Expose the progress event name for specta generation (frontend can listen to it)
#[tauri::command]
#[specta::specta]
pub fn tts_download_progress_event() -> String {
    crate::managers::tts::progress_event_name().to_string()
}

// Keep type usage for specta generation
#[allow(dead_code)]
fn _ensure_types(_p: TtsDownloadProgress, _s: TtsStatus) {}
