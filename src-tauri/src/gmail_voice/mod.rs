use tauri::AppHandle;

use crate::context::types::{ContextSnapshot, Surface};
use crate::settings::AppSettings;

#[derive(Debug)]
pub enum GmailVoiceError {
    NotGmail,
    ContextUnreliable(String),
    GenerationFailed(String),
    InsertFailed(String),
    SendFailed(String),
    SessionChanged,
}

impl std::fmt::Display for GmailVoiceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotGmail => formatter.write_str("Gmail is not active"),
            Self::ContextUnreliable(error) => {
                write!(formatter, "Gmail context unavailable: {error}")
            }
            Self::GenerationFailed(error) => write!(formatter, "Gmail generation failed: {error}"),
            Self::InsertFailed(error) => write!(formatter, "Gmail insertion failed: {error}"),
            Self::SendFailed(error) => write!(formatter, "Gmail send failed: {error}"),
            Self::SessionChanged => formatter.write_str("Gmail target changed"),
        }
    }
}

#[derive(Debug)]
pub enum GmailHandleResult {
    NotHandled,
    Drafted,
    Sent,
    Cancelled,
    Failed(GmailVoiceError),
}

#[cfg(target_os = "macos")]
mod action;
#[cfg(target_os = "macos")]
mod ax;
#[cfg(target_os = "macos")]
mod bridge;
#[cfg(target_os = "macos")]
mod context;
mod generator;
pub(crate) mod grammar;
pub(crate) mod session;

#[cfg(target_os = "macos")]
use action::{GmailActionController, GmailActionError};
#[cfg(target_os = "macos")]
use context::GmailContext;
#[cfg(target_os = "macos")]
use generator::{GmailGeneratedContent, GmailGenerator};
#[cfg(target_os = "macos")]
use grammar::{GmailVoiceInput, TerminalAction};
#[cfg(target_os = "macos")]
use session::{GmailSession, GmailSessionId, GmailSessionState};

static ACTIVE_SESSION: std::sync::Mutex<Option<session::GmailSession>> =
    std::sync::Mutex::new(None);

#[cfg(target_os = "macos")]
pub(crate) fn run_agent() {
    bridge::run_agent();
}

#[cfg(target_os = "macos")]
pub async fn handle(
    transcription: &str,
    snapshot: &ContextSnapshot,
    settings: &AppSettings,
    app: &AppHandle,
) -> GmailHandleResult {
    if !settings.experimental_gmail_voice_enabled || snapshot.surface != Surface::Gmail {
        return GmailHandleResult::NotHandled;
    }
    let Some(input) = grammar::parse(transcription) else {
        return GmailHandleResult::NotHandled;
    };
    match input {
        GmailVoiceInput::SessionAction(action) => handle_session_action(action),
        GmailVoiceInput::Start(command) => handle_start(command, settings, app).await,
    }
}

#[cfg(not(target_os = "macos"))]
pub async fn handle(
    _transcription: &str,
    _snapshot: &ContextSnapshot,
    _settings: &AppSettings,
    _app: &AppHandle,
) -> GmailHandleResult {
    GmailHandleResult::NotHandled
}

#[cfg(target_os = "macos")]
fn handle_session_action(action: TerminalAction) -> GmailHandleResult {
    match action {
        TerminalAction::Cancel => {
            let mut guard = ACTIVE_SESSION.lock().unwrap();
            if guard
                .as_ref()
                .is_some_and(|session| session.state.is_live())
            {
                *guard = None;
                GmailHandleResult::Cancelled
            } else {
                GmailHandleResult::NotHandled
            }
        }
        TerminalAction::Send => {
            let id = {
                let mut guard = ACTIVE_SESSION.lock().unwrap();
                let Some(session) = guard.as_mut() else {
                    return GmailHandleResult::NotHandled;
                };
                if !session.can_send() {
                    if session.is_expired_at(session::now_ms()) {
                        *guard = None;
                        return GmailHandleResult::NotHandled;
                    }
                    return GmailHandleResult::Failed(GmailVoiceError::SendFailed(
                        "send refused: draft, insertion, or recipient is not verified".to_string(),
                    ));
                }
                session.state = GmailSessionState::Sending;
                session.id
            };
            send_session(id)
        }
        TerminalAction::None => GmailHandleResult::NotHandled,
    }
}

