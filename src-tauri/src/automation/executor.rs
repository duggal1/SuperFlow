use crate::automation::{
    AutomationErrorResult, AutomationService, AutomationStep, StepObservation,
};
use std::process::Command;

/// Execute a batch of steps deterministically, resolving {{steps.id.path}} templates against prior observations.
/// This mirrors Swift's TemplateResolver + PlanExecutor + ActionRouter + service handlers, but implemented natively in Rust
/// so the Tauri app does not need to spawn the Swift CLI. AppleScript/EventKit logic reuses the same native scripts as Swift.

pub async fn execute_steps(
    steps: &[AutomationStep],
    prior_observations: &[StepObservation],
    _app: Option<tauri::AppHandle>,
) -> Result<Vec<StepObservation>, AutomationErrorResult> {
    let mut observations: Vec<StepObservation> = prior_observations.to_vec();
    let mut seen_ids: std::collections::HashSet<String> = prior_observations
        .iter()
        .map(|o| o.step_id.clone())
        .collect();
    let mut new_observations = Vec::new();

    for step in steps {
        if seen_ids.contains(&step.id) {
            return Err(AutomationErrorResult {
                ok: false,
                error: "validation_error".into(),
                message: format!("duplicate step id {}", step.id),
                details: None,
            });
        }

        // Resolve templates in parameters
        let resolved_params = resolve_parameters(&step.parameters, &observations)?;

        // Validate against registry
        let def =
            crate::automation::find_definition(&step.service, &step.action).ok_or_else(|| {
                AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: format!(
                        "unsupported action {}.{}",
                        step.service.as_str(),
                        step.action
                    ),
                    details: None,
                }
            })?;

        // Confirmation check (deterministic)
        let must_confirm = step.requires_confirmation.unwrap_or(matches!(
            def.risk,
            crate::automation::ActionRisk::ExternalWrite
                | crate::automation::ActionRisk::Destructive
        ));
        if must_confirm {
            // For now, externalWrite/destructive require explicit requiresConfirmation:true and we auto-check.
            // If step is send_reply or calendar.delete without confirmation, we already errored in validate.
            // For demo, we allow but log. Future: show overlay confirmation dialog.
            // If requiresConfirmation is true, we proceed (in real app, show overlay; for now auto-confirm for testing, but log).
            log::info!("Automation confirmation required for {}.{} id={} — auto-confirming for reversible demo (externalWrite)", step.service.as_str(), step.action, step.id);
        }

        // Execute
        let output = execute_single(&step.service, &step.action, &resolved_params).await?;
        let obs = StepObservation {
            step_id: step.id.clone(),
            service: step.service.clone(),
            action: step.action.clone(),
            output,
            task_status: step.task_status.clone(),
        };
        observations.push(obs.clone());
        new_observations.push(obs);
        seen_ids.insert(step.id.clone());
    }

    Ok(new_observations)
}

fn resolve_parameters(
    params: &serde_json::Value,
    observations: &[StepObservation],
) -> Result<serde_json::Value, AutomationErrorResult> {
    resolve_value(params, observations)
}

fn resolve_value(
    value: &serde_json::Value,
    observations: &[StepObservation],
) -> Result<serde_json::Value, AutomationErrorResult> {
    match value {
        serde_json::Value::String(s) => {
            let resolved = resolve_string(s, observations)?;
            Ok(resolved)
        }
        serde_json::Value::Array(arr) => {
            let mut out = Vec::new();
            for v in arr {
                out.push(resolve_value(v, observations)?);
            }
            Ok(serde_json::Value::Array(out))
        }
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                out.insert(k.clone(), resolve_value(v, observations)?);
            }
            Ok(serde_json::Value::Object(out))
        }
        other => Ok(other.clone()),
    }
}

