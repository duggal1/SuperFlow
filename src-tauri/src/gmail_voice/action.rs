#![cfg(target_os = "macos")]

use tauri::AppHandle;

use crate::gmail_voice::context;
use crate::gmail_voice::session::{GmailSession, GmailTargetIdentity};

#[derive(Debug)]
pub enum GmailActionError {
    InsertFailed(String),
    SendFailed(String),
    TargetChanged,
}

pub struct GmailActionController;

impl GmailActionController {
    pub fn populate_compose(
        session: &GmailSession,
        recipient_email: Option<String>,
        subject: String,
    ) -> Result<GmailTargetIdentity, GmailActionError> {
        context::populate_compose(session.identity.clone(), recipient_email, subject)
            .map_err(|_| GmailActionError::TargetChanged)
    }

    pub fn insert_body(
        app: &AppHandle,
        session: &GmailSession,
        body: &str,
    ) -> Result<GmailTargetIdentity, GmailActionError> {
        context::verify(
            session.identity.clone(),
            None,
            session.recipient_email.clone(),
        )
        .map_err(|_| GmailActionError::TargetChanged)?;
        crate::clipboard::paste_exact(body.to_string(), app.clone())
            .map_err(GmailActionError::InsertFailed)?;
        context::verify(
            session.identity.clone(),
            Some(body.to_string()),
            session.recipient_email.clone(),
        )
        .map_err(|error| GmailActionError::InsertFailed(error))
    }

    pub fn clear_editor(session: &GmailSession) -> Result<GmailTargetIdentity, GmailActionError> {
        context::clear_editor(session.identity.clone()).map_err(|error| {
            GmailActionError::InsertFailed(format!(
                "could not clear the existing Gmail draft body: {error}"
            ))
        })
    }

    pub fn execute_send(session: &GmailSession) -> Result<(), GmailActionError> {
        let body = session
            .generated_body
            .clone()
            .ok_or_else(|| GmailActionError::SendFailed("generated body is missing".to_string()))?;
        let recipient = session.recipient_email.clone().ok_or_else(|| {
            GmailActionError::SendFailed("verified recipient is missing".to_string())
        })?;
        match context::send(session.identity.clone(), body.clone(), recipient.clone()) {
            Ok(()) => Ok(()),
            Err(ax_err) if ax_err.contains("Send button") => {
                log::warn!(target: "gmail_voice", "AX Send failed ({}), falling back to Cmd+Enter", ax_err);
                #[cfg(target_os = "macos")]
                {
                    // Fallback: Cmd+Enter is Gmail's native send shortcut
                    // Verify still on same target before sending key
                    context::verify(
                        session.identity.clone(),
                        Some(body.clone()),
                        Some(recipient.clone()),
                    )
                    .map_err(|_| GmailActionError::TargetChanged)?;
                    // Use the app's Enigo state like clipboard does
                    // We need AppHandle; session doesn't have it, so we try global
                    // For now, use a fresh Enigo via with_enigo pattern if available
                    // As fallback, try to send Cmd+Enter via enigo crate directly
                    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
                    let mut enigo = Enigo::new(&Settings::default())
                        .map_err(|e| GmailActionError::SendFailed(format!("Enigo failed: {e}")))?;
                    enigo
                        .key(Key::Meta, Direction::Press)
                        .map_err(|e| GmailActionError::SendFailed(format!("{e}")))?;
                    enigo
                        .key(Key::Return, Direction::Press)
                        .map_err(|e| GmailActionError::SendFailed(format!("{e}")))?;
                    enigo
                        .key(Key::Return, Direction::Release)
                        .map_err(|e| GmailActionError::SendFailed(format!("{e}")))?;
                    enigo
                        .key(Key::Meta, Direction::Release)
                        .map_err(|e| GmailActionError::SendFailed(format!("{e}")))?;
                    Ok(())
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Err(GmailActionError::SendFailed(ax_err))
                }
            }
            Err(e) => Err(GmailActionError::SendFailed(e)),
        }
    }
}
