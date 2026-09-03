use crate::settings::get_settings;
use tauri::{AppHandle, Manager};

#[tauri::command]
#[specta::specta]
pub async fn submit_obsidian_clarification(
    app: AppHandle,
    transcript: String,
) -> Result<(), String> {
    let transcript = transcript.trim().to_string();
    if transcript.is_empty() {
        return Err("Empty clarification".to_string());
    }

    let settings = get_settings(&app);

    match crate::obsidian::handle_obsidian_transcript(&transcript, &settings, Some(&app)).await {
        crate::obsidian::ObsidianHandleResult::NotObsidian => {
            crate::overlay::show_obsidian_clarify_overlay(
                &app,
                "Could you share a bit more detail for this note?".to_string(),
            );
            Ok(())
        }
        crate::obsidian::ObsidianHandleResult::Success(result) => {
            crate::overlay::show_obsidian_success_overlay(&app, &result);
            if let Some(hm) =
                app.try_state::<std::sync::Arc<crate::managers::history::HistoryManager>>()
            {
                let file_name =
                    format!("obsidian-clarified-{}.wav", chrono::Utc::now().timestamp());
                let _ = hm.save_entry(
                    file_name,
                    transcript,
                    false,
                    Some(format!(
                        "Obsidian: {} ({})",
                        result.title, result.task_status
                    )),
                    None,
                    0.0,
                );
            }
            let app2 = app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(3800)).await;
                crate::overlay::hide_recording_overlay(&app2);
                crate::tray::change_tray_icon(&app2, crate::tray::TrayIconState::Idle);
            });
            Ok(())
        }
        crate::obsidian::ObsidianHandleResult::NeedsClarification(clarify) => {
            crate::overlay::show_obsidian_clarify_overlay(&app, clarify.clarification_message);
            Ok(())
        }
        crate::obsidian::ObsidianHandleResult::Failure(err) => {
            crate::overlay::show_obsidian_failure_overlay(&app, err.message.clone());
            log::warn!(
                "Obsidian clarification failed: {} - {}",
                err.error,
                err.message
            );
            let app2 = app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(3200)).await;
                crate::overlay::hide_recording_overlay(&app2);
                crate::tray::change_tray_icon(&app2, crate::tray::TrayIconState::Idle);
            });
            Ok(())
        }
    }
}
