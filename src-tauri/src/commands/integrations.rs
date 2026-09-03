use serde::Serialize;
use specta::Type;
use std::sync::RwLock;
use tauri::{AppHandle, Manager, State};
use tori_integrations::{google::GoogleConfig, microsoft::MicrosoftConfig, Integrations};

use crate::settings::{get_settings, write_settings};

/// Local Keychain slot name for the single connected account per provider.
/// Not a cloud user ID — see the integrations crate README.
const ACCOUNT: &str = "default";

/// Keychain service name shared by every `Integrations` instance the app
/// builds, so saved OAuth tokens survive credential/client rebuilds.
const KEYCHAIN_SERVICE: &str = "com.superflow.app";

/// Which providers currently have OAuth client credentials available. A
/// provider without credentials shows a setup form instead of Connect.
#[derive(Serialize, Type)]
pub struct IntegrationsCredentials {
    pub google: bool,
    pub microsoft: bool,
}

/// Managed app state holding the active [`Integrations`] clients. The inner
/// value is rebuilt (same Keychain service, same tokens) whenever the user
/// saves new OAuth client credentials, so no Tauri state swap is needed.
pub struct IntegrationsState(RwLock<Integrations>);

fn build_integrations(
    google_client_id: String,
    google_client_secret: Option<String>,
    microsoft_client_id: String,
) -> Integrations {
    // `new_unchecked` tolerates empty client IDs so the app can boot, report
    // status, and disconnect without credentials; `connect` then fails with a
    // clear configuration error.
    Integrations::new_unchecked(
        KEYCHAIN_SERVICE,
        GoogleConfig::workspace(google_client_id, google_client_secret),
        MicrosoftConfig::graph(microsoft_client_id),
    )
}

fn env_or_none(var: &str) -> Option<String> {
    std::env::var(var).ok().filter(|v| !v.trim().is_empty())
}

fn non_empty(value: Option<&String>) -> Option<String> {
    value
        .filter(|v| !v.trim().is_empty())
        .map(|v| v.trim().to_string())
}

impl IntegrationsState {
    /// Boot-time state from environment variables.
    /// [`Self::refresh_from_settings`] layers user-saved credentials on top
    /// once the settings store is ready.
    pub fn from_env() -> Self {
        Self(RwLock::new(build_integrations(
            env_or_none("TORI_GOOGLE_CLIENT_ID").unwrap_or_default(),
            env_or_none("TORI_GOOGLE_CLIENT_SECRET"),
            env_or_none("TORI_MICROSOFT_CLIENT_ID").unwrap_or_default(),
        )))
    }

    /// Rebuild the clients, preferring user-saved settings over env vars.
    pub fn refresh_from_settings(&self, app: &AppHandle) {
        let settings = get_settings(app);
        let google_client_id = non_empty(settings.google_oauth_client_id.as_ref())
            .or_else(|| env_or_none("TORI_GOOGLE_CLIENT_ID"))
            .unwrap_or_default();
        let google_client_secret = non_empty(settings.google_oauth_client_secret.as_ref())
            .or_else(|| env_or_none("TORI_GOOGLE_CLIENT_SECRET"));
        let microsoft_client_id = non_empty(settings.microsoft_oauth_client_id.as_ref())
            .or_else(|| env_or_none("TORI_MICROSOFT_CLIENT_ID"))
            .unwrap_or_default();

        *self.0.write().expect("integrations state poisoned") =
            build_integrations(google_client_id, google_client_secret, microsoft_client_id);
    }

    fn google(&self) -> Result<tori_integrations::google::GoogleClient, String> {
        Ok(self
            .0
            .read()
            .expect("integrations state poisoned")
            .google
            .clone())
    }

    fn microsoft(&self) -> Result<tori_integrations::microsoft::MicrosoftClient, String> {
        Ok(self
            .0
            .read()
            .expect("integrations state poisoned")
            .microsoft
            .clone())
    }
}

#[tauri::command]
#[specta::specta]
pub async fn google_connect(state: State<'_, IntegrationsState>) -> Result<(), String> {
    state.google()?.connect(ACCOUNT).await.map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn google_disconnect(state: State<'_, IntegrationsState>) -> Result<(), String> {
    state.google()?.disconnect(ACCOUNT).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn google_status(state: State<'_, IntegrationsState>) -> Result<bool, String> {
    state
        .google()?
        .status(ACCOUNT)
        .map(|status| status.connected)
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn microsoft_connect(state: State<'_, IntegrationsState>) -> Result<(), String> {
    state
        .microsoft()?
        .connect(ACCOUNT)
        .await
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn microsoft_disconnect(state: State<'_, IntegrationsState>) -> Result<(), String> {
    state.microsoft()?.disconnect(ACCOUNT).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn microsoft_status(state: State<'_, IntegrationsState>) -> Result<bool, String> {
    state
        .microsoft()?
        .status(ACCOUNT)
        .map(|status| status.connected)
        .map_err(Into::into)
}

/// Reports which providers have usable OAuth client credentials (settings
/// first, then env vars) so the UI can show a setup form for the rest.
#[tauri::command]
#[specta::specta]
pub fn integrations_credentials_status(app: AppHandle) -> IntegrationsCredentials {
    let settings = get_settings(&app);
    IntegrationsCredentials {
        google: non_empty(settings.google_oauth_client_id.as_ref())
            .or_else(|| env_or_none("TORI_GOOGLE_CLIENT_ID"))
            .is_some(),
        microsoft: non_empty(settings.microsoft_oauth_client_id.as_ref())
            .or_else(|| env_or_none("TORI_MICROSOFT_CLIENT_ID"))
            .is_some(),
    }
}

/// Persists Google OAuth client credentials (bring-your-own) and rebuilds the
/// integration clients. Tokens already in the Keychain are untouched.
#[tauri::command]
#[specta::specta]
pub fn integrations_save_google_credentials(
    app: AppHandle,
    client_id: String,
    client_secret: Option<String>,
) -> Result<(), String> {
    let client_id = client_id.trim().to_string();
    if client_id.is_empty() {
        return Err("Google client ID is required".into());
    }
    let mut settings = get_settings(&app);
    settings.google_oauth_client_id = Some(client_id);
    settings.google_oauth_client_secret = client_secret.filter(|v| !v.trim().is_empty());
    write_settings(&app, settings);
    app.state::<IntegrationsState>().refresh_from_settings(&app);
    Ok(())
}

/// Persists the Microsoft OAuth client ID (bring-your-own) and rebuilds the
/// integration clients. Tokens already in the Keychain are untouched.
#[tauri::command]
#[specta::specta]
pub fn integrations_save_microsoft_credentials(
    app: AppHandle,
    client_id: String,
) -> Result<(), String> {
    let client_id = client_id.trim().to_string();
    if client_id.is_empty() {
        return Err("Microsoft client ID is required".into());
    }
    let mut settings = get_settings(&app);
    settings.microsoft_oauth_client_id = Some(client_id);
    write_settings(&app, settings);
    app.state::<IntegrationsState>().refresh_from_settings(&app);
    Ok(())
}
