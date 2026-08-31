use crate::meeting::{intelligence, MeetingListEntry, MeetingManager, MeetingRecord};
use tauri::AppHandle;

#[tauri::command]
#[specta::specta]
pub fn list_meetings(
    app: AppHandle,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<MeetingListEntry>, String> {
    MeetingManager::new(&app)
        .and_then(|manager| {
            manager.list_meetings(limit.unwrap_or(100).min(250), offset.unwrap_or(0))
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn get_meeting(app: AppHandle, id: String) -> Result<Option<MeetingRecord>, String> {
    MeetingManager::new(&app)
        .and_then(|manager| manager.get_meeting(&id))
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn delete_meeting(app: AppHandle, id: String) -> Result<(), String> {
    MeetingManager::new(&app)
        .and_then(|manager| manager.delete_meeting(&id))
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn export_meeting_markdown(app: AppHandle, id: String) -> Result<String, String> {
    MeetingManager::new(&app)
        .and_then(|manager| manager.export_markdown(&id))
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn generate_meeting_intelligence(
    app: AppHandle,
    id: String,
) -> Result<MeetingRecord, String> {
    let manager = MeetingManager::new(&app).map_err(|error| error.to_string())?;
    let mut meeting = manager
        .get_meeting(&id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Meeting not found".to_string())?;

    if meeting.intelligence.is_none() {
        let settings = crate::settings::get_settings(&app);
        let analysis = intelligence::generate_intelligence(&meeting.transcript, &settings).await?;
        manager
            .save_intelligence(&id, &analysis)
            .map_err(|error| error.to_string())?;
        meeting.intelligence = Some(analysis);
    }

    Ok(meeting)
}

#[tauri::command]
#[specta::specta]
pub async fn ask_meeting(app: AppHandle, id: String, question: String) -> Result<String, String> {
    let question = question.trim();
    if question.is_empty() {
        return Err("Enter a question about this meeting".to_string());
    }

    let meeting = MeetingManager::new(&app)
        .map_err(|error| error.to_string())?
        .get_meeting(&id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Meeting not found".to_string())?;
    let settings = crate::settings::get_settings(&app);
    intelligence::ask_anything(question, &meeting.transcript, &settings).await
}
