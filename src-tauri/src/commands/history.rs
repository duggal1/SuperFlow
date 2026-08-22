use crate::actions::process_transcription_output;
use crate::audio_feedback::{play_feedback_sound, SoundType};
use crate::managers::{
    history::{HistoryManager, PaginatedHistory},
    transcription::TranscriptionManager,
};
use log::{debug, info, warn};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

/// Total attempts per restore, including the immediate first one. The first
/// attempt runs right away (the common transient failure recovers instantly);
/// later attempts get a short backoff to let a model reload or a still-running
/// stream worker release the engine.
const RETRANSCRIBE_ATTEMPTS: usize = 3;

/// Attempts for the final fallback layer: transcribing with a *different*
/// downloaded model after the selected model failed every retry. Two tries
/// absorb a flaky first load without dragging recovery out.
const FALLBACK_ATTEMPTS: usize = 2;

/// Store a recovered transcription (with its post-processing pass) on an
/// existing history entry. Shared by both recovery layers.
async fn persist_recovered_transcription(
    app: &AppHandle,
    history_manager: &HistoryManager,
    id: i64,
    post_process_requested: bool,
    transcription: String,
) -> Result<(), String> {
    let processed = process_transcription_output(app, &transcription, post_process_requested).await;
    history_manager
        .update_transcription(
            id,
            transcription,
            processed.post_processed_text,
            processed.post_process_prompt,
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Put the user's selected model back into the engine slot after the fallback
/// layer swapped in a substitute. Their selection is never rewritten — this
/// only restores the runtime so future dictations use their choice again.
/// Waits out an active dictation briefly instead of yanking the engine from a
/// live stream; if the selected model fails to come back (the usual reason a
/// fallback was needed), the working substitute stays loaded and the warning
/// is logged.
fn restore_selected_model_later(app: AppHandle, selected_model: String) {
    tauri::async_runtime::spawn(async move {
        for _ in 0..2 {
            if !app.state::<Arc<TranscriptionManager>>().is_streaming() {
                break;
            }
            let _ =
                tauri::async_runtime::spawn_blocking(|| std::thread::sleep(Duration::from_secs(5)))
                    .await;
        }
        let tm = Arc::clone(&*app.state::<Arc<TranscriptionManager>>());
        let model = selected_model.clone();
        match tauri::async_runtime::spawn_blocking(move || tm.ensure_model_loaded(&model)).await {
            Ok(Ok(())) => debug!("Restored selected model after fallback"),
            Ok(Err(e)) => warn!(
                "Selected model '{}' could not be restored after fallback: {}",
                selected_model, e
            ),
            Err(e) => warn!("Model restore task panicked: {}", e),
        }
    });
}

/// Recover a failed transcription through the full three-layer chain, all
/// against the always-present saved WAV file:
///
/// 1. *(live)* the original dictation attempt — handled in `actions.rs`;
/// 2. fresh-load retries of the selected model (`RETRANSCRIBE_ATTEMPTS`);
/// 3. a different downloaded model entirely (`FALLBACK_ATTEMPTS`) when the
///    selected model itself is the problem.
///
/// Emits `history-retranscribe` events (`started` / `fallback` / `completed` /
/// `failed`, plus the substitute model name on `fallback`) so the UI reflects
/// each stage in real time, plays the stop sound when recovery lands, and
/// updates the stored entry on success (which also emits the typed history
/// update event every open page already listens for).
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

    // One chain targets exactly one intended model: read the selection once
    // up front so retries and any engine restoration stay coherent even if
    // the user changes preferences mid-recovery.
    let selected_model = crate::settings::get_settings(&app).selected_model;
    if selected_model.trim().is_empty() {
        emit_status("failed");
        return Err("No transcription model selected".to_string());
    }

    emit_status("started");

    // ------------------------------------------------------------------
    // Layer 2 — the selected model again, freshly loaded each time.
    // ------------------------------------------------------------------
    let mut last_error = String::new();
    for attempt in 1..=RETRANSCRIBE_ATTEMPTS {
        if attempt > 1 {
            let delay = Duration::from_millis(1000 * attempt as u64);
            let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(delay)).await;
        }

        let tm = Arc::clone(&transcription_manager);
        let model_id = selected_model.clone();
        let attempt_samples = samples.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            tm.ensure_model_loaded(&model_id)?;
            tm.transcribe(attempt_samples)
        })
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
                persist_recovered_transcription(
                    &app,
                    &history_manager,
                    id,
                    entry.post_process_requested,
                    transcription,
                )
                .await?;
                emit_status("completed");
                play_feedback_sound(&app, SoundType::Stop);
                return Ok(());
            }
        }
    }

    // ------------------------------------------------------------------
    // Layer 3 — the selected model is persistently broken (every fresh
    // reload failed), so hand the audio to a different downloaded model.
    // The user's selection stays untouched; the engine is restored after.
    // ------------------------------------------------------------------
    let Some((fallback_id, fallback_name)) =
        transcription_manager.fallback_model_candidate(&selected_model)
    else {
        info!(
            "No alternate downloaded model available; entry {} stays failed ({})",
            id, last_error
        );
        emit_status("failed");
        return Err(last_error);
    };

    let _ = app.emit(
        "history-retranscribe",
        json!({ "id": id, "status": "fallback", "model": fallback_name }),
    );
    info!(
        "Falling back to model '{}' for entry {} after selected model failed: {}",
        fallback_id, id, last_error
    );

    for attempt in 1..=FALLBACK_ATTEMPTS {
        if attempt > 1 {
            let _ = tauri::async_runtime::spawn_blocking(|| {
                std::thread::sleep(Duration::from_millis(1500))
            })
            .await;
        }

        let tm = Arc::clone(&transcription_manager);
        let model_id = fallback_id.clone();
        let attempt_samples = samples.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            tm.ensure_model_loaded(&model_id)?;
            tm.transcribe(attempt_samples)
        })
        .await
        .map_err(|e| format!("Fallback transcription task panicked: {}", e))?;

        match result {
            Err(e) => {
                warn!(
                    "Fallback attempt {}/{} with '{}' failed for entry {}: {}",
                    attempt, FALLBACK_ATTEMPTS, fallback_id, id, e
                );
                last_error = e.to_string();
            }
            Ok(transcription) if transcription.trim().is_empty() => {
                last_error = "Recording contains no speech".to_string();
            }
            Ok(transcription) => {
                persist_recovered_transcription(
                    &app,
                    &history_manager,
                    id,
                    entry.post_process_requested,
                    transcription,
                )
                .await?;
                emit_status("completed");
                play_feedback_sound(&app, SoundType::Stop);
                restore_selected_model_later(app, selected_model);
                return Ok(());
            }
        }
    }

    emit_status("failed");
    restore_selected_model_later(app, selected_model);
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
