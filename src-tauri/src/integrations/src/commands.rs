use crate::Integrations;
use crate::google::{calendar as google_calendar, docs as google_docs, drive as google_drive, gmail};
use crate::microsoft::{calendar as microsoft_calendar, onedrive, outlook};
use crate::types::ConnectionStatus;
use tauri::State;

#[tauri::command]
pub async fn google_connect(state: State<'_, Integrations>, account: String) -> Result<(), String> {
    state.google.connect(&account).await.map_err(Into::into)
}

#[tauri::command]
pub fn google_disconnect(state: State<'_, Integrations>, account: String) -> Result<(), String> {
    state.google.disconnect(&account).map_err(Into::into)
}

#[tauri::command]
pub fn google_status(
    state: State<'_, Integrations>,
    account: String,
) -> Result<ConnectionStatus, String> {
    state.google.status(&account).map_err(Into::into)
}

#[tauri::command]
pub async fn google_gmail_list(
    state: State<'_, Integrations>,
    account: String,
    query: Option<String>,
    max_results: u32,
    page_token: Option<String>,
) -> Result<gmail::GmailMessageList, String> {
    gmail::list(
        &state.google,
        &account,
        query.as_deref(),
        max_results,
        page_token.as_deref(),
    )
    .await
    .map_err(Into::into)
}

#[tauri::command]
pub async fn google_gmail_get(
    state: State<'_, Integrations>,
    account: String,
    message_id: String,
) -> Result<gmail::GmailMessage, String> {
    gmail::get(&state.google, &account, &message_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn google_gmail_send(
    state: State<'_, Integrations>,
    account: String,
    input: gmail::SendGmailInput,
) -> Result<gmail::GmailMessageRef, String> {
    gmail::send(&state.google, &account, input)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn google_calendar_list(
    state: State<'_, Integrations>,
    account: String,
) -> Result<google_calendar::GoogleCalendarList, String> {
    google_calendar::list_calendars(&state.google, &account)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn google_calendar_events(
    state: State<'_, Integrations>,
    account: String,
    calendar_id: String,
    time_min: Option<String>,
    time_max: Option<String>,
    max_results: u32,
) -> Result<google_calendar::GoogleEventList, String> {
    google_calendar::list_events(
        &state.google,
        &account,
        &calendar_id,
        time_min.as_deref(),
        time_max.as_deref(),
        max_results,
    )
    .await
    .map_err(Into::into)
}

#[tauri::command]
pub async fn google_calendar_create_event(
    state: State<'_, Integrations>,
    account: String,
    input: google_calendar::CreateGoogleEventInput,
) -> Result<google_calendar::GoogleEvent, String> {
    google_calendar::create_event(&state.google, &account, input)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn google_drive_list(
    state: State<'_, Integrations>,
    account: String,
    query: Option<String>,
    page_size: u32,
    page_token: Option<String>,
) -> Result<google_drive::DriveFileList, String> {
    google_drive::list(
        &state.google,
        &account,
        query.as_deref(),
        page_size,
        page_token.as_deref(),
    )
    .await
    .map_err(Into::into)
}

#[tauri::command]
pub async fn google_drive_upload(
    state: State<'_, Integrations>,
    account: String,
    name: String,
    mime_type: String,
    bytes: Vec<u8>,
    parent_id: Option<String>,
) -> Result<google_drive::DriveFile, String> {
    google_drive::upload(
        &state.google,
        &account,
        &name,
        &mime_type,
        bytes,
        parent_id.as_deref(),
    )
    .await
    .map_err(Into::into)
}

#[tauri::command]
pub async fn google_docs_create(
    state: State<'_, Integrations>,
    account: String,
    title: String,
    content: String,
) -> Result<google_docs::GoogleDocument, String> {
    google_docs::create(&state.google, &account, &title, &content)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn microsoft_connect(
    state: State<'_, Integrations>,
    account: String,
) -> Result<(), String> {
    state.microsoft.connect(&account).await.map_err(Into::into)
}

#[tauri::command]
pub fn microsoft_disconnect(
    state: State<'_, Integrations>,
    account: String,
) -> Result<(), String> {
    state.microsoft.disconnect(&account).map_err(Into::into)
}

#[tauri::command]
pub fn microsoft_status(
    state: State<'_, Integrations>,
    account: String,
) -> Result<ConnectionStatus, String> {
    state.microsoft.status(&account).map_err(Into::into)
}

#[tauri::command]
pub async fn microsoft_outlook_list(
    state: State<'_, Integrations>,
    account: String,
    top: u32,
    next_link: Option<String>,
) -> Result<outlook::OutlookMessageList, String> {
    outlook::list(&state.microsoft, &account, top, next_link.as_deref())
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn microsoft_outlook_send(
    state: State<'_, Integrations>,
    account: String,
    input: outlook::SendOutlookInput,
) -> Result<(), String> {
    outlook::send(&state.microsoft, &account, input)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn microsoft_calendar_list(
    state: State<'_, Integrations>,
    account: String,
    top: u32,
    next_link: Option<String>,
) -> Result<microsoft_calendar::MicrosoftCalendarList, String> {
    microsoft_calendar::list_calendars(&state.microsoft, &account, top, next_link.as_deref())
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn microsoft_calendar_events(
    state: State<'_, Integrations>,
    account: String,
    top: u32,
    next_link: Option<String>,
) -> Result<microsoft_calendar::MicrosoftEventList, String> {
    microsoft_calendar::list_events(&state.microsoft, &account, top, next_link.as_deref())
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn microsoft_calendar_create_event(
    state: State<'_, Integrations>,
    account: String,
    input: microsoft_calendar::CreateMicrosoftEventInput,
) -> Result<microsoft_calendar::MicrosoftEvent, String> {
    microsoft_calendar::create_event(&state.microsoft, &account, input)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn microsoft_onedrive_list(
    state: State<'_, Integrations>,
    account: String,
    top: u32,
    next_link: Option<String>,
) -> Result<onedrive::OneDriveItemList, String> {
    onedrive::list_root(&state.microsoft, &account, top, next_link.as_deref())
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn microsoft_onedrive_upload(
    state: State<'_, Integrations>,
    account: String,
    path: String,
    mime_type: String,
    bytes: Vec<u8>,
) -> Result<onedrive::OneDriveItem, String> {
    onedrive::upload_small(&state.microsoft, &account, &path, &mime_type, bytes)
        .await
        .map_err(Into::into)
}
