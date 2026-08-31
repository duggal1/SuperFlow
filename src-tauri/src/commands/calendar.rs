use crate::settings::get_settings;
use tauri::{AppHandle, Manager};

#[tauri::command]
#[specta::specta]
pub async fn submit_calendar_clarification(
    app: AppHandle,
    transcript: String,
) -> Result<(), String> {
    let transcript = transcript.trim().to_string();
    if transcript.is_empty() {
        return Err("Empty clarification".to_string());
    }

    // Show tiny processing moment — single spinner left, extremely clean, like standard loading
    // We reuse the say_this pill's random loading but for calendar we show "Scheduling..." via the calendar handler's internal processing
    let settings = get_settings(&app);

    // Use the same strict flow: AI -> validate -> Swift/EventKit
    // This will show calendar_processing with title before Swift, then success
    match crate::calendar::handle_calendar_transcript(&transcript, &settings, Some(&app)).await {
        crate::calendar::CalendarHandleResult::NotCalendar => {
            // If still not calendar after clarification, treat as generic failure — show clarification again or fallback
            crate::overlay::show_calendar_clarify_overlay(
                &app,
                "Could you specify the date and time? For example, tomorrow at 9:30 AM."
                    .to_string(),
            );
            Ok(())
        }
        crate::calendar::CalendarHandleResult::Success(result) => {
            crate::overlay::show_calendar_success_overlay(&app, &result);
            // Save to history for audit
            if let Some(hm) =
                app.try_state::<std::sync::Arc<crate::managers::history::HistoryManager>>()
            {
                let file_name =
                    format!("calendar-clarified-{}.wav", chrono::Utc::now().timestamp());
                let _ = hm.save_entry(
                    file_name,
                    transcript,
                    false,
                    Some(format!("Calendar: {} @ {}", result.title, result.start)),
                    None,
                    0.0,
                );
            }
            let app2 = app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(3500)).await;
                crate::overlay::hide_recording_overlay(&app2);
                crate::tray::change_tray_icon(&app2, crate::tray::TrayIconState::Idle);
            });
            Ok(())
        }
        crate::calendar::CalendarHandleResult::NeedsClarification(clarify) => {
            // Still needs clarification — show the new question, keep input open
            crate::overlay::show_calendar_clarify_overlay(&app, clarify.clarification_message);
            Ok(())
        }
        crate::calendar::CalendarHandleResult::Failure(err) => {
            crate::overlay::show_calendar_failure_overlay(&app, err.message.clone());
            log::warn!(
                "Calendar clarification failed: {} - {}",
                err.error,
                err.message
            );
            let app2 = app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(3000)).await;
                crate::overlay::hide_recording_overlay(&app2);
                crate::tray::change_tray_icon(&app2, crate::tray::TrayIconState::Idle);
            });
            Ok(())
        }
    }
}