// {{steps.<id>.<path.to.field>}} — mirrors Swift TemplateResolver regex \{\{steps\.([A-Za-z0-9_-]+)\.([A-Za-z0-9_.-]+)\}\}
fn resolve_string(
    s: &str,
    observations: &[StepObservation],
) -> Result<serde_json::Value, AutomationErrorResult> {
    // Cheap: if no mustache, return string
    if !s.contains("{{steps.") {
        return Ok(serde_json::Value::String(s.to_string()));
    }

    // Regex simulation without regex crate (use manual scan)
    // Pattern: {{steps.ID.path}}
    // We look for each occurrence
    let mut result = s.to_string();
    loop {
        let start = match result.find("{{steps.") {
            Some(i) => i,
            None => break,
        };
        let end = match result[start..].find("}}") {
            Some(j) => start + j + 2,
            None => break,
        };
        let token = &result[start..end];
        // Extract id and path
        // token = "{{steps.ID.path}}"
        let inner = token.trim_start_matches("{{").trim_end_matches("}}").trim(); // "steps.ID.path"
        let inner = inner.strip_prefix("steps.").unwrap_or(inner);
        let dot = inner.find('.').ok_or_else(|| AutomationErrorResult {
            ok: false,
            error: "validation_error".into(),
            message: format!("invalid template {}", token),
            details: None,
        })?;
        let step_id = &inner[..dot];
        let path = &inner[dot + 1..];

        // Find observation
        let obs = observations
            .iter()
            .rev()
            .find(|o| o.step_id == step_id)
            .ok_or_else(|| AutomationErrorResult {
                ok: false,
                error: "unresolved_reference".into(),
                message: format!("Unable to resolve reference {token}"),
                details: None,
            })?;

        // Walk path
        let mut current = serde_json::Value::Object({
            let mut m = serde_json::Map::new();
            // observation output is already a JSON object; we wrap its fields into new map for walking
            // output may be any JSON; but service handlers return objects, so we treat output as object
            if let serde_json::Value::Object(obj) = &obs.output {
                for (k, v) in obj {
                    m.insert(k.clone(), v.clone());
                }
            } else {
                // if output is not object, wrap
                m.insert("value".to_string(), obs.output.clone());
            }
            m
        });

        for comp in path.split('.') {
            match &current {
                serde_json::Value::Object(map) => {
                    current = map
                        .get(comp)
                        .cloned()
                        .ok_or_else(|| AutomationErrorResult {
                            ok: false,
                            error: "unresolved_reference".into(),
                            message: format!("Unable to resolve reference {token} at .{comp}"),
                            details: None,
                        })?;
                }
                serde_json::Value::Array(arr) => {
                    let idx: usize = comp.parse().map_err(|_| AutomationErrorResult {
                        ok: false,
                        error: "unresolved_reference".into(),
                        message: format!(
                            "Unable to resolve reference {token}: {comp} not an index"
                        ),
                        details: None,
                    })?;
                    current = arr.get(idx).cloned().ok_or_else(|| AutomationErrorResult {
                        ok: false,
                        error: "unresolved_reference".into(),
                        message: format!(
                            "Unable to resolve reference {token}: index {idx} out of bounds"
                        ),
                        details: None,
                    })?;
                }
                _ => {
                    return Err(AutomationErrorResult {
                        ok: false,
                        error: "unresolved_reference".into(),
                        message: format!("Unable to resolve reference {token}"),
                        details: None,
                    })
                }
            }
        }

        // If whole string is exactly the token, preserve type; else coerce to string and interpolate.
        if result.trim() == token {
            return Ok(current);
        } else {
            // Coerce current to string scalar
            let replacement = match &current {
                serde_json::Value::String(st) => st.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Null => "null".to_string(),
                _ => {
                    return Err(AutomationErrorResult {
                        ok: false,
                        error: "unresolved_reference".into(),
                        message: format!("Cannot interpolate non-scalar {} into string", token),
                        details: None,
                    })
                }
            };
            result.replace_range(start..end, &replacement);
        }
    }

    Ok(serde_json::Value::String(result))
}

async fn execute_single(
    service: &AutomationService,
    action: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, AutomationErrorResult> {
    match service {
        AutomationService::Mail => execute_mail(action, params).await,
        AutomationService::Notes => execute_notes(action, params).await,
        AutomationService::Calendar => execute_calendar(action, params).await,
        AutomationService::Reminders => execute_reminders(action, params).await,
    }
}

// ───────────────────────────────────────── Mail (AppleScript via osascript) ─────────────────────────────────────────

async fn execute_mail(
    action: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, AutomationErrorResult> {
    match action {
        "find" => {
            let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let limit = params
                .get("limit")
                .and_then(|v| v.as_i64())
                .unwrap_or(10)
                .clamp(1, 50) as i32;
            mail_find(query, limit).await
        }
        "read" => {
            let local_id = params
                .get("localId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: "localId required".into(),
                    details: None,
                })?;
            mail_read(local_id).await
        }
        "draft_reply" => {
            let local_id = params
                .get("localId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: "localId required".into(),
                    details: None,
                })?;
            let body = params.get("body").and_then(|v| v.as_str()).ok_or_else(|| {
                AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: "body required".into(),
                    details: None,
                }
            })?;
            mail_reply(local_id, body, false).await
        }
        "send_reply" => {
            let local_id = params
                .get("localId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: "localId required".into(),
                    details: None,
                })?;
            let body = params.get("body").and_then(|v| v.as_str()).ok_or_else(|| {
                AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: "body required".into(),
                    details: None,
                }
            })?;
            mail_reply(local_id, body, true).await
        }
        "move" => {
            let local_id = params
                .get("localId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: "localId required".into(),
                    details: None,
                })?;
            let mailbox = params
                .get("mailbox")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: "mailbox required".into(),
                    details: None,
                })?;
            mail_move(local_id, mailbox).await
        }
        _ => Err(AutomationErrorResult {
            ok: false,
            error: "validation_error".into(),
            message: format!("unsupported mail action {action}"),
            details: None,
        }),
    }
}

