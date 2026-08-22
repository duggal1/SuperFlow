use crate::actions::process_transcription_output;
use crate::managers::{
    history::{HistoryManager, PaginatedHistory},
    transcription::TranscriptionManager,
};
use log::warn;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

/// Total attempts per restore, including the immediate first one. The first
/// attempt runs right away (the common transient failure recovers instantly);
/// later attempts get a short backoff to let a model reload or a still-running
/// stream worker release the engine.
const RETRANSCRIBE_ATTEMPTS: usize = 3;

/// Re-run transcription for an already-saved recording entirely against the
/// saved WAV file. Emits `history-retranscribe` events (`started` /
/// `completed` / `failed`) so the UI can show a restoring state on the card,
/// and updates the stored entry on success (which also emits the typed
/// history update event every open page already listens for).
pub async fn retranscribe_entry(
    app: AppHandle,
    history_manager: Arc<HistoryManager>,
    transcription_manager: Arc<TranscriptionManager>,
    id: i64,
) -> Result<(), String> {
    let emit_status = |status: &str| {
        let _ = app.emit(
            "history-retranscribe",
            json!({ "id": id, "status": status }),
        );
    };

    let entry = history_manager
        .get_entry_by_id(id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("History entry {} not found", id))?;

    // The audio is always saved, so a failed transcription can always be
    // recovered from disk.
    let audio_path = history_manager.get_audio_file_path(&entry.file_name);
    let samples = crate::audio_toolkit::read_wav_samples(&audio_path)
        .map_err(|e| format!("Failed to load audio: {}", e))?;

    if samples.is_empty() {
        return Err("Recording has no audio samples".to_string());
    }

    emit_status("started");

    let mut last_error = String::new();
    for attempt in 1..=RETRANSCRIBE_ATTEMPTS {
        if attempt > 1 {
            let delay = Duration::from_millis(1000 * attempt as u64);
            let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(delay)).await;
        }

        // Make sure the model is (re)loading; transcribe() waits for any
        // in-progress load before running.
        transcription_manager.initiate_model_load();

        let tm = Arc::clone(&transcription_manager);
        let attempt_samples = samples.clone();
        let result = tauri::async_runtime::spawn_blocking(move || tm.transcribe(attempt_samples))
            .await
            .map_err(|e| format!("Transcription task panicked: {}", e))?;

        match result {
            Err(e) => {
                warn!(
                    "Background re-transcription attempt {}/{} failed for entry {}: {}",
                    attempt, RETRANSCRIBE_ATTEMPTS, id, e
                );
                last_error = e.to_string();
            }
            Ok(transcription) if transcription.trim().is_empty() => {
                last_error = "Recording contains no speech".to_string();
            }
            Ok(transcription) => {
                let processed = process_transcription_output(
                    &app,
                    &transcription,
                    entry.post_process_requested,
                )
                .await;
                history_manager
                    .update_transcription(
                        id,
                        transcription,
                        processed.post_processed_text,
                        processed.post_process_prompt,
                    )
                    .map(|_| ())
                    .map_err(|e| e.to_string())?;
                emit_status("completed");
                return Ok(());
            }
        }
    }

    emit_status("failed");
    Err(last_error)
}

#[tauri::command]
#[specta::specta]
pub async fn get_history_entries(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    cursor: Option<i64>,
    limit: Option<usize>,
) -> Result<PaginatedHistory, String> {
    history_manager
        .get_history_entries(cursor, limit)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn toggle_history_entry_saved(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    id: i64,
) -> Result<(), String> {
    history_manager
        .toggle_saved_status(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_audio_file_path(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    file_name: String,
) -> Result<String, String> {
    let path = history_manager.get_audio_file_path(&file_name);
    path.to_str()
        .ok_or_else(|| "Invalid file path".to_string())
        .map(|s| s.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_history_entry(
    _app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    id: i64,
) -> Result<(), String> {
    history_manager
        .delete_entry(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn retry_history_entry_transcription(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
    id: i64,
) -> Result<(), String> {
    retranscribe_entry(
        app,
        Arc::clone(&history_manager),
        Arc::clone(&transcription_manager),
        id,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn update_history_limit(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    limit: usize,
) -> Result<(), String> {
    let mut settings = crate::settings::get_settings(&app);
    settings.history_limit = limit;
    crate::settings::write_settings(&app, settings);

    history_manager
        .cleanup_old_entries()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn update_recording_retention_period(
    app: AppHandle,
    history_manager: State<'_, Arc<HistoryManager>>,
    period: String,
) -> Result<(), String> {
    use crate::settings::RecordingRetentionPeriod;

    let retention_period = match period.as_str() {
        "never" => RecordingRetentionPeriod::Never,
        "preserve_limit" => RecordingRetentionPeriod::PreserveLimit,
        "days3" => RecordingRetentionPeriod::Days3,
        "weeks2" => RecordingRetentionPeriod::Weeks2,
        "months3" => RecordingRetentionPeriod::Months3,
        _ => return Err(format!("Invalid retention period: {}", period)),
    };

    let mut settings = crate::settings::get_settings(&app);
    settings.recording_retention_period = retention_period;
    crate::settings::write_settings(&app, settings);

    history_manager
        .cleanup_old_entries()
        .map_err(|e| e.to_string())?;

    Ok(())
}
