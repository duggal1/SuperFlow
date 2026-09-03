use tauri::State;
use tori_integrations::Integrations;

/// Local Keychain slot name for the single connected account per provider.
/// Not a cloud user ID — see the integrations crate README.
const ACCOUNT: &str = "default";

#[tauri::command]
#[specta::specta]
pub async fn google_connect(state: State<'_, Integrations>) -> Result<(), String> {
    state.google.connect(ACCOUNT).await.map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn google_disconnect(state: State<'_, Integrations>) -> Result<(), String> {
    state.google.disconnect(ACCOUNT).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn google_status(state: State<'_, Integrations>) -> Result<bool, String> {
    state
        .google
        .status(ACCOUNT)
        .map(|status| status.connected)
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn microsoft_connect(state: State<'_, Integrations>) -> Result<(), String> {
    state.microsoft.connect(ACCOUNT).await.map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn microsoft_disconnect(state: State<'_, Integrations>) -> Result<(), String> {
    state.microsoft.disconnect(ACCOUNT).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn microsoft_status(state: State<'_, Integrations>) -> Result<bool, String> {
    state
        .microsoft
        .status(ACCOUNT)
        .map(|status| status.connected)
        .map_err(Into::into)
}
