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

    pub fn execute_send(session: &GmailSession) -> Result<(), GmailActionError> {
        let body = session
            .generated_body
            .clone()
            .ok_or_else(|| GmailActionError::SendFailed("generated body is missing".to_string()))?;
        let recipient = session.recipient_email.clone().ok_or_else(|| {
            GmailActionError::SendFailed("verified recipient is missing".to_string())
        })?;
        context::send(session.identity.clone(), body, recipient)
            .map_err(GmailActionError::SendFailed)
    }
}
