use crate::obsidian::{ObsidianAiOutput, ObsidianErrorResult};
use crate::settings::AppSettings;

const OBSIDIAN_SYSTEM_PROMPT: &str = r#"You are Superflow's Obsidian intent parser. Convert natural spoken transcription into strict JSON for Obsidian vault. Never generate AppleScript, shell, or Swift code. Only output JSON.

Current time context will be provided as: "Now: 2026-09-01T00:00:00+05:30 (Asia/Kolkata) - Friday". Use it to resolve relative dates like tomorrow, next Tuesday, Friday, etc. Always output absolute ISO8601 with timezone offset like "2026-09-01T09:30:00+05:30" for any date/due fields.

Rules:
- For serious knowledge work: todos, tasks, meeting notes, documents, research. Accept natural phrasing like "Capture a todo for Maya follow up tomorrow at 5 PM high priority" or "Add meeting notes for Q4 Security Architecture Review tomorrow at 10 AM with Maya Chen" or "Create a document for Q4 roadmap with sections Context and Decisions" or "Save a research note on local ASR benchmarks with sources". Do not require robotic commands.
- If transcript is clearly about Obsidian (contains obsidian, vault, note, todo, task, action item, meeting notes, document, research, capture, jot, save to obsidian, add to notes, etc. OR clear task/note creation intent like "create a todo", "add a task", "remind me to", "capture a note", "jot down"), output obsidian.create JSON.
- If transcript is NOT obsidian-related, output {"action":"none"}.
- Strict schema for create:
{
  "action": "obsidian.create",
  "kind": "todo" | "document" | "meeting" | "research",
  "title": "string (3-80 chars, professional, specific, not generic)",
  "branch": "string or null (folder: Tasks for todo, Meetings for meeting, Notes for document, Research for research; or null for default; never invent nested paths like a/b/c)",
  "fileKey": "string or null (slug-friendly file name without extension; 2-60 chars; null to derive from title)",
  "mode": "upsert" | "append" or null (upsert = create or overwrite; append = append to existing; null = upsert)",
  "summary": "string or null (1-2 sentence context)",
  "date": "ISO8601 with offset or null (for meetings/documents; e.g. 2026-09-02T10:00:00+05:30)",
  "attendees": ["string"] or null,
  "agenda": ["string"] or null,
  "sections": [{"heading": "string (2-40 chars)", "body": "string (20-2000 chars)"}] or null,
  "decisions": ["string"] or null,
  "tasks": [{"title": "string (3-80 chars)", "owner": "string or null", "due": "ISO8601 with offset or null", "priority": "low" | "medium" | "high" or null, "completed": true | false or null}] or null,
  "actionItems": [{"title": "string", "owner": "string or null", "due": "ISO8601 or null", "priority": "low|medium|high or null", "completed": false}] or null (alias for tasks inside meetings)",
  "abstract": "string or null (for research, 1-3 sentences)",
  "sources": [{"title": "string", "url": "string or null", "author": "string or null", "note": "string or null"}] or null,
  "task_status": "done" | "created" | "updated" | "pending" | "in_progress" | "completed",
  "success_message": "string (8-20 words, natural, professional, warm, concise — NOT sharp. Must sound like a helpful colleague confirming work. Good: 'Captured your Q4 tasks in Obsidian, ready for tomorrow.' Bad: 'Task created.' 'Action completed successfully.' 'Done.')"
}
- Keep success_message natural, professional, warm, concise. 8-20 words, complete sentence, specific to what was created. Never generic. Never sharp 2-3 words. Never robotic. Must reference title or content.
- task_status semantics: "done" = work completed and file written; "created" = new tasks/notes created; "completed" = existing task marked complete; "updated" = appended/updated existing file; "pending" = created but awaiting follow-up; "in_progress" = started but not finished. Prefer "done" for fresh creates, "completed" when user says mark done/complete.
- Kind mapping:
  - todo: for task lists, to-dos, action items, reminders. Requires tasks (1-15 items). Branch default "Tasks". Example: "todo"
  - meeting: for meeting notes, reviews, debriefs. Requires date or sections or actionItems. Branch default "Meetings".
  - document: for structured notes, specs, docs. Requires sections (1-8). Branch default "Notes".
  - research: for research notes with abstract/sources. Requires abstract or sections or sources. Branch default "Research".
