pub mod parser;
pub mod writer;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsidianTask {
    pub title: String,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub due: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub completed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsidianSection {
    pub heading: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsidianSource {
    pub title: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsidianCreateRequest {
    pub action: String,
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default, rename = "fileKey")]
    pub file_key: Option<String>,
    #[serde(default)]
    pub vaultPath: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub attendees: Option<Vec<String>>,
    #[serde(default)]
    pub agenda: Option<Vec<String>>,
    #[serde(default)]
    pub sections: Option<Vec<ObsidianSection>>,
    #[serde(default)]
    pub decisions: Option<Vec<String>>,
    #[serde(default)]
    pub tasks: Option<Vec<ObsidianTask>>,
    #[serde(default, rename = "actionItems")]
    pub action_items: Option<Vec<ObsidianTask>>,
    #[serde(default, rename = "abstract")]
    pub abstract_text: Option<String>,
    #[serde(default)]
    pub sources: Option<Vec<ObsidianSource>>,
    pub task_status: String,
    pub success_message: String,
    #[serde(default)]
    pub openAfterWrite: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsidianNeedsClarification {
    pub action: String,
    pub missing_fields: Vec<String>,
    pub clarification_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ObsidianAiOutput {
    Create(ObsidianCreateRequest),
    Clarify(ObsidianNeedsClarification),
    None { action: String },
}

#[derive(Debug, Clone)]
pub struct ValidatedObsidianNote {
    pub vault_path: Option<String>,
    pub kind: String,
    pub title: String,
    pub branch: String,
    pub file_key: Option<String>,
    pub mode: Option<String>,
    pub summary: Option<String>,
    pub date: Option<String>,
    pub attendees: Option<Vec<String>>,
    pub agenda: Option<Vec<String>>,
    pub sections: Option<Vec<ObsidianSection>>,
    pub decisions: Option<Vec<String>>,
    pub tasks: Option<Vec<ObsidianTask>>,
    pub action_items: Option<Vec<ObsidianTask>>,
    pub abstract_text: Option<String>,
    pub sources: Option<Vec<ObsidianSource>>,
    pub task_status: String,
    pub success_message: String,
    pub open_after_write: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsidianSuccessResult {
    pub ok: bool,
    pub action: String,
    pub kind: String,
    pub title: String,
    pub path: String,
    pub task_status: String,
    pub success_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsidianErrorResult {
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
        || lower.trim() == "task created"
        || lower.trim() == "task created."
        || lower.trim() == "done"
        || lower.trim() == "done."
}

fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
}

fn is_natural_professional(msg: &str) -> bool {
    // Natural professional should be a sentence, not 2-3 sharp words, contain at least one verb-ish and be warm
    let wc = word_count(msg);
    if wc < 6 || wc > 24 {
        return false;
    }
    // Must contain at least some Obsidian-specific context or natural language, not just "Done"
    let lower = msg.to_lowercase();
    if lower == "done" || lower == "task created" || lower == "note created" {
        return false;
    }
    true
}

pub fn validate_create_request(
    req: &ObsidianCreateRequest,
) -> Result<ValidatedObsidianNote, ObsidianErrorResult> {
    if req.action != "obsidian.create" {
        return Err(ObsidianErrorResult {
            ok: false,
            error: "validation_error".to_string(),
            message: "Invalid action, expected obsidian.create".to_string(),
            details: None,
        });
    }
    let kind = req.kind.trim().to_lowercase();
    if !["todo", "document", "meeting", "research"].contains(&kind.as_str()) {
        return Err(ObsidianErrorResult {
            ok: false,
            error: "validation_error".to_string(),
            message: "Invalid kind, must be todo|document|meeting|research".to_string(),
            details: Some(req.kind.clone()),
        });
    }
    let title = req.title.trim();
    if title.is_empty() || title.len() < 3 || title.len() > 120 {
        return Err(ObsidianErrorResult {
            ok: false,
            error: "validation_error".to_string(),
            message: "Title is required (3-120 chars)".to_string(),
            details: None,
        });
    }
    // Validate task_status
    let task_status = req.task_status.trim().to_lowercase();
    let allowed_status = [
        "done",
        "created",
        "updated",
        "pending",
        "in_progress",
        "completed",
        "in-progress",
    ];
    if !allowed_status.contains(&task_status.as_str()) {
        return Err(ObsidianErrorResult {
            ok: false,
            error: "validation_error".to_string(),
            message:
                "Invalid task_status, must be done|created|updated|pending|in_progress|completed"
                    .to_string(),
            details: Some(req.task_status.clone()),
        });
    }
    // Validate success_message natural professional concise (NOT sharp)
    let success = req.success_message.trim();
    if !is_natural_professional(success) {
        return Err(ObsidianErrorResult {
            ok: false,
            error: "validation_error".to_string(),
            message: "Invalid success_message: must be natural, professional, 6-24 words, warm, specific (not sharp 2-3 words)".to_string(),
            details: Some(success.to_string()),
        });
    }
    if is_generic_success(success) {
        return Err(ObsidianErrorResult {
            ok: false,
            error: "validation_error".to_string(),
            message: "success_message must not be generic".to_string(),
            details: Some(success.to_string()),
        });
    }
    if success.len() < 20 || success.len() > 280 {
        return Err(ObsidianErrorResult {
            ok: false,
            error: "validation_error".to_string(),
            message: "success_message length must be 20-280 chars".to_string(),
            details: Some(format!("len={}", success.len())),
        });
    }

    // Branch handling: default per kind if null
    let branch = req
        .branch
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| match kind.as_str() {
            "todo" => "Tasks".to_string(),
            "meeting" => "Meetings".to_string(),
            "research" => "Research".to_string(),
            _ => "Notes".to_string(),
        });
    if branch.len() > 60
        || branch.contains('/')
        || branch.contains('\\')
        || branch == "."
        || branch == ".."
    {
        return Err(ObsidianErrorResult {
            ok: false,
            error: "validation_error".to_string(),
            message: "Invalid branch/folder".to_string(),
            details: Some(branch),
        });
    }

    // Mode validation
    if let Some(mode) = &req.mode {
        let m = mode.trim().to_lowercase();
        if m != "upsert" && m != "append" {
            return Err(ObsidianErrorResult {
                ok: false,
                error: "validation_error".to_string(),
                message: "Invalid mode, must be upsert or append".to_string(),
                details: Some(mode.clone()),
            });
        }
    }

    // Kind-specific validation
    match kind.as_str() {
        "todo" => {
            let tasks = req.tasks.as_ref().or(req.action_items.as_ref());
            if tasks.is_none() || tasks.unwrap().is_empty() {
                return Err(ObsidianErrorResult {
                    ok: false,
                    error: "validation_error".to_string(),
                    message: "Todo requires at least one task".to_string(),
                    details: None,
                });
            }
            let tasks = tasks.unwrap();
            if tasks.len() > 15 {
                return Err(ObsidianErrorResult {
                    ok: false,
                    error: "validation_error".to_string(),
                    message: "Too many tasks (max 15)".to_string(),
                    details: None,
                });
            }
            for t in tasks {
                if t.title.trim().is_empty() || t.title.len() > 120 {
                    return Err(ObsidianErrorResult {
                        ok: false,
                        error: "validation_error".to_string(),
                        message: "Task title invalid".to_string(),
                        details: Some(t.title.clone()),
                    });
                }
                if let Some(p) = &t.priority {
                    let pl = p.trim().to_lowercase();
                    if !["low", "medium", "high"].contains(&pl.as_str()) {
                        return Err(ObsidianErrorResult {
                            ok: false,
                            error: "validation_error".to_string(),
                            message: "Invalid priority, must be low|medium|high".to_string(),
                            details: Some(p.clone()),
                        });
                    }
                }
                if let Some(due) = &t.due {
                    if chrono::DateTime::parse_from_rfc3339(due).is_err() {
                        return Err(ObsidianErrorResult {
                            ok: false,
                            error: "validation_error".to_string(),
                            message: "Invalid task due date, must be ISO8601 with offset"
                                .to_string(),
                            details: Some(due.clone()),
                        });
                    }
                }
            }
        }
        "document" => {
            let sections_empty = req.sections.as_ref().map(|s| s.is_empty()).unwrap_or(true);
            let summary_empty = req
                .summary
                .as_ref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true);
            if sections_empty && summary_empty {
                return Err(ObsidianErrorResult {
                    ok: false,
                    error: "validation_error".to_string(),
                    message: "Document requires at least one section or summary".to_string(),
                    details: None,
                });
            }
            if let Some(sections) = &req.sections {
                if sections.len() > 12 {
                    return Err(ObsidianErrorResult {
                        ok: false,
                        error: "validation_error".to_string(),
                        message: "Too many sections (max 12)".to_string(),
                        details: None,
                    });
                }
                for s in sections {
                    if s.heading.trim().is_empty() || s.heading.len() > 80 {
                        return Err(ObsidianErrorResult {
                            ok: false,
                            error: "validation_error".to_string(),
                            message: "Section heading invalid".to_string(),
                            details: Some(s.heading.clone()),
                        });
                    }
                    if s.body.trim().is_empty() || s.body.len() > 4000 {
                        return Err(ObsidianErrorResult {
                            ok: false,
                            error: "validation_error".to_string(),
                            message: "Section body invalid".to_string(),
                            details: None,
                        });
                    }
                }
            }
        }
        "meeting" => {
            let has_date = req
                .date
                .as_ref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            let has_sections = req
                .sections
                .as_ref()
                .map(|s| !s.is_empty())
                .unwrap_or(false);
            let has_action = req
                .action_items
                .as_ref()
                .or(req.tasks.as_ref())
                .map(|v| !v.is_empty())
                .unwrap_or(false);
            let has_attendees = req
                .attendees
                .as_ref()
                .map(|v| !v.is_empty())
                .unwrap_or(false);
            let has_agenda = req.agenda.as_ref().map(|v| !v.is_empty()).unwrap_or(false);
            if !has_date && !has_sections && !has_action && !has_attendees && !has_agenda {
                return Err(ObsidianErrorResult {
                    ok: false,
                    error: "validation_error".to_string(),
                    message: "Meeting requires date, attendees, agenda, sections, or action items"
                        .to_string(),
                    details: None,
                });
            }
            if let Some(date) = &req.date {
                if chrono::DateTime::parse_from_rfc3339(date).is_err() {
                    return Err(ObsidianErrorResult {
                        ok: false,
                        error: "validation_error".to_string(),
                        message: "Invalid meeting date, must be ISO8601 with offset".to_string(),
                        details: Some(date.clone()),
                    });
                }
            }
        }
        "research" => {
            let has_abstract = req
                .abstract_text
                .as_ref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            let has_sections = req
                .sections
                .as_ref()
                .map(|s| !s.is_empty())
                .unwrap_or(false);
            let has_sources = req.sources.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
            if !has_abstract && !has_sections && !has_sources {
                return Err(ObsidianErrorResult {
                    ok: false,
                    error: "validation_error".to_string(),
                    message: "Research requires abstract, sections, or sources".to_string(),
                    details: None,
                });
            }
        }
        _ => {}
    }

    // Optional fields length checks
    if let Some(summary) = &req.summary {
        if summary.len() > 2000 {
            return Err(ObsidianErrorResult {
                ok: false,
                error: "validation_error".to_string(),
                message: "Summary too long".to_string(),
                details: None,
            });
        }
    }
    if let Some(abs) = &req.abstract_text {
        if abs.len() > 3000 {
            return Err(ObsidianErrorResult {
                ok: false,
                error: "validation_error".to_string(),
                message: "Abstract too long".to_string(),
                details: None,
            });
        }
    }

    Ok(ValidatedObsidianNote {
        vault_path: req.vaultPath.clone(),
        kind: kind.clone(),
        title: title.to_string(),
        branch,
        file_key: req
            .file_key
            .clone()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        mode: req.mode.clone(),
        summary: req.summary.clone(),
        date: req.date.clone(),
        attendees: req.attendees.clone(),
        agenda: req.agenda.clone(),
        sections: req.sections.clone(),
        decisions: req.decisions.clone(),
        tasks: req.tasks.clone().or_else(|| req.action_items.clone()),
        action_items: req.action_items.clone().or_else(|| req.tasks.clone()),
        abstract_text: req.abstract_text.clone(),
        sources: req.sources.clone(),
        task_status: task_status,
        success_message: success.to_string(),
        open_after_write: req.openAfterWrite,
    })
}

pub fn validate_ai_output(raw: &str) -> Result<ObsidianAiOutput, ObsidianErrorResult> {
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
    if let Ok(create) = serde_json::from_str::<ObsidianCreateRequest>(json_str) {
        if create.action == "obsidian.create" {
            return Ok(ObsidianAiOutput::Create(create));
        }
    }
    if let Ok(clarify) = serde_json::from_str::<ObsidianNeedsClarification>(json_str) {
        if clarify.action == "obsidian.needs_clarification" {
            return Ok(ObsidianAiOutput::Clarify(clarify));
        }
    }
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
        if let Some(action) = val.get("action").and_then(|v| v.as_str()) {
            if action == "none" || action == "passthrough" || action == "obsidian.none" {
                return Ok(ObsidianAiOutput::None {
                    action: action.to_string(),
                });
            }
            return Err(ObsidianErrorResult {
                ok: false,
                error: "validation_error".to_string(),
                message: "Invalid obsidian JSON".to_string(),
                details: Some(json_str.chars().take(300).collect()),
            });
        }
    }
    Err(ObsidianErrorResult {
        ok: false,
        error: "validation_error".to_string(),
        message: "AI did not return valid obsidian JSON".to_string(),
        details: Some(json_str.chars().take(300).collect()),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObsidianHandleResult {
    NotObsidian,
    Success(ObsidianSuccessResult),
    NeedsClarification(ObsidianNeedsClarification),
    Failure(ObsidianErrorResult),
}

pub async fn handle_obsidian_transcript(
    transcript: &str,
    settings: &crate::settings::AppSettings,
    app: Option<&tauri::AppHandle>,
) -> ObsidianHandleResult {
    let ai_output = match parser::parse_obsidian_intent(transcript, settings).await {
        Ok(Some(output)) => output,
        Ok(None) => return ObsidianHandleResult::NotObsidian,
        Err(e) => return ObsidianHandleResult::Failure(e),
    };

    match ai_output {
        ObsidianAiOutput::Create(req) => {
            let validated = match validate_create_request(&req) {
                Ok(v) => v,
                Err(e) => return ObsidianHandleResult::Failure(e),
            };
            if let Some(app) = app {
                crate::overlay::show_obsidian_processing_overlay(app, &validated.title);
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            }
            match writer::execute_write(&validated) {
                Ok(path) => {
                    let result = ObsidianSuccessResult {
                        ok: true,
                        action: "obsidian.create".to_string(),
                        kind: validated.kind.clone(),
                        title: validated.title.clone(),
                        path: path.to_string_lossy().to_string(),
                        task_status: validated.task_status.clone(),
                        success_message: validated.success_message.clone(),
                    };
                    ObsidianHandleResult::Success(result)
                }
                Err(e) => ObsidianHandleResult::Failure(e),
            }
        }
        ObsidianAiOutput::Clarify(clarify) => {
            if clarify.clarification_message.trim().is_empty()
                || clarify.clarification_message.len() > 160
            {
                return ObsidianHandleResult::Failure(ObsidianErrorResult {
                    ok: false,
                    error: "validation_error".to_string(),
                    message: "Invalid clarification message".to_string(),
                    details: None,
                });
            }
            ObsidianHandleResult::NeedsClarification(clarify)
        }
        ObsidianAiOutput::None { .. } => ObsidianHandleResult::NotObsidian,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_todo_req() -> ObsidianCreateRequest {
        ObsidianCreateRequest {
            action: "obsidian.create".to_string(),
            kind: "todo".to_string(),
            title: "Maya Chen — Follow-up".to_string(),
            branch: Some("Tasks".to_string()),
            file_key: None,
            vaultPath: None,
            mode: None,
            summary: None,
            date: None,
            attendees: None,
            agenda: None,
            sections: None,
            decisions: None,
            tasks: Some(vec![ObsidianTask {
                title: "Follow up with Maya Chen".to_string(),
                owner: None,
                due: Some("2026-09-02T17:00:00+05:30".to_string()),
                priority: Some("high".to_string()),
                completed: Some(false),
            }]),
            action_items: None,
            abstract_text: None,
            sources: None,
            task_status: "done".to_string(),
            success_message:
                "Captured your follow-up with Maya Chen in Obsidian, due tomorrow at 5 PM."
                    .to_string(),
            openAfterWrite: None,
        }
    }

    #[test]
    fn validates_good_todo() {
        let req = valid_todo_req();
        assert!(validate_create_request(&req).is_ok());
    }

    #[test]
    fn rejects_sharp_success_message() {
        let mut req = valid_todo_req();
        req.success_message = "Task created.".to_string();
        assert!(validate_create_request(&req).is_err());
    }

    #[test]
    fn rejects_generic_success() {
        let mut req = valid_todo_req();
        req.success_message = "Action completed successfully.".to_string();
        assert!(validate_create_request(&req).is_err());
    }

    #[test]
    fn rejects_missing_tasks_for_todo() {
        let mut req = valid_todo_req();
        req.tasks = None;
        req.action_items = None;
        assert!(validate_create_request(&req).is_err());
    }

    #[test]
    fn validates_meeting() {
        let mut req = valid_todo_req();
        req.kind = "meeting".to_string();
        req.title = "Q4 Security Review".to_string();
        req.date = Some("2026-09-02T10:00:00+05:30".to_string());
        req.attendees = Some(vec!["Maya Chen".to_string()]);
        req.tasks = None;
        req.action_items = Some(vec![ObsidianTask {
            title: "Finish doc".to_string(),
            owner: None,
            due: None,
            priority: None,
            completed: Some(false),
        }]);
        req.success_message =
            "Meeting notes for Q4 review are now in Obsidian, scheduled for tomorrow at 10 AM."
                .to_string();
        assert!(validate_create_request(&req).is_ok());
    }

    #[test]
    fn extracts_json_from_code_fence() {
        let raw = "```json\n{\"action\":\"obsidian.create\",\"kind\":\"todo\",\"title\":\"Test\",\"tasks\":[{\"title\":\"Do thing\"}],\"task_status\":\"done\",\"success_message\":\"Captured your test task in Obsidian, ready for tomorrow.\"}\n```";
        let out = validate_ai_output(raw).unwrap();
        match out {
            ObsidianAiOutput::Create(c) => assert_eq!(c.title, "Test"),
            _ => panic!("expected create"),
        }
    }

    #[test]
    fn parses_needs_clarification() {
        let raw = r#"{"action":"obsidian.needs_clarification","missing_fields":["tasks"],"clarification_message":"What tasks should I add?"}"#;
        let out = validate_ai_output(raw).unwrap();
        match out {
            ObsidianAiOutput::Clarify(c) => assert_eq!(c.missing_fields, vec!["tasks"]),
            _ => panic!("expected clarify"),
        }
    }

    #[test]
    fn rejects_invalid_task_status() {
        let mut req = valid_todo_req();
        req.task_status = "blazing".to_string();
        assert!(validate_create_request(&req).is_err());
    }
}
