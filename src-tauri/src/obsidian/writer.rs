use crate::obsidian::{ObsidianErrorResult, ValidatedObsidianNote};
use std::path::{Path, PathBuf};

fn escape_yaml(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn clean_opt(value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn render_markdown(note: &ValidatedObsidianNote) -> Result<String, ObsidianErrorResult> {
    let mut output: Vec<String> = Vec::new();
    output.push("---".to_string());
    output.push(format!("type: {}", note.kind));
    output.push(format!("title: \"{}\"", escape_yaml(&note.title)));
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    output.push(format!("updated: {}", now));
    output.push("---".to_string());
    output.push(String::new());
    output.push(format!("# {}", note.title));
    output.push(String::new());

    if let Some(summary) = clean_opt(&note.summary) {
        output.push(summary);
        output.push(String::new());
    }

    match note.kind.as_str() {
        "document" => render_document(note, &mut output),
        "meeting" => render_meeting(note, &mut output),
        "todo" => render_todo(note, &mut output),
        "research" => render_research(note, &mut output),
        _ => {}
    }

    Ok(output
        .join("\n")
        .trim_matches(|c: char| c.is_whitespace())
        .to_string()
        + "\n")
}

fn render_document(note: &ValidatedObsidianNote, output: &mut Vec<String>) {
    render_sections(note.sections.as_ref(), output);
}

fn render_meeting(note: &ValidatedObsidianNote, output: &mut Vec<String>) {
    if let Some(date) = clean_opt(&note.date) {
        output.push(format!("**Date:** {}", date));
        output.push(String::new());
    }
    if let Some(attendees) = &note.attendees {
        if !attendees.is_empty() {
            output.push("## Attendees".to_string());
            output.push(String::new());
            for a in attendees {
                output.push(format!("- {}", a));
            }
            output.push(String::new());
        }
    }
    if let Some(agenda) = &note.agenda {
        if !agenda.is_empty() {
            output.push("## Agenda".to_string());
            output.push(String::new());
            for a in agenda {
                output.push(format!("- {}", a));
            }
            output.push(String::new());
        }
    }
    render_sections(note.sections.as_ref(), output);
    if let Some(decisions) = &note.decisions {
        if !decisions.is_empty() {
            output.push("## Decisions".to_string());
            output.push(String::new());
            for d in decisions {
                output.push(format!("- {}", d));
            }
            output.push(String::new());
        }
    }
    if let Some(actions) = &note.action_items {
        if !actions.is_empty() {
            output.push("## Action Items".to_string());
            output.push(String::new());
            render_tasks(actions, output);
        }
    } else if let Some(tasks) = &note.tasks {
        if !tasks.is_empty() {
            output.push("## Action Items".to_string());
            output.push(String::new());
            render_tasks(tasks, output);
        }
    }
}

fn render_todo(note: &ValidatedObsidianNote, output: &mut Vec<String>) {
    if let Some(tasks) = &note.tasks {
        render_tasks(tasks, output);
    } else if let Some(actions) = &note.action_items {
        render_tasks(actions, output);
    }
}

fn render_research(note: &ValidatedObsidianNote, output: &mut Vec<String>) {
    if let Some(abs) = clean_opt(&note.abstract_text) {
        output.push("## Abstract".to_string());
        output.push(String::new());
        output.push(abs);
        output.push(String::new());
    }
    render_sections(note.sections.as_ref(), output);
    if let Some(sources) = &note.sources {
        if !sources.is_empty() {
            output.push("## Sources".to_string());
            output.push(String::new());
            for s in sources {
                let mut line = format!("- {}", s.title);
                if let Some(author) = clean_opt(&s.author) {
                    line += &format!(" — {}", author);
                }
                if let Some(url) = clean_opt(&s.url) {
                    line += &format!(" — {}", url);
                }
                output.push(line);
                if let Some(note) = clean_opt(&s.note) {
                    output.push(format!("  - {}", note));
                }
            }
            output.push(String::new());
        }
    }
}

fn render_sections(
    sections: Option<&Vec<crate::obsidian::ObsidianSection>>,
    output: &mut Vec<String>,
) {
    let Some(sections) = sections else { return };
    for sec in sections {
        output.push(format!("## {}", sec.heading));
        output.push(String::new());
        output.push(sec.body.clone());
        output.push(String::new());
    }
}

fn render_tasks(tasks: &[crate::obsidian::ObsidianTask], output: &mut Vec<String>) {
    for task in tasks {
        let completed = task.completed.unwrap_or(false);
        let mut line = format!("- [{}] {}", if completed { "x" } else { " " }, task.title);
        let mut meta: Vec<String> = Vec::new();
        if let Some(owner) = clean_opt(&task.owner) {
            meta.push(format!("Owner: {}", owner));
        }
        if let Some(due) = clean_opt(&task.due) {
            meta.push(format!("Due: {}", due));
        }
        if let Some(priority) = clean_opt(&task.priority) {
            meta.push(format!("Priority: {}", priority));
        }
        if !meta.is_empty() {
            line += " — ";
            line += &meta.join(" · ");
        }
        output.push(line);
    }
    output.push(String::new());
}

fn resolve_vault(vault_path: Option<&String>) -> Result<PathBuf, ObsidianErrorResult> {
    if let Some(path) = vault_path {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            let expanded = shellexpand_tilde(trimmed);
            return Ok(PathBuf::from(expanded));
        }
    }
    if let Ok(path) = std::env::var("OBSIDIAN_VAULT_PATH") {
        if !path.trim().is_empty() {
            let expanded = shellexpand_tilde(&path);
            return Ok(PathBuf::from(expanded));
        }
    }
    if let Some(path) = detect_vault_from_obsidian_config() {
        return Ok(PathBuf::from(path));
    }
    Err(ObsidianErrorResult {
        ok: false,
        error: "missing_vault".to_string(),
        message: "No Obsidian vault was found. Open your vault in Obsidian once, or set OBSIDIAN_VAULT_PATH.".to_string(),
        details: None,
    })
}

/// Reads the user's registered vaults from Obsidian's own config
/// (`~/Library/Application Support/obsidian/obsidian.json`) and picks the
/// currently-open one, falling back to the most recently opened. This is how
/// the app finds the vault without any user setup; the config is written and
/// kept current by Obsidian itself.
fn detect_vault_from_obsidian_config() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let config_path =
        std::path::Path::new(&home).join("Library/Application Support/obsidian/obsidian.json");
    let raw = std::fs::read_to_string(config_path).ok()?;
    pick_vault_from_config_json(&raw)
}

