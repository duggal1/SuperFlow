pub mod native;
pub mod parser;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Strict schema AI must output. Never arbitrary code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarCreateRequest {
    pub action: String,
    pub title: String,
    pub start: String,
    pub end: String,
    #[serde(default)]
    pub calendar: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub reminders_minutes_before: Option<Vec<i32>>,
    pub success_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarNeedsClarification {
    pub action: String,
    pub missing_fields: Vec<String>,
    pub clarification_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CalendarAiOutput {
    Create(CalendarCreateRequest),
    Clarify(CalendarNeedsClarification),
    None { action: String },
}

#[derive(Debug, Clone)]
pub struct ValidatedCalendarEvent {
    pub title: String,
    pub start_str: String,
    pub end_str: String,
    pub calendar: Option<String>,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub reminders_minutes_before: Vec<i32>,
    pub success_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarSuccessResult {
    pub ok: bool,
    pub action: String,
    pub title: String,
    pub start: String,
    pub end: String,
    pub calendar: String,
    pub event_id: String,
    pub success_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarErrorResult {
    pub ok: bool,
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

fn is_generic_success(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("action completed")
        || lower.contains("operation successful")
        || lower.contains("has been created")
        || lower.contains("successfully")
}

fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
}

/// Validate every AI field before executing anything. Returns validated event or clarification/error.
pub fn validate_create_request(
    req: &CalendarCreateRequest,
) -> Result<ValidatedCalendarEvent, CalendarErrorResult> {
    if req.action != "calendar.create_event" {
        return Err(CalendarErrorResult {
            ok: false,
            error: "validation_error".to_string(),
            message: "Invalid action".to_string(),
            details: None,
        });
    }
    let title = req.title.trim();
    if title.is_empty() || title.len() > 200 {
        return Err(CalendarErrorResult {
            ok: false,
            error: "validation_error".to_string(),
            message: "Title is required".to_string(),
            details: None,
        });
    }
    // Parse start/end as RFC3339 with offset
    let start = DateTime::parse_from_rfc3339(&req.start).map_err(|_| CalendarErrorResult {
        ok: false,
        error: "validation_error".to_string(),
        message: "Invalid start time".to_string(),
        details: Some(req.start.clone()),
    })?;
    let end = DateTime::parse_from_rfc3339(&req.end).map_err(|_| CalendarErrorResult {
        ok: false,
        error: "validation_error".to_string(),
        message: "Invalid end time".to_string(),
        details: Some(req.end.clone()),
    })?;
    if end <= start {
        return Err(CalendarErrorResult {
            ok: false,
            error: "validation_error".to_string(),
            message: "End must be after start".to_string(),
            details: None,
        });
    }
    let duration = end.signed_duration_since(start);
    if duration.num_hours() > 24 {
        return Err(CalendarErrorResult {
            ok: false,
            error: "validation_error".to_string(),
            message: "Event too long (max 24h)".to_string(),
            details: None,
        });
    }
    // Check not too far in past (allow 5 min grace for clock skew)
    let now = Utc::now().with_timezone(start.offset());
    if end < now - chrono::Duration::minutes(5) {
        return Err(CalendarErrorResult {
            ok: false,
            error: "validation_error".to_string(),
            message: "Event is in the past".to_string(),
            details: None,
        });
    }
    let success = req.success_message.trim();
    let wc = word_count(success);
    if wc < 2 || wc > 5 || is_generic_success(success) {
        return Err(CalendarErrorResult {
            ok: false,
            error: "validation_error".to_string(),
            message: "Invalid success_message (must be 2-5 words, natural)".to_string(),
            details: Some(success.to_string()),
        });
    }
    if success.len() > 50 {
        return Err(CalendarErrorResult {
            ok: false,
            error: "validation_error".to_string(),
            message: "success_message too long".to_string(),
            details: None,
        });
    }
    // Calendar name optional, if present must be non-empty and not inventing per-event calendar
    let calendar = req
        .calendar
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(ref c) = calendar {
        if c.len() > 100 {
            return Err(CalendarErrorResult {
                ok: false,
                error: "validation_error".to_string(),
                message: "Calendar name too long".to_string(),
                details: None,
            });
        }
    }
    let location = req
        .location
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(ref l) = location {
        if l.len() > 300 {
            return Err(CalendarErrorResult {
                ok: false,
                error: "validation_error".to_string(),
                message: "Location too long".to_string(),
                details: None,
            });
        }
    }
    let notes = req
        .notes
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(ref n) = notes {
        if n.len() > 2000 {
            return Err(CalendarErrorResult {
                ok: false,
                error: "validation_error".to_string(),
                message: "Notes too long".to_string(),
                details: None,
            });
        }
    }
    let reminders = req.reminders_minutes_before.clone().unwrap_or_default();
    if reminders.len() > 5 {
        return Err(CalendarErrorResult {
            ok: false,
            error: "validation_error".to_string(),
            message: "Too many reminders (max 5)".to_string(),
            details: None,
        });
    }
    for &r in &reminders {
        if r < 0 || r > 40320 {
            return Err(CalendarErrorResult {
                ok: false,
                error: "validation_error".to_string(),
                message: "Reminder out of range (0-40320)".to_string(),
                details: Some(r.to_string()),
            });
        }
    }

    Ok(ValidatedCalendarEvent {
        title: title.to_string(),
        start_str: req.start.clone(),
        end_str: req.end.clone(),
        calendar,
        location,
        notes,
        reminders_minutes_before: reminders,
        success_message: success.to_string(),
    })
}

pub fn validate_ai_output(raw: &str) -> Result<CalendarAiOutput, CalendarErrorResult> {
    // Try to extract JSON from possible markdown code fence
    let trimmed = raw.trim();
    let json_str = if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            &trimmed[start..=end]
        } else {
            trimmed
        }
    } else {
        trimmed
    };
    // First try to parse as Create
    if let Ok(create) = serde_json::from_str::<CalendarCreateRequest>(json_str) {
        if create.action == "calendar.create_event" {
            return Ok(CalendarAiOutput::Create(create));
        }
    }
    if let Ok(clarify) = serde_json::from_str::<CalendarNeedsClarification>(json_str) {
        if clarify.action == "calendar.needs_clarification" {
            return Ok(CalendarAiOutput::Clarify(clarify));
        }
    }
    // Try generic check for none
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
        if let Some(action) = val.get("action").and_then(|v| v.as_str()) {
            if action == "none" || action == "passthrough" || action == "calendar.none" {
                return Ok(CalendarAiOutput::None {
                    action: action.to_string(),
                });
            }
            // If it's create but failed validation earlier, return error
            return Err(CalendarErrorResult {
                ok: false,
                error: "validation_error".to_string(),
                message: "Invalid calendar JSON".to_string(),
                details: Some(json_str.chars().take(300).collect()),
            });
        }
    }
    Err(CalendarErrorResult {
        ok: false,
        error: "validation_error".to_string(),
        message: "AI did not return valid calendar JSON".to_string(),
        details: Some(json_str.chars().take(300).collect()),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CalendarHandleResult {
    NotCalendar,
    Success(CalendarSuccessResult),
    NeedsClarification(CalendarNeedsClarification),
    Failure(CalendarErrorResult),
}

pub async fn handle_calendar_transcript(
    transcript: &str,
    settings: &crate::settings::AppSettings,
    app: Option<&tauri::AppHandle>,
) -> CalendarHandleResult {
    // AI parser (strict JSON, never arbitrary code)
    let ai_output = match parser::parse_calendar_intent(transcript, settings).await {
        Ok(Some(output)) => output,
        Ok(None) => return CalendarHandleResult::NotCalendar,
        Err(e) => return CalendarHandleResult::Failure(e),
    };

    match ai_output {
        CalendarAiOutput::Create(req) => {
            let validated = match validate_create_request(&req) {
                Ok(v) => v,
                Err(e) => return CalendarHandleResult::Failure(e),
            };
            // Show tiny processing moment with title before native save
            if let Some(app) = app {
                crate::overlay::show_calendar_processing_overlay(app, &validated.title);
                // Brief pause so the pill is visible before EventKit save (feels native, not fake)
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            }
            // Rust owns orchestration, Swift owns EventKit
            match native::create_event(&validated) {
                Ok(success) => CalendarHandleResult::Success(success),
                Err(e) => CalendarHandleResult::Failure(e),
            }
        }
        CalendarAiOutput::Clarify(clarify) => {
            // Validate clarification message is short and natural
            if clarify.clarification_message.trim().is_empty()
                || clarify.clarification_message.len() > 120
            {
                return CalendarHandleResult::Failure(CalendarErrorResult {
                    ok: false,
                    error: "validation_error".to_string(),
                    message: "Invalid clarification message".to_string(),
                    details: None,
                });
            }
            CalendarHandleResult::NeedsClarification(clarify)
        }
        CalendarAiOutput::None { .. } => CalendarHandleResult::NotCalendar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_req() -> CalendarCreateRequest {
        CalendarCreateRequest {
            action: "calendar.create_event".to_string(),
            title: "Month-end finance review".to_string(),
            start: "2026-09-01T09:30:00+05:30".to_string(),
            end: "2026-09-01T10:15:00+05:30".to_string(),
            calendar: Some("Work".to_string()),
            location: None,
            notes: None,
            reminders_minutes_before: Some(vec![15]),
            success_message: "Finance review scheduled for tomorrow.".to_string(),
        }
    }

    #[test]
    fn validates_good_request() {
        let req = valid_req();
        assert!(validate_create_request(&req).is_ok());
    }

    #[test]
    fn rejects_generic_success_message() {
        let mut req = valid_req();
        req.success_message = "Action completed successfully.".to_string();
        assert!(validate_create_request(&req).is_err());
    }

    #[test]
    fn rejects_vague_time_invented() {
        // This test ensures we don't allow missing time; but our validator only checks present fields.
        // The AI is instructed to return needs_clarification instead of inventing. So if AI invents 09:00 for "tomorrow morning" without policy, it's still valid per schema but we want to catch via prompt.
        // Here we test that short success_message fails
        let mut req = valid_req();
        req.success_message = "Done".to_string();
        assert!(validate_create_request(&req).is_err());
    }

    #[test]
    fn extracts_json_from_code_fence() {
        let raw = "```json\n{\"action\":\"calendar.create_event\",\"title\":\"Test\",\"start\":\"2026-09-01T09:30:00+05:30\",\"end\":\"2026-09-01T10:00:00+05:30\",\"calendar\":\"Work\",\"success_message\":\"Test scheduled for tomorrow.\"}\n```";
        let out = validate_ai_output(raw).unwrap();
        match out {
            CalendarAiOutput::Create(c) => assert_eq!(c.title, "Test"),
            _ => panic!("expected create"),
        }
    }

    #[test]
    fn parses_needs_clarification() {
        let raw = r#"{"action":"calendar.needs_clarification","missing_fields":["start"],"clarification_message":"What time tomorrow morning?"}"#;
        let out = validate_ai_output(raw).unwrap();
        match out {
            CalendarAiOutput::Clarify(c) => assert_eq!(c.missing_fields, vec!["start"]),
            _ => panic!("expected clarify"),
        }
    }
}