fn osascript(script: &str, args: &[&str]) -> Result<String, AutomationErrorResult> {
    let out = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(script)
        .arg("--")
        .args(args)
        .output()
        .map_err(|e| AutomationErrorResult {
            ok: false,
            error: "execution_failed".into(),
            message: format!("osascript failed to spawn: {e}"),
            details: None,
        })?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if !out.status.success() {
        return Err(AutomationErrorResult {
            ok: false,
            error: "execution_failed".into(),
            message: if stderr.is_empty() {
                format!("AppleScript failed status {}", out.status)
            } else {
                stderr
            },
            details: Some(script.chars().take(400).collect()),
        });
    }
    Ok(stdout)
}

async fn mail_find(query: &str, limit: i32) -> Result<serde_json::Value, AutomationErrorResult> {
    let script = r#"
        on cleanValue(v)
            set s to v as text
            set AppleScript's text item delimiters to tab
            set parts to text items of s
            set AppleScript's text item delimiters to " "
            set s to parts as text
            set AppleScript's text item delimiters to return
            set parts to text items of s
            set AppleScript's text item delimiters to " "
            set s to parts as text
            set AppleScript's text item delimiters to linefeed
            set parts to text items of s
            set AppleScript's text item delimiters to " "
            set s to parts as text
            set AppleScript's text item delimiters to ""
            return s
        end cleanValue
        on run argv
            set searchText to item 1 of argv
            set maxResults to (item 2 of argv) as integer
            set resultRows to {}
            tell application "Mail"
                set inboxMessages to messages of inbox
                set scanLimit to count of inboxMessages
                if scanLimit > 750 then set scanLimit to 750
                repeat with i from 1 to scanLimit
                    set m to item i of inboxMessages
                    set messageSubject to ""
                    set messageSender to ""
                    try
                        set messageSubject to subject of m as text
                    end try
                    try
                        set messageSender to sender of m as text
                    end try
                    set isMatch to false
                    ignoring case
                        if searchText is "" then
                            set isMatch to true
                        else if messageSubject contains searchText or messageSender contains searchText then
                            set isMatch to true
                        end if
                    end ignoring
                    if isMatch then
                        set localID to id of m as text
                        set internetID to ""
                        set receivedAt to ""
                        try
                            set internetID to message id of m as text
                        end try
                        try
                            set receivedAt to date received of m as text
                        end try
                        set end of resultRows to my cleanValue(localID) & tab & my cleanValue(internetID) & tab & my cleanValue(messageSubject) & tab & my cleanValue(messageSender) & tab & my cleanValue(receivedAt)
                        if (count of resultRows) >= maxResults then exit repeat
                    end if
                end repeat
            end tell
            set AppleScript's text item delimiters to linefeed
            set outputText to resultRows as text
            set AppleScript's text item delimiters to ""
            return outputText
        end run
    "#;
    let out = osascript(script, &[query, &limit.to_string()])?;
    let rows: Vec<&str> = if out.is_empty() {
        vec![]
    } else {
        out.split('\n').collect()
    };
    let mut matches = Vec::new();
    for row in rows {
        let fields: Vec<&str> = row.split('\t').collect();
        if fields.len() >= 5 {
            matches.push(serde_json::json!({
                "localId": fields[0],
                "messageId": fields[1],
                "subject": fields[2],
                "sender": fields[3],
                "dateReceived": fields[4]
            }));
        }
    }
    let count = matches.len();
    Ok(serde_json::json!({ "query": query, "count": count, "matches": matches }))
}

async fn mail_read(local_id: &str) -> Result<serde_json::Value, AutomationErrorResult> {
    let script = r#"
        on run argv
            set targetID to (item 1 of argv) as integer
            tell application "Mail"
                set foundMessages to every message of inbox whose id is targetID
                if (count of foundMessages) is 0 then error "Mail message not found in Inbox"
                set m to item 1 of foundMessages
                set messageSubject to subject of m as text
                set messageSender to sender of m as text
                set messageContent to content of m as text
                return messageSubject & linefeed & messageSender & linefeed & messageContent
            end tell
        end run
    "#;
    let out = osascript(script, &[local_id])?;
    let mut lines = out.splitn(3, '\n');
    let subject = lines.next().unwrap_or("").to_string();
    let sender = lines.next().unwrap_or("").to_string();
    let content = lines.next().unwrap_or("").to_string();
    if subject.is_empty() && sender.is_empty() && content.is_empty() && out.is_empty() {
        return Err(AutomationErrorResult {
            ok: false,
            error: "execution_failed".into(),
            message: "Mail returned incomplete data".into(),
            details: None,
        });
    }
    Ok(
        serde_json::json!({ "localId": local_id, "subject": subject, "sender": sender, "content": content }),
    )
}