fn pick_vault_from_config_json(raw: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(raw).ok()?;
    let vaults = parsed.get("vaults")?.as_object()?;
    vaults
        .values()
        .filter_map(|v| {
            let path = v.get("path")?.as_str()?.to_string();
            let open = v.get("open").and_then(|o| o.as_bool()).unwrap_or(false);
            let ts = v.get("ts").and_then(|t| t.as_u64()).unwrap_or(0);
            Some((open, ts, path))
        })
        .max_by_key(|(open, ts, _)| (*open, *ts))
        .map(|(_, _, path)| path)
}

fn shellexpand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = dirs_next_home() {
            return path.replacen('~', &home, 1);
        }
    } else if path == "~" {
        if let Some(home) = dirs_next_home() {
            return home;
        }
    }
    path.to_string()
}

fn dirs_next_home() -> Option<String> {
    std::env::var("HOME").ok()
}

fn sanitize_component(value: &str) -> Result<String, ObsidianErrorResult> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed == "."
        || trimmed == ".."
        || trimmed.contains('/')
        || trimmed.contains('\\')
    {
        return Err(ObsidianErrorResult {
            ok: false,
            error: "validation_error".to_string(),
            message: "Invalid vault folder name".to_string(),
            details: Some(value.to_string()),
        });
    }
    Ok(trimmed.to_string())
}

fn slug(value: &str) -> Result<String, ObsidianErrorResult> {
    let lowered = value.to_lowercase().trim().to_string();
    // Simplify: replace spaces with -, keep alphanumeric and - _
    let mut pieces: String = lowered
        .replace(' ', "-")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    while pieces.contains("--") {
        pieces = pieces.replace("--", "-");
    }
    let result = pieces.trim_matches('-').to_string();
    if result.is_empty() {
        return Err(ObsidianErrorResult {
            ok: false,
            error: "validation_error".to_string(),
            message: "Invalid file name".to_string(),
            details: Some(value.to_string()),
        });
    }
    Ok(result)
}

