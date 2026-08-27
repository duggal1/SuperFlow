use serde::{Deserialize, Serialize};

use crate::gmail_voice::bridge::{self, GmailAgentRequest, GmailAgentResponse};
use crate::gmail_voice::grammar::GmailIntent;
use crate::gmail_voice::session::GmailTargetIdentity;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplyContext {
    pub sender_name: String,
    pub sender_email: String,
    pub subject: String,
    pub source_message: String,
    pub thread_context: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComposeContext {
    pub recipient_email: Option<String>,
    pub subject: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GmailContext {
    Reply(ReplyContext),
    Compose(ComposeContext),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedGmailContext {
    pub identity: GmailTargetIdentity,
    pub context: GmailContext,
    pub editor_body: String,
}

pub fn capture(
    intent: GmailIntent,
    expected_pid: i32,
    expected_bundle_id: String,
) -> Result<CapturedGmailContext, String> {
    match bridge::request(GmailAgentRequest::Capture {
        intent,
        expected_pid,
        expected_bundle_id,
    })? {
        GmailAgentResponse::Captured(captured) => Ok(captured),
        GmailAgentResponse::Rejected(reason) => Err(reason),
        _ => Err("Gmail Accessibility agent returned an unexpected response".to_string()),
    }
}

pub fn verify(
    identity: GmailTargetIdentity,
    expected_body: Option<String>,
    expected_recipient_email: Option<String>,
) -> Result<GmailTargetIdentity, String> {
    match bridge::request(GmailAgentRequest::Verify {
        identity,
        expected_body,
        expected_recipient_email,
    })? {
        GmailAgentResponse::Verified(identity) => Ok(identity),
        GmailAgentResponse::Rejected(reason) => Err(reason),
        _ => Err("Gmail Accessibility agent returned an unexpected response".to_string()),
    }
}

pub fn populate_compose(
    identity: GmailTargetIdentity,
    recipient_email: Option<String>,
    subject: String,
) -> Result<GmailTargetIdentity, String> {
    match bridge::request(GmailAgentRequest::PopulateCompose {
        identity,
        recipient_email,
        subject,
    })? {
        GmailAgentResponse::Verified(identity) => Ok(identity),
        GmailAgentResponse::Rejected(reason) => Err(reason),
        _ => Err("Gmail Accessibility agent returned an unexpected response".to_string()),
    }
}

pub fn send(
    identity: GmailTargetIdentity,
    expected_body: String,
    expected_recipient_email: String,
) -> Result<(), String> {
    match bridge::request(GmailAgentRequest::Send {
        identity,
        expected_body,
        expected_recipient_email,
    })? {
        GmailAgentResponse::Sent => Ok(()),
        GmailAgentResponse::Rejected(reason) => Err(reason),
        _ => Err("Gmail Accessibility agent returned an unexpected response".to_string()),
    }
}