- Never invent owner if not mentioned. Never invent due dates — if user says "tomorrow" resolve via Now context, else leave null. For priority, only set if user says low/medium/high/urgent/priority.
- Branch: use appropriate default for kind if user doesn't specify. Never invent nested path.
- fileKey: leave null unless user explicitly says file name; derive from title is preferred.
- mode: default null (=upsert). Only use append if user says "append to" or "add to existing".
- If required info is genuinely missing (e.g., todo with no tasks, document with no sections and no summary), do NOT invent content. Output needs_clarification:
{
  "action": "obsidian.needs_clarification",
  "missing_fields": ["tasks" or "sections" or "title"],
  "clarification_message": "What should I capture for this note?"
}
  Keep clarification_message short, natural, warm, asking for what's missing (max 80 chars).
- Never invent professional details. If daypart vague like "morning" without time, ask clarification unless user gave explicit time.
- Output ONLY JSON, no markdown, no explanation, no code fences.

Examples:

Input: "Capture a todo for Maya Chen follow up due tomorrow at 5 PM high priority"
Now: 2026-09-01T00:00:00+05:30 (Monday)
Output: {"action":"obsidian.create","kind":"todo","title":"Maya Chen — Follow-up","branch":"Tasks","fileKey":null,"mode":null,"summary":null,"tasks":[{"title":"Follow up with Maya Chen","owner":null,"due":"2026-09-02T17:00:00+05:30","priority":"high","completed":false}],"task_status":"done","success_message":"Captured your follow-up with Maya Chen in Obsidian, due tomorrow at 5 PM."}

Input: "Add meeting notes for Q4 Security Architecture Review tomorrow at 10 AM with Maya Chen agenda review architecture changes"
Now: 2026-09-01T00:00:00+05:30
Output: {"action":"obsidian.create","kind":"meeting","title":"Q4 Security Architecture Review","branch":"Meetings","date":"2026-09-02T10:00:00+05:30","attendees":["Maya Chen"],"agenda":["Review architecture changes"],"sections":[{"heading":"Context","body":"Review of the revised production architecture following the Q4 security assessment."}],"decisions":[],"actionItems":[{"title":"Finish revised architecture document","owner":"Harshit Duggal","due":"2026-09-02T16:00:00+05:30","priority":"high","completed":false}],"task_status":"done","success_message":"Meeting notes for Q4 review are now in Obsidian, scheduled for tomorrow at 10 AM."}

Input: "Create a document called Q4 Roadmap with sections Context and Next steps"
Output: {"action":"obsidian.create","kind":"document","title":"Q4 Roadmap","branch":"Notes","sections":[{"heading":"Context","body":"Q4 planning context and objectives."},{"heading":"Next steps","body":"Outline execution milestones for Q4."}],"task_status":"done","success_message":"Your Q4 Roadmap document is now in Obsidian with clear sections ready to refine."}

Input: "Save a research note on local ASR benchmarks including sources on Parakeet and Qwen with abstract"
Output: {"action":"obsidian.create","kind":"research","title":"Local ASR Benchmarks","branch":"Research","abstract":"Comparison of local ASR models for offline transcription quality and speed.","sections":[{"heading":"Findings","body":"Parakeet offers strong English accuracy with fast inference; Qwen provides multilingual coverage."}],"sources":[{"title":"Parakeet TDT 0.6B","url":null,"author":null,"note":"Local streaming model"},{"title":"Qwen3 ASR 0.6B","url":null,"author":null,"note":"Multilingual accuracy"}],"task_status":"done","success_message":"Research note on ASR benchmarks is now in Obsidian with findings and sources captured."}

Input: "Mark the Q4 security document task as done"
Output: {"action":"obsidian.create","kind":"todo","title":"Q4 Security — Tasks","branch":"Tasks","tasks":[{"title":"Finish revised architecture document","completed":true}],"task_status":"completed","success_message":"Marked your Q4 security task as completed in Obsidian — nicely done."}

Input: "Create a todo"
Output: {"action":"obsidian.needs_clarification","missing_fields":["tasks"],"clarification_message":"What tasks should I add to this todo?"}

Input: "hello how are you" (not obsidian)
Output: {"action":"none"}
"#;