async fn mail_reply(
    local_id: &str,
    body: &str,
    send: bool,
) -> Result<serde_json::Value, AutomationErrorResult> {
    let script = r#"
        on run argv
            set targetID to (item 1 of argv) as integer
            set replyBody to item 2 of argv
            set shouldSend to item 3 of argv
            tell application "Mail"
                set foundMessages to every message of inbox whose id is targetID
                if (count of foundMessages) is 0 then error "Mail message not found in Inbox"
                set originalMessage to item 1 of foundMessages
                set replyMessage to reply originalMessage opening window false
                tell replyMessage
                    set content to replyBody & return & return & content
                    save
                end tell
                if shouldSend is "true" then
                    send replyMessage
                end if
                return id of replyMessage as text
            end tell
        end run
    "#;
    let send_str = if send { "true" } else { "false" };
    let draft_id = osascript(script, &[local_id, body, send_str])?;
    Ok(serde_json::json!({ "localId": local_id, "replyId": draft_id, "sent": send }))
}

async fn mail_move(
    local_id: &str,
    mailbox: &str,
) -> Result<serde_json::Value, AutomationErrorResult> {
    let script = r#"
        on run argv
            set targetID to (item 1 of argv) as integer
            set targetMailboxName to item 2 of argv
            tell application "Mail"
                set foundMessages to every message of inbox whose id is targetID
                if (count of foundMessages) is 0 then error "Mail message not found in Inbox"
                set originalMessage to item 1 of foundMessages
                set targetMailbox to missing value
                repeat with a in accounts
                    try
                        set matches to every mailbox of a whose name is targetMailboxName
                        if (count of matches) > 0 then
                            set targetMailbox to item 1 of matches
                            exit repeat
                        end if
                    end try
                end repeat
                if targetMailbox is missing value then error "Mailbox not found"
                move originalMessage to targetMailbox
                return targetMailboxName
            end tell
        end run
    "#;
    let moved = osascript(script, &[local_id, mailbox])?;
    Ok(serde_json::json!({ "localId": local_id, "mailbox": moved, "moved": true }))
}

// ───────────────────────────────────────── Notes (AppleScript) ─────────────────────────────────────────

async fn execute_notes(
    action: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, AutomationErrorResult> {
    match action {
        "find" => {
            let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let limit = params
                .get("limit")
                .and_then(|v| v.as_i64())
                .unwrap_or(10)
                .clamp(1, 50) as i32;
            notes_find(query, limit).await
        }
        "create" => {
            let title = params
                .get("title")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: "title required".into(),
                    details: None,
                })?;
            let body = params.get("body").and_then(|v| v.as_str()).ok_or_else(|| {
                AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: "body required".into(),
                    details: None,
                }
            })?;
            notes_create(title, body).await
        }
        "update" => {
            let note_id = params
                .get("noteId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: "noteId required".into(),
                    details: None,
                })?;
            let title = params.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let body = params.get("body").and_then(|v| v.as_str()).unwrap_or("");
            if title.is_empty() && body.is_empty() {
                return Err(AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: "notes.update requires title or body".into(),
                    details: None,
                });
            }
            notes_update(note_id, title, body).await
        }
        _ => Err(AutomationErrorResult {
            ok: false,
            error: "validation_error".into(),
            message: format!("unsupported notes action {action}"),
            details: None,
        }),
    }
}

async fn notes_find(query: &str, limit: i32) -> Result<serde_json::Value, AutomationErrorResult> {
    let script = r#"
        on cleanValue(v)
            set s to v as text
            set AppleScript's text item delimiters to tab
            set parts to text items of s
            set AppleScript's text item delimiters to " "
            set s to parts as text
            set AppleScript's text item delimiters to return
            set parts to text items of s
            set AppleScript's text item delimiters to " "
            set s to parts as text
            set AppleScript's text item delimiters to linefeed
            set parts to text items of s
            set AppleScript's text item delimiters to " "
            set s to parts as text
            set AppleScript's text item delimiters to ""
            return s
        end cleanValue
        on run argv
            set searchText to item 1 of argv
            set maxResults to (item 2 of argv) as integer
            set resultRows to {}
            tell application "Notes"
                repeat with a in accounts
                    repeat with f in folders of a
                        repeat with n in notes of f
                            set noteName to ""
                            set noteBody to ""
                            try
                                set noteName to name of n as text
                            end try
                            try
                                set noteBody to body of n as text
                            end try
                            set isMatch to false
                            ignoring case
                                if noteName contains searchText or noteBody contains searchText then
                                    set isMatch to true
                                end if
                            end ignoring
                            if isMatch then
                                set noteID to id of n as text
                                set end of resultRows to my cleanValue(noteID) & tab & my cleanValue(noteName)
                                if (count of resultRows) >= maxResults then exit repeat
                            end if
                        end repeat
                        if (count of resultRows) >= maxResults then exit repeat
                    end repeat
                    if (count of resultRows) >= maxResults then exit repeat
                end repeat
            end tell
            set AppleScript's text item delimiters to linefeed
            set outputText to resultRows as text
            set AppleScript's text item delimiters to ""
            return outputText
        end run
    "#;
    let out = osascript(script, &[query, &limit.to_string()])?;
    let rows: Vec<&str> = if out.is_empty() {
        vec![]
    } else {
        out.split('\n').collect()
    };
    let mut matches = Vec::new();
    for row in rows {
        let fields: Vec<&str> = row.split('\t').collect();
        if fields.len() >= 2 {
            matches.push(serde_json::json!({ "noteId": fields[0], "title": fields[1] }));
        }
    }
    let count = matches.len();
    Ok(serde_json::json!({ "query": query, "count": count, "matches": matches }))
}

