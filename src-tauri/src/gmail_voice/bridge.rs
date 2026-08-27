#![cfg(target_os = "macos")]

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::gmail_voice::context::CapturedGmailContext;
use crate::gmail_voice::grammar::GmailIntent;
use crate::gmail_voice::session::GmailTargetIdentity;

const AGENT_TIMEOUT: Duration = Duration::from_millis(1_500);

#[derive(Debug, Serialize, Deserialize)]
pub enum GmailAgentRequest {
    Capture {
        intent: GmailIntent,
        expected_pid: i32,
        expected_bundle_id: String,
    },
    Verify {
        identity: GmailTargetIdentity,
        expected_body: Option<String>,
        expected_recipient_email: Option<String>,
    },
    PopulateCompose {
        identity: GmailTargetIdentity,
        recipient_email: Option<String>,
        subject: String,
    },
    Send {
        identity: GmailTargetIdentity,
        expected_body: String,
        expected_recipient_email: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum GmailAgentResponse {
    Captured(CapturedGmailContext),
    Verified(GmailTargetIdentity),
    Sent,
    Rejected(String),
}

pub fn request(request: GmailAgentRequest) -> Result<GmailAgentResponse, String> {
    if cfg!(test) {
        return Err("Gmail Accessibility agent is unavailable in unit tests".to_string());
    }
    let executable = std::env::current_exe()
        .map_err(|error| format!("could not locate Gmail Accessibility agent: {error}"))?;
    let mut child = Command::new(executable)
        .arg("--gmail-agent")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("could not start Gmail Accessibility agent: {error}"))?;

    let payload = serde_json::to_vec(&request)
        .map_err(|error| format!("could not encode Gmail Accessibility request: {error}"))?;
    child
        .stdin
        .take()
        .ok_or_else(|| "Gmail Accessibility agent stdin was unavailable".to_string())?
        .write_all(&payload)
        .map_err(|error| format!("could not send Gmail Accessibility request: {error}"))?;

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Gmail Accessibility agent stdout was unavailable".to_string())?;
    let reader = std::thread::spawn(move || {
        let mut output = String::new();
        let _ = stdout.read_to_string(&mut output);
        output
    });

    match wait_with_timeout(&mut child, AGENT_TIMEOUT) {
        Ok(()) => {}
        Err(AgentWaitError::TimedOut) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Gmail Accessibility agent timed out".to_string());
        }
        Err(AgentWaitError::Exited(status)) => {
            return Err(format!(
                "Gmail Accessibility agent exited unsuccessfully: {status}"
            ));
        }
        Err(AgentWaitError::Wait(error)) => {
            return Err(format!(
                "could not wait for Gmail Accessibility agent: {error}"
            ));
        }
    }
    let output = reader
        .join()
        .map_err(|_| "Gmail Accessibility agent output reader failed".to_string())?;
    serde_json::from_str(output.lines().last().unwrap_or(""))
        .map_err(|error| format!("invalid Gmail Accessibility response: {error}"))
}

pub fn run_agent() {
    let mut input = String::new();
    let response = match std::io::stdin().read_to_string(&mut input) {
        Ok(_) => match serde_json::from_str::<GmailAgentRequest>(&input) {
            Ok(request) => crate::gmail_voice::ax::execute(request),
            Err(error) => GmailAgentResponse::Rejected(format!("invalid request: {error}")),
        },
        Err(error) => GmailAgentResponse::Rejected(format!("could not read request: {error}")),
    };
    if let Ok(json) = serde_json::to_string(&response) {
        println!("{json}");
    }
}

enum AgentWaitError {
    TimedOut,
    Exited(std::process::ExitStatus),
    Wait(std::io::Error),
}

fn wait_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> Result<(), AgentWaitError> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => return Err(AgentWaitError::Exited(status)),
            Err(error) => return Err(AgentWaitError::Wait(error)),
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(None) => return Err(AgentWaitError::TimedOut),
        }
    }
}
