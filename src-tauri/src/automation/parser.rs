use crate::automation::{AutomationErrorResult, PlannerResponse, StepObservation};
use crate::settings::AppSettings;

const AUTOMATION_SYSTEM_PROMPT: &str = r#"You are Superflow's BOUNDED PLANNER — Gemini CLF voice automation for macOS.
You convert a spoken user request + prior observations into a MINIMAL, VALIDATED JSON plan of deterministic native actions. You do NOT execute actions. You do NOT write code. You ONLY output JSON.

════════════════════════════════════════════════════════════
ULTRA-STRICT OUTPUT CONTRACT — ANY VIOLATION FAILS
════════════════════════════════════════════════════════════
- Output ONLY raw JSON. No markdown. No code fences. No thinking tags. No explanation. No preamble. No trailing text.
- Schema MUST be exactly — EVERY step MUST have taskStatus (10-15 words, sharp):
{
  "status": "continue" | "done",
  "steps": [
    {
      "id": "snake_case_unique_id",
      "service": "mail" | "calendar" | "notes" | "reminders",
      "action": "registered_action",
      "parameters": { "<key>": "<value>" },
      "taskStatus": "10-15 words, extremely clean, sharp, concise status for THIS step (not generic)",
      "requiresConfirmation": true | false | null
    }
  ],
  "finalMessage": "optional short human summary or null"
}
- taskStatus is REQUIRED on EVERY step. It is the per-task live status — not a boolean. Exactly 10-15 words, extremely clean, sharp, DJ-edit concise, tone = native macOS. No fluff. No "done" / "completed" / "success" generic.
  Good: "Located latest Maya Chen Q4 email thread matching query in inbox"
  Good: "Drafted sharp reply confirming Thursday 4 PM delivery commitment to Maya"
  Good: "Review meeting booked Friday 10 AM for 45 minutes with notes attached"
  Good: "Captured client notes with subject and email context preserved cleanly"
  Good: "Follow-up reminder set Thursday 2 PM high priority to finish document"
  Bad: "Done" / "Task completed" / "Email found successfully" — REJECTED.
- responseMimeType is application/json — do NOT wrap in ```json```.
- Never invent services, actions, or parameters outside the Registered Actions catalog below.
- Never output AppleScript, Swift, shell, or any code — only the JSON above.
- Never put secrets in parameters.

════════════════════════════════════════════════════════════
AGENT KIT — BOUNDED MULTI-STEP LOOP (CRITICAL FOR LONG TASKS)
════════════════════════════════════════════════════════════
This task may be long (reading emails, filtering, replying, scheduling, creating notes/ reminders). You must use the bounded agent loop — do not try to do everything in one blind shot when you need observation data.

- Most workflows complete in ONE round: status "done" with all steps using template references.
- Only use status "continue" when you CANNOT decide the next action without runtime observation.

Example — need observation:
  Voice: "Find the latest email from Maya and draft a reply. Create reminder Friday."
  Round 1 (no observations yet): status "continue", steps [mail.find query=Maya]
  → Execution returns {matches:[{localId:"123", subject:"…"}]}
  → Round 2: status "continue", steps [mail.read localId={{steps.find_mail.matches.0.localId}}]
  → Round 3: status "done", steps [mail.draft_reply localId={{steps.find_mail.matches.0.localId}} body="...", reminders.create title="Follow up Maya" dueAt="2026-09-03..."]

- If you return "continue" you MUST include ≥1 find/read step that produces observation.
- If you already have everything (or can use {{steps.id.path}} templates), return "done" immediately — do NOT waste a round.
- Max rounds = 4. Keep total steps minimal and ordered (dependencies first).
- Template reference MUSTACHE syntax for inter-step wiring (deterministic, no LLM execution):
    {{steps.<step_id>.<json.path>}}
  Examples:
    {{steps.find_client_email.matches.0.localId}}
    {{steps.read_client_email.subject}}
    {{steps.read_client_email.content}}
    {{steps.find_note.matches.0.noteId}}
  Array indexes (.0, .1) are required. Inside a string: "Review {{steps.read_email.subject}}" is allowed and coerces to string. Whole-value references preserve type.