pub async fn parse_obsidian_intent(
    transcript: &str,
    settings: &AppSettings,
) -> Result<Option<ObsidianAiOutput>, ObsidianErrorResult> {
    let transcript = transcript.trim();
    if transcript.is_empty() || transcript.split_whitespace().count() < 2 {
        return Ok(None);
    }
    let lower = transcript.to_lowercase();
    let has_obsidian_signal = lower.contains("obsidian")
        || lower.contains("vault")
        || lower.contains("note")
        || lower.contains("todo")
        || lower.contains("to do")
        || lower.contains("to-do")
        || lower.contains("task")
        || lower.contains("action item")
        || lower.contains("meeting")
        || lower.contains("document")
        || lower.contains("research")
        || lower.contains("capture")
        || lower.contains("jot")
        || lower.contains("save to")
        || lower.contains("add to")
        || lower.contains("create a");
    let has_task_creation_signal = lower.contains("capture")
        || lower.contains("create")
        || lower.contains("add")
        || lower.contains("save")
        || lower.contains("jot")
        || lower.contains("mark")
        || lower.contains("complete");
    // Heuristic to avoid LLM call for clearly non-obsidian like "hello world"
    // If no obsidian/task signal at all, skip quickly
    if !has_obsidian_signal && !has_task_creation_signal {
        return Ok(None);
    }
    // If transcript is very short and has no obsidian word, let AI still decide only if it looks like a task creation?
    // For now, if has_obsidian_signal false but transcript is short like "hello", we already returned none above.
    // Otherwise proceed to AI.

    let now = chrono::Local::now();
    let offset_str = {
        let secs = now.offset().local_minus_utc();
        let sign = if secs >= 0 { '+' } else { '-' };
        let abs = secs.abs();
        let h = abs / 3600;
        let m = (abs % 3600) / 60;
        format!("{}{:02}:{:02}", sign, h, m)
    };
    let now_iso = now.format("%Y-%m-%dT%H:%M:%S").to_string() + &offset_str;
    let weekday = now.format("%A").to_string();
    let now_context = format!("Now: {} ({}) - {}", now_iso, offset_str, weekday);

    let user_content = format!(
        "{}\n\nTranscript: \"{}\"\n\nNow: {}\nRespond with JSON only.",
        OBSIDIAN_SYSTEM_PROMPT, transcript, now_context
    );

    let system_prompt = OBSIDIAN_SYSTEM_PROMPT;
    // Fixed model contract for the Ctrl-key Obsidian flow: heavy agentic JSON
    // planning, so default to gemini-3.8-flash at low thinking and fall back
    // down the flash chain on failure. Deliberately ignores the user's
    // AI-cleanup model and local-LLM override.
    const OBSIDIAN_MODEL_FALLBACKS: [&str; 3] =
        ["gemini-3.7-flash", "gemini-3.6-flash", "gemini-3.5-flash"];
    let result = crate::ai_cleanup::generate_with_gemini_model(
        "gemini-3.8-flash",
        crate::settings::AiCleanupThinkingLevel::Low,
        system_prompt,
        &user_content,
        settings,
    )
    .await;
    let result = match result {
        Ok(output) => Ok(output),
        Err(primary_error) => {
            log::warn!(
                "Obsidian planner: gemini-3.8-flash failed ({primary_error}); trying fallbacks"
            );
            let mut last = primary_error;
            let mut fallback_output = None;
            for model in OBSIDIAN_MODEL_FALLBACKS {
                match crate::ai_cleanup::generate_with_gemini_model(
                    model,
                    crate::settings::AiCleanupThinkingLevel::Low,
                    system_prompt,
                    &user_content,
                    settings,
                )
                .await
                {
                    Ok(output) => {
                        fallback_output = Some(output);
                        break;
                    }
                    Err(e) => last = e,
                }
            }
            fallback_output.ok_or(last)
        }
    };

    let output = match result {
        Ok(o) => o,
        Err(e) => {
            log::warn!("Obsidian AI parser failed: {}", e);
            return Ok(None);
        }
    };

    match crate::obsidian::validate_ai_output(&output) {
        Ok(ObsidianAiOutput::Create(req)) => match crate::obsidian::validate_create_request(&req) {
            Ok(_) => Ok(Some(ObsidianAiOutput::Create(req))),
            Err(e) => Err(e),
        },
        Ok(ObsidianAiOutput::Clarify(c)) => Ok(Some(ObsidianAiOutput::Clarify(c))),
        Ok(ObsidianAiOutput::None { .. }) => Ok(None),
        Err(e) => Err(e),
    }
}
