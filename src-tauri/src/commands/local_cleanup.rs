use crate::local_cleanup::{self, CleanupModelStatus};
use tauri::AppHandle;

/// Install state of the mandatory S1-mini text clean-up model.
#[tauri::command]
#[specta::specta]
pub async fn get_cleanup_model_status(app_handle: AppHandle) -> Result<CleanupModelStatus, String> {
    Ok(local_cleanup::status(&app_handle))
}

/// Explicit user-driven install (onboarding page or dashboard card). Progress
/// streams via `cleanup-model-progress`; completion/failure via
/// `cleanup-model-complete` / `cleanup-model-failed`.
#[tauri::command]
#[specta::specta]
pub async fn install_cleanup_model(app_handle: AppHandle) -> Result<(), String> {
    local_cleanup::install(app_handle);
    Ok(())
}
