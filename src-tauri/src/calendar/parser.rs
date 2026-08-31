use crate::calendar::{CalendarAiOutput, CalendarErrorResult};
use crate::settings::AppSettings;

const CALENDAR_SYSTEM_PROMPT: &str = r#"You are Superflow's Calendar intent parser. Convert natural spoken transcription into strict JSON for Apple Calendar. Never generate AppleScript, shell, or Swift code. Only output JSON.

Current time context will be provided as: "Now: 2026-09-01T00:00:00+05:30 (Asia/Kolkata) - Friday". Use it to resolve relative dates like tomorrow, next Tuesday, Friday, etc. Always output absolute ISO8601 start/end with timezone offset like "2026-09-01T09:30:00+05:30".

Rules:
- For professional scheduling: interviews, client calls, finance reviews, month-end close, project deadlines, hiring, planning, operational meetings. Accept natural phrasing like "Schedule my month-end finance review tomorrow at 9:30 AM for 45 minutes. Remind me 15 minutes before." or "Set up an interview debrief Friday at 4 PM for an hour." Do not require robotic commands.
- If transcript is clearly about calendar (contains schedule, set up, add, block, calendar, interview, review, meeting, debrief, etc. with a time), output calendar.create_event JSON.
- If transcript is NOT calendar-related, output {"action":"none"}.
- Strict schema for create:
{
  "action": "calendar.create_event",
  "title": "string (2-80 chars, professional title, not generic)",
  "start": "ISO8601 with offset, e.g. 2026-09-01T09:30:00+05:30",
  "end": "ISO8601 with offset, must be after start",
  "calendar": "Work" or null (prefer Work for professional, Personal for personal, or null to use default writable calendar; never invent new calendar per event)",
  "location": null or string,
  "notes": null or string,
  "reminders_minutes_before": [15] or [10] or null (array of ints 0-40320, use what user said, e.g. "remind me 15 minutes before" -> [15], "remind me 10 minutes before" -> [10], if not mentioned -> null or []),
  "success_message": "3-5 words, extremely clean, deterministic, e.g. Calendar booked or Meeting scheduled or Finance review booked"
}
- Keep success_message 3-5 words, extremely clean, deterministic. Good: "Calendar booked" "Meeting scheduled" "Finance review booked" Bad: "Action completed successfully." "Calendar operation successful."
- End = start + duration. If user says "for 45 minutes", end = start +45m. If "for an hour", +60m. If "Block 30 minutes", +30m. If no duration, default 60 minutes.
- Calendar: use Work for professional contexts unless user says personal.
- Reminders: only if user explicitly says remind. Otherwise null.
- If required info is genuinely missing (e.g., "schedule my finance review tomorrow morning" has no exact time, or "schedule interview Friday" with no time), do NOT invent time. Output needs_clarification:
{
  "action": "calendar.needs_clarification",
  "missing_fields": ["start"],
  "clarification_message": "What time tomorrow morning?"
}
  Keep clarification_message short, natural, asking for what's missing.
