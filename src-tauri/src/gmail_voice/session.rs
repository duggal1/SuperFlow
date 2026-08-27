use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::gmail_voice::grammar::GmailIntent;

const SESSION_TTL_MS: u64 = 120_000;
static SESSION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GmailSessionId(u64);

impl GmailSessionId {
    pub fn new() -> Self {
        Self(SESSION_SEQUENCE.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GmailSessionState {
    Capturing,
    Generating,
    Inserting,
    DraftReady,
    Sending,
    Completed,
    Invalidated,
}

impl GmailSessionState {
    pub fn is_live(self) -> bool {
        matches!(
            self,
            Self::Capturing | Self::Generating | Self::Inserting | Self::DraftReady
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GmailTargetIdentity {
    pub bundle_id: String,
    pub pid: i32,
    pub url: String,
    pub window_identity: String,
    pub thread_key: String,
    pub editor_identity: String,
    pub recipient_email: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GmailSession {
    pub id: GmailSessionId,
    pub state: GmailSessionState,
    pub intent: GmailIntent,
    pub identity: GmailTargetIdentity,
    pub recipient_name: Option<String>,
    pub recipient_email: Option<String>,
    pub subject: Option<String>,
    pub source_message: Option<String>,
    pub thread_context: Option<String>,
    pub generated_body: Option<String>,
    pub generated_subject: Option<String>,
    pub insertion_verified: bool,
    pub created_at_ms: u64,
}

impl GmailSession {
    pub fn new(intent: GmailIntent, identity: GmailTargetIdentity) -> Self {
        Self {
            id: GmailSessionId::new(),
            state: GmailSessionState::Capturing,
            intent,
            identity,
            recipient_name: None,
            recipient_email: None,
            subject: None,
            source_message: None,
            thread_context: None,
            generated_body: None,
            generated_subject: None,
            insertion_verified: false,
            created_at_ms: now_ms(),
        }
    }

    pub fn is_expired_at(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.created_at_ms) > SESSION_TTL_MS
    }

    pub fn can_send_at(&self, now_ms: u64) -> bool {
        let recipient_verified = match self.intent {
            GmailIntent::Reply | GmailIntent::Compose => self
                .recipient_email
                .as_deref()
                .is_some_and(is_email_address),
        };
        self.state == GmailSessionState::DraftReady
            && !self.is_expired_at(now_ms)
            && self.insertion_verified
            && recipient_verified
            && self
                .generated_body
                .as_deref()
                .is_some_and(|body| !body.trim().is_empty())
    }

    pub fn can_send(&self) -> bool {
        self.can_send_at(now_ms())
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn is_email_address(value: &str) -> bool {
    let Some((local, domain)) = value.trim().split_once('@') else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> GmailTargetIdentity {
        GmailTargetIdentity {
            bundle_id: "com.google.Chrome".to_string(),
            pid: 42,
            url: "https://mail.google.com/mail/u/0/#inbox/thread".to_string(),
            window_identity: "window-1".to_string(),
            thread_key: "inbox/thread".to_string(),
            editor_identity: "0/4/2:Message Body".to_string(),
            recipient_email: Some("alex@example.com".to_string()),
        }
    }

    #[test]
    fn send_requires_ready_verified_unexpired_draft_and_recipient() {
        let mut session = GmailSession::new(GmailIntent::Reply, identity());
        session.recipient_email = Some("alex@example.com".to_string());
        session.generated_body = Some("Hello Alex".to_string());
        assert!(!session.can_send_at(session.created_at_ms));

        session.state = GmailSessionState::DraftReady;
        session.insertion_verified = true;
        assert!(session.can_send_at(session.created_at_ms));

        session.recipient_email = None;
        assert!(!session.can_send_at(session.created_at_ms));
    }

    #[test]
    fn expired_session_cannot_send() {
        let mut session = GmailSession::new(GmailIntent::Compose, identity());
        session.recipient_email = Some("alex@example.com".to_string());
        session.generated_body = Some("Hello".to_string());
        session.state = GmailSessionState::DraftReady;
        session.insertion_verified = true;
        assert!(!session.can_send_at(session.created_at_ms + SESSION_TTL_MS + 1));
    }

    #[test]
    fn target_identity_detects_every_material_change() {
        let original = identity();
        let mut changed = original.clone();
        changed.editor_identity = "0/7/2:Message Body".to_string();
        assert_ne!(original, changed);
        changed = original.clone();
        changed.recipient_email = Some("other@example.com".to_string());
        assert_ne!(original, changed);
        changed = original.clone();
        changed.url.push_str("-other");
        assert_ne!(original, changed);
    }
}