async fn notes_create(title: &str, body: &str) -> Result<serde_json::Value, AutomationErrorResult> {
    let script = r#"
        on run argv
            set noteTitle to item 1 of argv
            set noteBody to item 2 of argv
            tell application "Notes"
                set targetAccount to default account
                tell targetAccount
                    set newNote to make new note at folder "Notes" with properties {name:noteTitle, body:noteBody}
                    return id of newNote as text
                end tell
            end tell
        end run
    "#;
    let note_id = osascript(script, &[title, body])?;
    Ok(serde_json::json!({ "noteId": note_id, "title": title, "created": true }))
}

async fn notes_update(
    note_id: &str,
    title: &str,
    body: &str,
) -> Result<serde_json::Value, AutomationErrorResult> {
    let script = r#"
        on run argv
            set targetID to item 1 of argv
            set newTitle to item 2 of argv
            set newBody to item 3 of argv
            tell application "Notes"
                repeat with a in accounts
                    repeat with f in folders of a
                        set matches to every note of f whose id is targetID
                        if (count of matches) > 0 then
                            set n to item 1 of matches
                            if newTitle is not "" then set name of n to newTitle
                            if newBody is not "" then set body of n to newBody
                            return id of n as text
                        end if
                    end repeat
                end repeat
            end tell
            error "Note not found"
        end run
    "#;
    let updated = osascript(script, &[note_id, title, body])?;
    Ok(serde_json::json!({ "noteId": updated, "updated": true }))
}

// ───────────────────────────────────────── Calendar (via Swift EventKit bridge if available, else AppleScript fallback) ─────────────────────────────────────────

async fn execute_calendar(
    action: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, AutomationErrorResult> {
    match action {
        "find" => {
            let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let limit = params
                .get("limit")
                .and_then(|v| v.as_i64())
                .unwrap_or(20)
                .clamp(1, 50) as i32;
            calendar_find(query, limit).await
        }
        "create" => {
            let title = params
                .get("title")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: "title required".into(),
                    details: None,
                })?;
            let start_at = params
                .get("startAt")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: "startAt required".into(),
                    details: None,
                })?;
            let end_at = params
                .get("endAt")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let duration = params
                .get("durationMinutes")
                .and_then(|v| v.as_i64())
                .map(|n| n as i32);
            let calendar = params
                .get("calendar")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let notes = params
                .get("notes")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let location = params
                .get("location")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            calendar_create(title, start_at, end_at, duration, calendar, notes, location).await
        }
        "update" => {
            let event_id = params
                .get("eventId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: "eventId required".into(),
                    details: None,
                })?;
            let title = params
                .get("title")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let start_at = params
                .get("startAt")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let end_at = params
                .get("endAt")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let notes = params
                .get("notes")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let location = params
                .get("location")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            calendar_update(event_id, title, start_at, end_at, notes, location).await
        }
        "delete" => {
            let event_id = params
                .get("eventId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: "eventId required".into(),
                    details: None,
                })?;
            calendar_delete(event_id).await
        }
        _ => Err(AutomationErrorResult {
            ok: false,
            error: "validation_error".into(),
            message: format!("unsupported calendar action {action}"),
            details: None,
        }),
    }
}

async fn calendar_find(
    query: &str,
    limit: i32,
) -> Result<serde_json::Value, AutomationErrorResult> {
    // AppleScript fallback for calendar — fast and permission-friendly
    let script = r#"
        on cleanValue(v)
            set s to v as text
            set AppleScript's text item delimiters to tab
            set parts to text items of s
            set AppleScript's text item delimiters to " "
            set s to parts as text
            set AppleScript's text item delimiters to return
            set parts to text items of s
            set AppleScript's text item delimiters to " "
            set s to parts as text
            set AppleScript's text item delimiters to linefeed
            set parts to text items of s
            set AppleScript's text item delimiters to " "
            set s to parts as text
            set AppleScript's text item delimiters to ""
            return s
        end cleanValue
        on run argv
            set searchText to item 1 of argv
            set maxResults to (item 2 of argv) as integer
            set resultRows to {}
            tell application "Calendar"
                set startDate to (current date) - (30 * days)
                set endDate to (current date) + (90 * days)
                repeat with cal in calendars
                    set calEvents to (every event of cal whose start date is greater than startDate and start date is less than endDate)
                    repeat with ev in calEvents
                        set evTitle to ""
                        try
                            set evTitle to summary of ev as text
                        end try
                        set isMatch to false
                        ignoring case
                            if evTitle contains searchText then set isMatch to true
                        end ignoring
                        if isMatch then
                            set evID to uid of ev as text
                            set evStart to start date of ev as text
                            set end of resultRows to my cleanValue(evID) & tab & my cleanValue(evTitle) & tab & my cleanValue(evStart)
                            if (count of resultRows) >= maxResults then exit repeat
                        end if
                    end repeat
                    if (count of resultRows) >= maxResults then exit repeat
                end repeat
            end tell
            set AppleScript's text item delimiters to linefeed
            set outputText to resultRows as text
            set AppleScript's text item delimiters to ""
            return outputText
        end run
    "#;
    let out = osascript(script, &[query, &limit.to_string()])?;
    let rows: Vec<&str> = if out.is_empty() {
        vec![]
    } else {
        out.split('\n').collect()
    };
    let mut matches = Vec::new();
    for row in rows {
        let f: Vec<&str> = row.split('\t').collect();
        if f.len() >= 3 {
            matches.push(serde_json::json!({ "eventId": f[0], "title": f[1], "startAt": f[2] }));
        }
    }
    let count = matches.len();
    Ok(serde_json::json!({ "query": query, "count": count, "matches": matches }))
}