- Never invent professional details. If daypart vague like "morning" without time, ask clarification unless user has explicit default policy (they don't).
- Output ONLY JSON, no markdown, no explanation, no code fences.

Examples:

Input: "Schedule my month-end finance review tomorrow at 9:30 AM for 45 minutes. Remind me 15 minutes before."
Now: 2026-08-31T00:00:00+05:30
Output: {"action":"calendar.create_event","title":"Month-end finance review","start":"2026-09-01T09:30:00+05:30","end":"2026-09-01T10:15:00+05:30","calendar":"Work","location":null,"notes":null,"reminders_minutes_before":[15],"success_message":"Calendar booked"}

Input: "Set up an interview debrief Friday at 4 PM for an hour."
Now: 2026-08-31T00:00:00+05:30 (Monday)
Output: {"action":"calendar.create_event","title":"Interview debrief","start":"2026-09-05T16:00:00+05:30","end":"2026-09-05T17:00:00+05:30","calendar":"Work","location":null,"notes":null,"reminders_minutes_before":null,"success_message":"Meeting scheduled"}

Input: "Add a client renewal review next Tuesday at 10 AM. Block 30 minutes and remind me 10 minutes before."
Output: {"action":"calendar.create_event","title":"Client renewal review","start":"2026-09-08T10:00:00+05:30","end":"2026-09-08T10:30:00+05:30","calendar":"Work","location":null,"notes":null,"reminders_minutes_before":[10],"success_message":"Calendar booked"}

Input: "schedule my finance review tomorrow morning"
Output: {"action":"calendar.needs_clarification","missing_fields":["start"],"clarification_message":"What time tomorrow morning?"}

Input: "hello how are you" (not calendar)
Output: {"action":"none"}
"#;

pub async fn parse_calendar_intent(
    transcript: &str,
    settings: &AppSettings,
) -> Result<Option<CalendarAiOutput>, CalendarErrorResult> {
    let transcript = transcript.trim();
    if transcript.is_empty() || transcript.split_whitespace().count() < 3 {
        return Ok(None);
    }
    // Quick heuristic to avoid calling LLM for non-calendar transcripts (keeps costs low and deterministic)
    let lower = transcript.to_lowercase();
    let has_calendar_signal = lower.contains("schedule")
        || lower.contains("calendar")
        || lower.contains("event")
        || lower.contains("interview")
        || lower.contains("review")
        || lower.contains("meeting")
        || lower.contains("debrief")
        || lower.contains("block")
        || lower.contains("remind")
        || lower.contains("month-end")
        || lower.contains("finance");
    let has_time_signal = lower.contains("am")
        || lower.contains("pm")
        || lower.contains("tomorrow")
        || lower.contains("today")
        || lower.contains("monday")
        || lower.contains("tuesday")
        || lower.contains("wednesday")
        || lower.contains("thursday")
        || lower.contains("friday")
        || lower.contains("saturday")
        || lower.contains("sunday")
        || lower.contains("next")
        || lower.contains(":")
        || lower.contains("at ");
    if !has_calendar_signal && !has_time_signal {
        // Still let AI decide but we can early return none for obvious non-calendar like "hello world" to save LLM call
        // However for safety, we still call AI if it might be calendar but heuristic missed; but we can be conservative:
        // If no time signal at all, it's likely not calendar with required start, so return none quickly
        if !has_time_signal {
            return Ok(None);
        }
    }

    let now = chrono::Local::now();
    // Build offset string like +05:30
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
        CALENDAR_SYSTEM_PROMPT, transcript, now_context
    );

    // Use the existing AI generate path (Gemini or local LLM) - same as super whisper
    // We call ai_cleanup::generate with a custom system prompt for calendar
    let system_prompt = CALENDAR_SYSTEM_PROMPT;
    // Build a minimal settings clone? We need to call generate with system_prompt and user_content
    // ai_cleanup::generate takes system_prompt and user_content and routes via settings
    let result = crate::ai_cleanup::generate(system_prompt, &user_content, settings).await;

    let output = match result {
        Ok(o) => o,
        Err(e) => {
            // If AI fails, treat as not calendar (fallback to normal paste) but log
            log::warn!("Calendar AI parser failed: {}", e);
            return Ok(None);
        }
    };

    // Validate output as calendar JSON
    match crate::calendar::validate_ai_output(&output) {
        Ok(CalendarAiOutput::Create(req)) => {
            // Validate fields fully (including success_message etc)
            match crate::calendar::validate_create_request(&req) {
                Ok(_) => Ok(Some(CalendarAiOutput::Create(req))),
                Err(e) => Err(e),
            }
        }
        Ok(CalendarAiOutput::Clarify(c)) => Ok(Some(CalendarAiOutput::Clarify(c))),
        Ok(CalendarAiOutput::None { .. }) => Ok(None),
        Err(e) => Err(e),
    }
}