#[cfg(target_os = "macos")]
async fn handle_start(
    command: grammar::GmailVoiceCommand,
    settings: &AppSettings,
    app: &AppHandle,
) -> GmailHandleResult {
    if command.instruction.trim().is_empty() {
        return GmailHandleResult::Failed(GmailVoiceError::GenerationFailed(
            "Gmail instruction is empty".to_string(),
        ));
    }
    let Some(frontmost) = crate::context::detector::frontmost_app() else {
        return GmailHandleResult::Failed(GmailVoiceError::NotGmail);
    };
    let Some(bundle_id) = frontmost.bundle_id.clone() else {
        return GmailHandleResult::Failed(GmailVoiceError::NotGmail);
    };
    let captured = match context::capture(command.intent, frontmost.pid, bundle_id) {
        Ok(captured) => captured,
        Err(error) => return GmailHandleResult::Failed(GmailVoiceError::ContextUnreliable(error)),
    };
    if !captured.editor_body.trim().is_empty() {
        return GmailHandleResult::Failed(GmailVoiceError::InsertFailed(
            "Gmail editor must be empty before voice drafting".to_string(),
        ));
    }

    let mut session = GmailSession::new(command.intent, captured.identity.clone());
    populate_session_context(&mut session, &captured.context, &command);
    session.state = GmailSessionState::Generating;
    let id = session.id;
    *ACTIVE_SESSION.lock().unwrap() = Some(session);
    let mut transaction = GmailTransaction::new(id);

    let generated = match GmailGenerator::generate(settings, &command, &captured.context).await {
        Ok(generated) => generated,
        Err(error) => return GmailHandleResult::Failed(GmailVoiceError::GenerationFailed(error)),
    };
    if !active_session_is(id, GmailSessionState::Generating) {
        return GmailHandleResult::Failed(GmailVoiceError::SessionChanged);
    }
    let identity = active_session(id)
        .map(|session| session.identity)
        .ok_or(GmailVoiceError::SessionChanged);
    let identity = match identity {
        Ok(identity) => identity,
        Err(error) => return GmailHandleResult::Failed(error),
    };
    if context::verify(identity, None, None).is_err() {
        return GmailHandleResult::Failed(GmailVoiceError::SessionChanged);
    }
    if !transition(
        id,
        GmailSessionState::Generating,
        GmailSessionState::Inserting,
    ) {
        return GmailHandleResult::Failed(GmailVoiceError::SessionChanged);
    }

    if let GmailGeneratedContent::Compose { subject, .. } = &generated {
        let session = match active_session(id) {
            Some(session) => session,
            None => return GmailHandleResult::Failed(GmailVoiceError::SessionChanged),
        };
        let updated_identity = match GmailActionController::populate_compose(
            &session,
            session.recipient_email.clone(),
            subject.clone(),
        ) {
            Ok(identity) => identity,
            Err(error) => return GmailHandleResult::Failed(map_action_error(error)),
        };
        if !update_session(id, |session| {
            session.identity = updated_identity.clone();
            session.subject = Some(subject.clone());
            session.recipient_email = updated_identity.recipient_email.clone();
        }) {
            return GmailHandleResult::Failed(GmailVoiceError::SessionChanged);
        }
    }

    let body = generated.body().to_string();
    let session = match active_session(id) {
        Some(session) => session,
        None => return GmailHandleResult::Failed(GmailVoiceError::SessionChanged),
    };
    let verified_identity = match GmailActionController::insert_body(app, &session, &body) {
        Ok(identity) => identity,
        Err(error) => return GmailHandleResult::Failed(map_action_error(error)),
    };
    if !update_session(id, |session| {
        session.identity = verified_identity;
        session.generated_body = Some(body.clone());
        session.generated_subject = match &generated {
            GmailGeneratedContent::Compose { subject, .. } => Some(subject.clone()),
            GmailGeneratedContent::Reply { .. } => None,
        };
        session.insertion_verified = true;
        session.state = GmailSessionState::DraftReady;
    }) {
        return GmailHandleResult::Failed(GmailVoiceError::SessionChanged);
    }

    transaction.commit();
    if command.terminal_action == TerminalAction::Send {
        if !transition(
            id,
            GmailSessionState::DraftReady,
            GmailSessionState::Sending,
        ) {
            return GmailHandleResult::Failed(GmailVoiceError::SessionChanged);
        }
        return send_session(id);
    }
    GmailHandleResult::Drafted
}

#[cfg(target_os = "macos")]
fn populate_session_context(
    session: &mut GmailSession,
    context: &GmailContext,
    command: &grammar::GmailVoiceCommand,
) {
    match context {
        GmailContext::Reply(reply) => {
            session.recipient_name = Some(reply.sender_name.clone());
            session.recipient_email = Some(reply.sender_email.clone());
            session.subject = Some(reply.subject.clone());
            session.source_message = Some(reply.source_message.clone());
            session.thread_context = reply.thread_context.clone();
            session.identity.recipient_email = Some(reply.sender_email.clone());
        }
        GmailContext::Compose(compose) => {
            session.recipient_email = compose
                .recipient_email
                .clone()
                .or_else(|| grammar::literal_recipient_email(command.recipient_hint.as_deref()));
            session.recipient_name = command.recipient_hint.clone();
            session.subject = compose.subject.clone();
        }
    }
}

#[cfg(target_os = "macos")]
fn send_session(id: GmailSessionId) -> GmailHandleResult {
    let Some(session) = active_session(id) else {
        return GmailHandleResult::Failed(GmailVoiceError::SessionChanged);
    };
    if session.state != GmailSessionState::Sending
        || !session.insertion_verified
        || session.recipient_email.is_none()
    {
        invalidate_session(id);
        return GmailHandleResult::Failed(GmailVoiceError::SendFailed(
            "Gmail draft is not fully verified".to_string(),
        ));
    }
    match GmailActionController::execute_send(&session) {
        Ok(()) => {
            let mut guard = ACTIVE_SESSION.lock().unwrap();
            if guard.as_ref().is_some_and(|active| active.id == id) {
                if let Some(active) = guard.as_mut() {
                    active.state = GmailSessionState::Completed;
                }
                *guard = None;
                GmailHandleResult::Sent
            } else {
                GmailHandleResult::Failed(GmailVoiceError::SessionChanged)
            }
        }
        Err(error) => {
            invalidate_session(id);
            GmailHandleResult::Failed(map_action_error(error))
        }
    }
}