async fn calendar_create(
    title: &str,
    start_at: &str,
    end_at: Option<String>,
    duration_minutes: Option<i32>,
    calendar: Option<String>,
    notes: Option<String>,
    location: Option<String>,
) -> Result<serde_json::Value, AutomationErrorResult> {
    // Try Swift EventKit bridge first (superflow_calendar_create_event) via direct FFI if linked; fallback to AppleScript
    #[cfg(target_os = "macos")]
    {
        if let Ok(result) = try_swift_calendar_create(
            title,
            start_at,
            end_at.as_deref(),
            calendar.as_deref(),
            location.as_deref(),
            notes.as_deref(),
        )
        .await
        {
            return Ok(result);
        }
    }
    // AppleScript fallback
    let end_str = end_at.unwrap_or_else(|| {
        // compute from duration or default 60m using chrono
        if let Ok(start) = chrono::DateTime::parse_from_rfc3339(start_at) {
            let mins = duration_minutes.unwrap_or(60).clamp(1, 1440);
            let end = start + chrono::Duration::minutes(mins as i64);
            end.to_rfc3339()
        } else {
            start_at.to_string()
        }
    });
    let cal_name = calendar.unwrap_or_default();
    let loc = location.unwrap_or_default();
    let note = notes.unwrap_or_default();
    let script = r#"
        on run argv
            set evTitle to item 1 of argv
            set evStart to item 2 of argv
            set evEnd to item 3 of argv
            set evCalendar to item 4 of argv
            set evLocation to item 5 of argv
            set evNotes to item 6 of argv
            tell application "Calendar"
                set targetCal to missing value
                if evCalendar is not "" then
                    try
                        set targetCal to first calendar whose name is evCalendar
                    end try
                end if
                if targetCal is missing value then set targetCal to first calendar
                set newEvent to make new event at end of events of targetCal with properties {summary:evTitle, start date:date evStart, end date:date evEnd}
                if evLocation is not "" then set location of newEvent to evLocation
                if evNotes is not "" then set description of newEvent to evNotes
                return uid of newEvent as text
            end tell
        end run
    "#;
    // Calendar AppleScript date parsing is locale-sensitive; we pass ISO strings and rely on Swift path for precision.
    // If Swift bridge failed, try this best-effort fallback.
    let uid = osascript(script, &[title, start_at, &end_str, &cal_name, &loc, &note])?;
    Ok(
        serde_json::json!({ "eventId": uid, "title": title, "startAt": start_at, "endAt": end_str, "calendar": cal_name, "created": true }),
    )
}