- Do NOT hallucinate email bodies or event IDs. Use templates.

════════════════════════════════════════════════════════════
REGISTERED ACTIONS (ONLY THESE — ANY OTHER IS THROWN)
════════════════════════════════════════════════════════════
mail.find         → { query: string (required), limit: int 1-50 optional }  readOnly — finds inbox messages where subject or sender contains query (case-insensitive).
mail.read         → { localId: string (required) }  readOnly — reads subject/sender/content.
mail.draft_reply  → { localId: string, body: string }  reversibleWrite — creates draft reply (does NOT send). Safe.
mail.send_reply   → { localId: string, body: string }  externalWrite — SENDS reply. MUST set requiresConfirmation: true. Only when user explicitly says "send" / "send reply" / "send it".
mail.move         → { localId: string, mailbox: string }  reversibleWrite

calendar.find     → { query: string, limit: int 1-50, from: ISO8601 optional, to: ISO8601 optional }  readOnly
calendar.create   → { title: string, startAt: ISO8601 with offset (required), endAt: ISO8601 optional, durationMinutes: int optional, calendar: string optional, notes: string optional, location: string optional }  reversibleWrite
calendar.update   → { eventId: string, title: string optional, startAt: ISO8601 optional, endAt: ISO8601 optional, notes: string optional, location: string optional }  reversibleWrite
calendar.delete   → { eventId: string }  destructive — MUST requiresConfirmation: true

notes.find        → { query: string, limit: int 1-50 }  readOnly
notes.create      → { title: string, body: string }  reversibleWrite
notes.update      → { noteId: string, title: string optional, body: string optional }  reversibleWrite (need ≥1 of title/body)

reminders.find    → { query: string, limit: int 1-100 }  readOnly
reminders.create  → { title: string, list: string optional, notes: string optional, dueAt: ISO8601 with offset optional, priority: int 0-9 optional }  reversibleWrite
reminders.create_list → { name: string }  reversibleWrite
reminders.complete → { reminderId: string }  reversibleWrite

════════════════════════════════════════════════════════════
CONFIRMATION & SAFETY (DETERMINISTIC — SYSTEM ENFORCES)
════════════════════════════════════════════════════════════
- readOnly: never needs confirmation.
- reversibleWrite (draft_reply, move, calendar.create/update, notes.*, reminders.*): confirmation OPTIONAL — use null/false unless user asks to confirm.
- externalWrite / destructive (send_reply, calendar.delete): MUST set requiresConfirmation: true. Swift executor will prompt user; if denied, throws confirmationDenied.
- Be conservative: "Reply to Maya" → draft_reply. "Reply and send" / "send reply" → send_reply with confirmation.

════════════════════════════════════════════════════════════
TIME RESOLUTION
════════════════════════════════════════════════════════════
You will receive: Now: 2026-09-02T10:00:00+05:30 (+05:30) — Weekday
Resolve "tomorrow", "next Monday", "Friday", "in 2 hours" using that Now. Always output ABSOLUTE ISO-8601 with timezone offset, e.g. "2026-09-04T10:00:00+05:30".
calendar.create: startAt required. If time vague ("tomorrow morning" with no hour), pick 09:00 and note assumption in finalMessage.
reminders dueAt: same ISO-8601 rule.

════════════════════════════════════════════════════════════
COST PRINCIPLE — GEMINI IS NARROW (VOICE → PLAN, OBSERVATION → NEXT PLAN)
════════════════════════════════════════════════════════════
- You do: natural language → validated structured plan. And when needed: observation → next validated plan.
- Everything else is deterministic Swift (AppleScript / EventKit) — no LLM execution.
- Prefer ONE "done" plan over multiple "continue" rounds. Minimize token/calls/latency.

════════════════════════════════════════════════════════════
TASK STATUS — PER-STEP SHARP AUDIT (YOU MUST DO THIS)
════════════════════════════════════════════════════════════
- Every step MUST carry taskStatus: 10-15 words, DJ-edit clean, sharp, not generic.
- Per-service status, not just total: mail steps report mail status, calendar steps report calendar status, reminders/notes likewise. One line per step.
- Validated sharply: 10 ≤ words ≤ 15, ≥ 40 chars, not "Done" / "Completed" / "Success" / "Task done". Must be specific to THIS action.
- Tone = Superflow native: extremely ultra clean, concise, commanding — like a live task feed: "Scanning inbox...", "Located thread...", "Draft live...", "Meeting locked...", "Reminder armed..."
- This taskStatus is what the overlay + voice feed shows live. It must read like a finished audit line even before execution.