#[cfg(target_os = "macos")]
fn map_action_error(error: GmailActionError) -> GmailVoiceError {
    match error {
        GmailActionError::InsertFailed(error) => GmailVoiceError::InsertFailed(error),
        GmailActionError::SendFailed(error) => GmailVoiceError::SendFailed(error),
        GmailActionError::TargetChanged => GmailVoiceError::SessionChanged,
    }
}

#[cfg(target_os = "macos")]
fn active_session(id: GmailSessionId) -> Option<GmailSession> {
    ACTIVE_SESSION
        .lock()
        .unwrap()
        .as_ref()
        .filter(|session| session.id == id)
        .cloned()
}

#[cfg(target_os = "macos")]
fn active_session_is(id: GmailSessionId, state: GmailSessionState) -> bool {
    ACTIVE_SESSION
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|session| session.id == id && session.state == state)
}

#[cfg(target_os = "macos")]
fn transition(id: GmailSessionId, from: GmailSessionState, to: GmailSessionState) -> bool {
    let mut guard = ACTIVE_SESSION.lock().unwrap();
    let Some(session) = guard
        .as_mut()
        .filter(|session| session.id == id && session.state == from)
    else {
        return false;
    };
    session.state = to;
    true
}

#[cfg(target_os = "macos")]
fn update_session(id: GmailSessionId, update: impl FnOnce(&mut GmailSession)) -> bool {
    let mut guard = ACTIVE_SESSION.lock().unwrap();
    let Some(session) = guard.as_mut().filter(|session| session.id == id) else {
        return false;
    };
    update(session);
    true
}

#[cfg(target_os = "macos")]
fn invalidate_session(id: GmailSessionId) {
    let mut guard = ACTIVE_SESSION.lock().unwrap();
    if guard.as_ref().is_some_and(|session| session.id == id) {
        if let Some(session) = guard.as_mut() {
            session.state = GmailSessionState::Invalidated;
        }
        *guard = None;
    }
}

#[cfg(target_os = "macos")]
struct GmailTransaction {
    id: GmailSessionId,
    committed: bool,
}

#[cfg(target_os = "macos")]
impl GmailTransaction {
    fn new(id: GmailSessionId) -> Self {
        Self {
            id,
            committed: false,
        }
    }

    fn commit(&mut self) {
        self.committed = true;
    }
}

#[cfg(target_os = "macos")]
impl Drop for GmailTransaction {
    fn drop(&mut self) {
        if !self.committed {
            invalidate_session(self.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gmail_voice::grammar::GmailIntent;

    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn transaction_only_invalidates_its_own_session() {
        let _test_guard = TEST_LOCK.lock().unwrap();
        let identity = session::GmailTargetIdentity {
            bundle_id: "com.google.Chrome".to_string(),
            pid: 1,
            url: "https://mail.google.com/#inbox/one".to_string(),
            window_identity: "window".to_string(),
            thread_key: "inbox/one".to_string(),
            editor_identity: "editor".to_string(),
            recipient_email: Some("a@example.com".to_string()),
        };
        let first = GmailSession::new(GmailIntent::Reply, identity.clone());
        let first_id = first.id;
        *ACTIVE_SESSION.lock().unwrap() = Some(first);
        let transaction = GmailTransaction::new(first_id);

        let second = GmailSession::new(GmailIntent::Reply, identity);
        let second_id = second.id;
        *ACTIVE_SESSION.lock().unwrap() = Some(second);
        drop(transaction);

        assert!(active_session(second_id).is_some());
        *ACTIVE_SESSION.lock().unwrap() = None;
    }

    #[test]
    fn sending_transition_is_atomic() {
        let _test_guard = TEST_LOCK.lock().unwrap();
        let identity = session::GmailTargetIdentity {
            bundle_id: "com.google.Chrome".to_string(),
            pid: 1,
            url: "https://mail.google.com/#inbox/one".to_string(),
            window_identity: "window".to_string(),
            thread_key: "inbox/one".to_string(),
            editor_identity: "editor".to_string(),
            recipient_email: Some("a@example.com".to_string()),
        };
        let mut session = GmailSession::new(GmailIntent::Reply, identity);
        session.state = GmailSessionState::DraftReady;
        let id = session.id;
        *ACTIVE_SESSION.lock().unwrap() = Some(session);
        assert!(transition(
            id,
            GmailSessionState::DraftReady,
            GmailSessionState::Sending
        ));
        assert!(!transition(
            id,
            GmailSessionState::DraftReady,
            GmailSessionState::Sending
        ));
        *ACTIVE_SESSION.lock().unwrap() = None;
    }
}