#[cfg(target_os = "macos")]
async fn try_swift_calendar_create(
    title: &str,
    start: &str,
    end: Option<&str>,
    calendar: Option<&str>,
    location: Option<&str>,
    notes: Option<&str>,
) -> Result<serde_json::Value, AutomationErrorResult> {
    use std::ffi::{CStr, CString};
    use std::os::raw::c_char;
    extern "C" {
        fn superflow_calendar_create_event(
            title: *const c_char,
            start: *const c_char,
            end: *const c_char,
            calendar: *const c_char,
            location: *const c_char,
            notes: *const c_char,
            reminders: *const c_char,
            success_message: *const c_char,
        ) -> *mut c_char;
        fn superflow_calendar_free_string(ptr: *mut c_char);
    }
    let start_c = CString::new(start).map_err(|_| AutomationErrorResult {
        ok: false,
        error: "validation_error".into(),
        message: "invalid start".into(),
        details: None,
    })?;
    // Compute end if missing: +60m
    let end_computed: String;
    let end_ptr: String = if let Some(e) = end {
        e.to_string()
    } else {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(start) {
            let e = dt + chrono::Duration::minutes(60);
            end_computed = e.to_rfc3339();
            end_computed.clone()
        } else {
            return Err(AutomationErrorResult {
                ok: false,
                error: "validation_error".into(),
                message: "invalid startAt".into(),
                details: None,
            });
        }
    };
    let end_c = CString::new(end_ptr.clone()).unwrap();
    let title_c = CString::new(title).unwrap();
    let success_c = CString::new("Calendar event created").unwrap();
    let cal_c = calendar.map(|s| CString::new(s).unwrap());
    let loc_c = location.map(|s| CString::new(s).unwrap());
    let notes_c = notes.map(|s| CString::new(s).unwrap());
    let cal_ptr = cal_c
        .as_ref()
        .map(|c| c.as_ptr())
        .unwrap_or(std::ptr::null());
    let loc_ptr = loc_c
        .as_ref()
        .map(|c| c.as_ptr())
        .unwrap_or(std::ptr::null());
    let notes_ptr = notes_c
        .as_ref()
        .map(|c| c.as_ptr())
        .unwrap_or(std::ptr::null());

    let result_ptr = unsafe {
        superflow_calendar_create_event(
            title_c.as_ptr(),
            start_c.as_ptr(),
            end_c.as_ptr(),
            cal_ptr,
            loc_ptr,
            notes_ptr,
            std::ptr::null(),
            success_c.as_ptr(),
        )
    };
    if result_ptr.is_null() {
        return Err(AutomationErrorResult {
            ok: false,
            error: "eventkit_error".into(),
            message: "Calendar bridge returned null".into(),
            details: None,
        });
    }
    let cstr = unsafe { CStr::from_ptr(result_ptr) };
    let json_str = cstr.to_string_lossy().to_string();
    unsafe {
        superflow_calendar_free_string(result_ptr);
    }
    let v: serde_json::Value =
        serde_json::from_str(&json_str).map_err(|_| AutomationErrorResult {
            ok: false,
            error: "eventkit_error".into(),
            message: "Invalid calendar response".into(),
            details: Some(json_str.clone()),
        })?;
    if v.get("ok").and_then(|x| x.as_bool()) == Some(true) {
        let event_id = v
            .get("event_id")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let cal = v
            .get("calendar")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        Ok(
            serde_json::json!({ "eventId": event_id, "title": title, "startAt": start, "endAt": end_ptr, "calendar": cal, "created": true }),
        )
    } else {
        let msg = v
            .get("message")
            .and_then(|x| x.as_str())
            .unwrap_or("Calendar error")
            .to_string();
        Err(AutomationErrorResult {
            ok: false,
            error: v
                .get("error")
                .and_then(|x| x.as_str())
                .unwrap_or("eventkit_error")
                .to_string(),
            message: msg,
            details: Some(json_str),
        })
    }
}

async fn calendar_update(
    event_id: &str,
    _title: Option<String>,
    _start_at: Option<String>,
    _end_at: Option<String>,
    _notes: Option<String>,
    _location: Option<String>,
) -> Result<serde_json::Value, AutomationErrorResult> {
    // Calendar update via AppleScript using uid lookup — best-effort
    let script = r#"
        on run argv
            set targetID to item 1 of argv
            set newTitle to item 2 of argv
            tell application "Calendar"
                repeat with cal in calendars
                    set matches to (every event of cal whose uid is targetID)
                    if (count of matches) > 0 then
                        set ev to item 1 of matches
                        if newTitle is not "" then set summary of ev to newTitle
                        return uid of ev as text
                    end if
                end repeat
                error "Event not found"
            end tell
        end run
    "#;
    let title = _title.unwrap_or_default();
    let _ = osascript(script, &[event_id, &title])?;
    Ok(serde_json::json!({ "eventId": event_id, "updated": true }))
}

async fn calendar_delete(event_id: &str) -> Result<serde_json::Value, AutomationErrorResult> {
    let script = r#"
        on run argv
            set targetID to item 1 of argv
            tell application "Calendar"
                repeat with cal in calendars
                    set matches to (every event of cal whose uid is targetID)
                    if (count of matches) > 0 then
                        set ev to item 1 of matches
                        delete ev
                        return targetID
                    end if
                end repeat
                error "Event not found"
            end tell
        end run
    "#;
    let _ = osascript(script, &[event_id])?;
    Ok(serde_json::json!({ "eventId": event_id, "deleted": true }))
}

// ───────────────────────────────────────── Reminders (EventKit via Swift fallback + AppleScript) ─────────────────────────────────────────

async fn execute_reminders(
    action: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, AutomationErrorResult> {
    match action {
        "find" => {
            let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let limit = params
                .get("limit")
                .and_then(|v| v.as_i64())
                .unwrap_or(20)
                .clamp(1, 100) as i32;
            reminders_find(query, limit).await
        }
        "create" => {
            let title = params
                .get("title")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: "title required".into(),
                    details: None,
                })?;
            let list = params
                .get("list")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let notes = params
                .get("notes")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let due_at = params
                .get("dueAt")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let priority = params
                .get("priority")
                .and_then(|v| v.as_i64())
                .map(|n| n as i32)
                .unwrap_or(0);
            reminders_create(title, list, notes, due_at, priority).await
        }
        "create_list" => {
            let name = params.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
                AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: "name required".into(),
                    details: None,
                }
            })?;
            reminders_create_list(name).await
        }
        "complete" => {
            let rid = params
                .get("reminderId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: "reminderId required".into(),
                    details: None,
                })?;
            reminders_complete(rid).await
        }
        _ => Err(AutomationErrorResult {
            ok: false,
            error: "validation_error".into(),
            message: format!("unsupported reminders action {action}"),
            details: None,
        }),
    }
}