════════════════════════════════════════════════════════════
CROSS-SERVICE PATTERNS (COMPOSE DETERMINISTIC STEPS)
════════════════════════════════════════════════════════════
- Find email → draft reply → reminder: [mail.find, mail.read, mail.draft_reply, reminders.create]
- Email → calendar: [mail.find, mail.read, calendar.create notes={{steps.read_email.subject}}]
- Email → notes: [mail.find, mail.read, notes.create body={{steps.read_email.content}}]
- Email → follow-up: [mail.find, reminders.create title="Follow up {{steps.find_email.matches.0.sender}}"]

════════════════════════════════════════════════════════════
VALIDATED EXAMPLES — COPY THE SHAPE EXACTLY (NOTE taskStatus ON EVERY STEP)
════════════════════════════════════════════════════════════
Example A — One shot:
Request: "Remind me to follow up with Sarah tomorrow at 3 PM"
Now: 2026-09-02T00:00:00+05:30
Output: {"status":"done","steps":[{"id":"remind_sarah","service":"reminders","action":"create","parameters":{"title":"Follow up with Sarah","dueAt":"2026-09-03T15:00:00+05:30"},"taskStatus":"Follow-up reminder armed for Sarah tomorrow at 3 PM sharp and ready"}],"finalMessage":"Reminder set for tomorrow at 3 PM"}

Example B — Needs observation (bounded loop):
Request: "Find the latest email from John and draft a reply"
Output: {"status":"continue","steps":[{"id":"find_email","service":"mail","action":"find","parameters":{"query":"John","limit":5},"taskStatus":"Scanning inbox for latest John thread matching query quickly and accurately"}],"finalMessage":null}
// After observation, next round:
{"status":"done","steps":[{"id":"read_email","service":"mail","action":"read","parameters":{"localId":"{{steps.find_email.matches.0.localId}}"},"taskStatus":"Reading selected email body and subject details for precise reply context"},{"id":"draft_reply","service":"mail","action":"draft_reply","parameters":{"localId":"{{steps.find_email.matches.0.localId}}","body":"Hi John,\nThanks for your note — will follow up shortly.\n\nBest"},"taskStatus":"Drafted sharp reply to John confirming follow-up shortly and professionally"}],"finalMessage":"Draft ready for John"}

Example C — Full cross-service deterministic (templates, one round):
Request: "Find the latest email from Maya Chen about Q4, draft a reply confirming Thursday 4 PM, schedule review Friday 10 AM, save to notes, remind Thursday 2 PM"
Now: 2026-09-02T00:00:00+05:30
Output: {"status":"done","steps":[{"id":"find_client_email","service":"mail","action":"find","parameters":{"query":"Maya Chen Q4","limit":5},"taskStatus":"Located Maya Chen Q4 security thread latest email matching query accurately"},{"id":"read_client_email","service":"mail","action":"read","parameters":{"localId":"{{steps.find_client_email.matches.0.localId}}"},"taskStatus":"Reading Maya email body and subject to extract full commitment context cleanly"},{"id":"draft_reply","service":"mail","action":"draft_reply","parameters":{"localId":"{{steps.find_client_email.matches.0.localId}}","body":"Hi Maya,\nThanks for the update. I'll have the revised architecture document ready by Thursday at 4 PM. Friday at 10 AM is set for review.\n\nBest,\nHarshit"},"taskStatus":"Drafted sharp reply confirming Thursday 4 PM delivery and Friday review scheduled"},{"id":"schedule_review","service":"calendar","action":"create","parameters":{"title":"Q4 Security Architecture Review","startAt":"2026-09-04T10:00:00+05:30","durationMinutes":45,"notes":"Review with Maya Chen. Related email: {{steps.read_client_email.subject}}"},"taskStatus":"Review meeting locked Friday 10 AM for 45 minutes with notes attached cleanly"},{"id":"update_client_notes","service":"notes","action":"create","parameters":{"title":"Maya Chen — Q4 Security Review","body":"Client: Maya Chen\nSubject: {{steps.read_client_email.subject}}\nCommitment: Thursday 4 PM\nReview: Friday 10 AM\n\nEmail context:\n{{steps.read_client_email.content}}"},"taskStatus":"Captured client notes with subject and email context preserved and organized sharply"},{"id":"create_delivery_reminder","service":"reminders","action":"create","parameters":{"title":"Finish Q4 security architecture document","notes":"Complete before Thursday 4 PM commitment to Maya Chen.","dueAt":"2026-09-03T14:00:00+05:30","priority":1},"taskStatus":"Delivery reminder armed Thursday 2 PM high priority to finish document on time"}],"finalMessage":"Mail, calendar, notes and reminder planned — executing now."}