fn atomic_write(content: &str, to: &Path) -> Result<(), ObsidianErrorResult> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ObsidianErrorResult {
            ok: false,
            error: "write_failed".to_string(),
            message: format!("Failed to create directory: {}", e),
            details: None,
        })?;
    }
    std::fs::write(to, content).map_err(|e| ObsidianErrorResult {
        ok: false,
        error: "write_failed".to_string(),
        message: e.to_string(),
        details: None,
    })
}

fn append_write(content: &str, to: &Path) -> Result<(), ObsidianErrorResult> {
    if !to.exists() {
        return atomic_write(content, to);
    }
    let existing = std::fs::read_to_string(to).map_err(|e| ObsidianErrorResult {
        ok: false,
        error: "write_failed".to_string(),
        message: e.to_string(),
        details: None,
    })?;
    let new_content = format!("{}\n\n{}", existing.trim_end(), content);
    atomic_write(&new_content, to)
}

pub fn execute_write(note: &ValidatedObsidianNote) -> Result<PathBuf, ObsidianErrorResult> {
    let vault = resolve_vault(note.vault_path.as_ref())?;
    let branch = sanitize_component(&note.branch)?;
    let workspace = vault.join(branch);
    std::fs::create_dir_all(&workspace).map_err(|e| ObsidianErrorResult {
        ok: false,
        error: "write_failed".to_string(),
        message: e.to_string(),
        details: None,
    })?;
    let raw_file_key = note.file_key.clone().unwrap_or_else(|| note.title.clone());
    let file_key = slug(&raw_file_key)?;
    let file_url = workspace.join(format!("{}.md", file_key));
    let markdown = render_markdown(note)?;
    match note.mode.as_deref().unwrap_or("upsert") {
        "append" => append_write(&markdown, &file_url)?,
        _ => atomic_write(&markdown, &file_url)?,
    }
    // open after write is not needed for Rust path; could trigger obsidian:// open via open crate but keep silent
    Ok(file_url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::obsidian::ValidatedObsidianNote;

    fn todo_note() -> ValidatedObsidianNote {
        ValidatedObsidianNote {
            vault_path: None,
            kind: "todo".to_string(),
            title: "Maya Chen — Follow-up".to_string(),
            branch: "Tasks".to_string(),
            file_key: None,
            mode: None,
            summary: None,
            date: None,
            attendees: None,
            agenda: None,
            sections: None,
            decisions: None,
            tasks: Some(vec![crate::obsidian::ObsidianTask {
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
        }
    }

    #[test]
    fn renders_todo_markdown() {
        let note = todo_note();
        let md = render_markdown(&note).unwrap();
        assert!(md.contains("# Maya Chen"));
        assert!(md.contains("- [ ] Follow up with Maya Chen"));
        assert!(md.contains("Due: 2026-09-02"));
    }

    #[test]
    fn slug_sanitizes() {
        assert_eq!(slug("Hello World").unwrap(), "hello-world");
        assert_eq!(
            slug("Q4 Security Architecture Review!").unwrap(),
            "q4-security-architecture-review"
        );
    }

    #[test]
    fn branch_validation() {
        assert!(sanitize_component("Tasks").is_ok());
        assert!(sanitize_component("../evil").is_err());
        assert!(sanitize_component("a/b").is_err());
    }

    #[test]
    fn picks_open_vault_over_newer_closed_one() {
        let raw = r#"{"vaults":{"a":{"path":"/v/old","ts":999,"open":true},"b":{"path":"/v/new","ts":5000,"open":false}}}"#;
        assert_eq!(pick_vault_from_config_json(raw).as_deref(), Some("/v/old"));
    }

    #[test]
    fn falls_back_to_most_recent_when_none_open() {
        let raw = r#"{"vaults":{"a":{"path":"/v/old","ts":10,"open":false},"b":{"path":"/v/new","ts":20,"open":false}}}"#;
        assert_eq!(pick_vault_from_config_json(raw).as_deref(), Some("/v/new"));
    }

    #[test]
    fn returns_none_for_garbage_config() {
        assert!(pick_vault_from_config_json("not json").is_none());
        assert!(pick_vault_from_config_json(r#"{"vaults":{}}"#).is_none());
    }
}