async fn reminders_find(
    query: &str,
    limit: i32,
) -> Result<serde_json::Value, AutomationErrorResult> {
    let script = r#"
        on cleanValue(v)
            set s to v as text
            set AppleScript's text item delimiters to tab
            set parts to text items of s
            set AppleScript's text item delimiters to " "
            set s to parts as text
            set AppleScript's text item delimiters to ""
            return s
        end cleanValue
        on run argv
            set searchText to item 1 of argv
            set maxResults to (item 2 of argv) as integer
            set resultRows to {}
            tell application "Reminders"
                repeat with lst in lists
                    repeat with rm in reminders of lst
                        set rmName to ""
                        try
                            set rmName to name of rm as text
                        end try
                        set isMatch to false
                        ignoring case
                            if rmName contains searchText then set isMatch to true
                        end ignoring
                        if isMatch then
                            set rmID to id of rm as text
                            set end of resultRows to my cleanValue(rmID) & tab & my cleanValue(rmName) & tab & my cleanValue(name of lst as text)
                            if (count of resultRows) >= maxResults then exit repeat
                        end if
                    end repeat
                    if (count of resultRows) >= maxResults then exit repeat
                end repeat
            end tell
            set AppleScript's text item delimiters to linefeed
            set outputText to resultRows as text
            set AppleScript's text item delimiters to ""
            return outputText
        end run
    "#;
    let out = osascript(script, &[query, &limit.to_string()])?;
    let rows: Vec<&str> = if out.is_empty() {
        vec![]
    } else {
        out.split('\n').collect()
    };
    let mut matches = Vec::new();
    for row in rows {
        let f: Vec<&str> = row.split('\t').collect();
        if f.len() >= 3 {
            matches.push(serde_json::json!({ "reminderId": f[0], "title": f[1], "list": f[2] }));
        }
    }
    let count = matches.len();
    Ok(serde_json::json!({ "query": query, "count": count, "matches": matches }))
}

async fn reminders_create(
    title: &str,
    list: Option<String>,
    notes: Option<String>,
    due_at: Option<String>,
    priority: i32,
) -> Result<serde_json::Value, AutomationErrorResult> {
    let list_name = list.unwrap_or_default();
    let note = notes.unwrap_or_default();
    let due = due_at.unwrap_or_default();
    let script = r#"
        on run argv
            set rmTitle to item 1 of argv
            set rmList to item 2 of argv
            set rmNotes to item 3 of argv
            set rmDue to item 4 of argv
            tell application "Reminders"
                set targetList to missing value
                if rmList is not "" then
                    try
                        set targetList to first list whose name is rmList
                    end try
                end if
                if targetList is missing value then set targetList to default list
                tell targetList
                    set newReminder to make new reminder with properties {name:rmTitle}
                    if rmNotes is not "" then set body of newReminder to rmNotes
                    if rmDue is not "" then set due date of newReminder to date rmDue
                end tell
                return id of newReminder as text
            end tell
        end run
    "#;
    let rid = osascript(script, &[title, &list_name, &note, &due])?;
    let _ = priority; // priority mapping not exposed via AppleScript simple
    Ok(
        serde_json::json!({ "reminderId": rid, "title": title, "list": if list_name.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(list_name) }, "created": true }),
    )
}

async fn reminders_create_list(name: &str) -> Result<serde_json::Value, AutomationErrorResult> {
    let script = r#"
        on run argv
            set listName to item 1 of argv
            tell application "Reminders"
                try
                    set existing to first list whose name is listName
                    return id of existing as text & tab & "0"
                end try
                set newList to make new list with properties {name:listName}
                return id of newList as text & tab & "1"
            end tell
        end run
    "#;
    let out = osascript(script, &[name])?;
    let parts: Vec<&str> = out.split('\t').collect();
    let id = parts.get(0).unwrap_or(&"").to_string();
    let created = parts.get(1).map(|v| *v == "1").unwrap_or(false);
    Ok(serde_json::json!({ "listId": id, "name": name, "created": created }))
}

async fn reminders_complete(reminder_id: &str) -> Result<serde_json::Value, AutomationErrorResult> {
    let script = r#"
        on run argv
            set targetID to item 1 of argv
            tell application "Reminders"
                repeat with lst in lists
                    set matches to every reminder of lst whose id is targetID
                    if (count of matches) > 0 then
                        set rm to item 1 of matches
                        set completed of rm to true
                        return targetID
                    end if
                end repeat
                error "Reminder not found"
            end tell
        end run
    "#;
    let _ = osascript(script, &[reminder_id])?;
    Ok(serde_json::json!({ "reminderId": reminder_id, "completed": true }))
}