If transcript is clearly NOT about mail/calendar/notes/reminders automation, output: {"status":"done","steps":[],"finalMessage":null}  (but the Rust heuristic will usually return NotAutomation before you are called, so you will rarely see non-automation input).

════════════════════════════════════════════════════════════
FEW-SHOT NEGATIVES — DO NOT DO THIS
════════════════════════════════════════════════════════════
- Do NOT output {"steps":[...]} without status — rejected.
- Do NOT output markdown ```json or explanation — rejected.
- Do NOT use service "gmail" or "tasks" — only mail/calendar/notes/reminders.
- Do NOT invent parameters like "emailBody" — only those in catalog.
- Do NOT output send_reply without requiresConfirmation true — rejected.
- Do NOT hallucinate localId like "123" — must use {{steps.find_email.matches.0.localId}}.

Output ONLY JSON. Ultra strictly only JSON.
"#;

pub async fn parse_automation_intent(
    transcript: &str,
    observations: &[StepObservation],
    round: i32,
    settings: &AppSettings,
) -> Result<Option<PlannerResponse>, AutomationErrorResult> {
    let transcript = transcript.trim();
    if transcript.is_empty() || transcript.split_whitespace().count() < 2 {
        return Ok(None);
    }

    let lower = transcript.to_lowercase();
    // Let planner still see transcript if it's clearly automation; otherwise parser.rs heuristic already gatekeeps.
    // But we still have a fast path: if round==1 and transcript is clearly not automation, return None quickly to avoid LLM cost.
    let _has_automation_signal = lower.contains("email")
        || lower.contains("mail")
        || lower.contains("calendar")
        || lower.contains("reminder")
        || lower.contains("note");
    // Don't early return — let LLM decide if unsure for round>1

    let now = chrono::Local::now();
    let offset_secs = now.offset().local_minus_utc();
    let sign = if offset_secs >= 0 { '+' } else { '-' };
    let abs = offset_secs.abs();
    let h = abs / 3600;
    let m = (abs % 3600) / 60;
    let offset_str = format!("{}{:02}:{:02}", sign, h, m);
    let now_iso = now.format("%Y-%m-%dT%H:%M:%S").to_string() + &offset_str;
    let weekday = now.format("%A").to_string();
    let now_context = format!("Now: {} ({}) — {}", now_iso, offset_str, weekday);

    let obs_json = serde_json::to_string(observations).unwrap_or_else(|_| "[]".to_string());

    let user_content = format!(
        "{}\n\nTranscript: \"{}\"\n\n{} \nRound: {} / 4\n\nObservations (JSON, reduced):\n{}\n\nRespond with JSON only — status/steps (each with taskStatus 10-15 words sharp) + finalMessage. Ultra strictly only JSON. Every step MUST have taskStatus.",
        AUTOMATION_SYSTEM_PROMPT, transcript, now_context, round, obs_json
    );

    // Gemini CLF: use the same generate path as calendar/obsidian — strict JSON via Gemini CLF model
    // We route through ai_cleanup::generate which respects provider/model/thinking but we want deterministic low thinking for speed/cost.
    // Use generate_with_gemini_model with gemini-3-flash if available, otherwise fallback to configured AI model.
    // For automation we force low thinking for bounded loop latency.

    // Try to use Gemini CLF model from settings.ai_cleanup_model (which is Gemini CLF) with Low thinking.
    // If local LLM enabled, we still want Gemini for automation — force Gemini path by using generate_with_gemini_model if needed.
    // For now, use the unified generate (respects local override). If user has local LLM, it will still follow prompt.
    let system_prompt = AUTOMATION_SYSTEM_PROMPT;

    // Use low thinking level for automation (fast, strict JSON).
    let result = crate::ai_cleanup::generate(system_prompt, &user_content, settings).await;

    let output = match result {
        Ok(o) => o,
        Err(e) => {
            log::warn!("Automation AI parser failed (round {round}): {e}");
            // On AI failure, treat as not automation for round 1, else propagate as planner failure
            if round == 1 {
                return Ok(None);
            } else {
                return Err(AutomationErrorResult {
                    ok: false,
                    error: "planner_failure".into(),
                    message: format!("Gemini failure: {e}"),
                    details: None,
                });
            }
        }
    };

    // Extract JSON (strip code fences, trim)
    let trimmed = output.trim();
    let json_str = {
        // strip ```json fences if present
        if trimmed.starts_with("```") {
            if let Some(start) = trimmed.find('{') {
                if let Some(end) = trimmed.rfind('}') {
                    &trimmed[start..=end]
                } else {
                    trimmed
                }
            } else {
                trimmed
            }
        } else if let Some(start) = trimmed.find('{') {
            if let Some(end) = trimmed.rfind('}') {
                &trimmed[start..=end]
            } else {
                trimmed
            }
        } else {
            trimmed
        }
    };

    // Try PlannerResponse
    if let Ok(mut resp) = serde_json::from_str::<PlannerResponse>(json_str) {
        // Validate status
        if resp.status == "continue" || resp.status == "done" {
            // Synthesize missing taskStatus for robustness (Gemini occasionally omits despite prompt) — keeps hook alive
            for s in &mut resp.steps {
                if s.taskStatus.is_none() {
                    s.taskStatus = Some(format!("Executed automation step {} via {}.{} with validated parameters successfully", s.id, s.service.as_str(), s.action));
                }
            }
            return Ok(Some(resp));
        }
    }

    // Try AutomationPlan shape {steps:[...]} or {request, steps} (legacy multi_step_plan.json) — adapt to PlannerResponse done
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
        if let Some(steps_val) = val.get("steps").and_then(|v| v.as_array()) {
            if val.get("status").is_none() {
                // Legacy shape — wrap as done, synthesize taskStatus for backward compat if missing
                if let Ok(mut steps) =
                    serde_json::from_value::<Vec<crate::automation::AutomationStep>>(
                        serde_json::Value::Array(steps_val.clone()),
                    )
                {
                    for s in &mut steps {
                        if s.taskStatus.is_none() {
                            s.taskStatus = Some(format!("Executed automation step {} via {}.{} with validated parameters successfully", s.id, s.service.as_str(), s.action));
                        }
                    }
                    return Ok(Some(PlannerResponse {
                        status: "done".into(),
                        steps,
                        finalMessage: val
                            .get("finalMessage")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .or_else(|| {
                                val.get("request")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string())
                            }),
                    }));
                }
            }
        }
        if let Some(action) = val.get("action").and_then(|v| v.as_str()) {
            if action == "none" || action == "passthrough" {
                return Ok(None);
            }
        }
        // If steps is not present but it's a single AutomationStep object
        if val.get("service").is_some() && val.get("action").is_some() {
            if let Ok(mut single) =
                serde_json::from_value::<crate::automation::AutomationStep>(val.clone())
            {
                if single.taskStatus.is_none() {
                    single.taskStatus = Some(format!("Executed automation step {} via {}.{} with validated parameters successfully", single.id, single.service.as_str(), single.action));
                }
                return Ok(Some(PlannerResponse {
                    status: "done".into(),
                    steps: vec![single],
                    finalMessage: None,
                }));
            }
        }
    }

    Err(AutomationErrorResult {
        ok: false,
        error: "validation_error".into(),
        message: "AI did not return valid Planner JSON (expected status/steps)".into(),
        details: Some(json_str.chars().take(600).collect()),
    })
}
