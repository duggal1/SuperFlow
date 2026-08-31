use crate::meeting::manager::{MeetingIntelligence, MeetingSegment};
use crate::settings::AiCleanupThinkingLevel;

const MODEL: &str = "gemini-3.6-flash";
const SYSTEM_PROMPT: &str = r#"Analyze professional conversations with direct, evidence-based judgment.

Rules:
- Be brutally honest, precise, and useful. No praise unless the transcript earns it.
- No sycophancy, motivational filler, therapy language, or invented intent.
- Separate observed transcript evidence from inference.
- Never claim to know what another participant privately thought.
- Ground criticism in a timestamp and concrete evidence whenever possible.
- Return valid JSON only. Include every field in the schema. Use [] when a section has no supported findings."#;

const ANALYSIS_PROMPT: &str = r#"Infer the meeting type, then evaluate the conversation using the rubric appropriate to that type.

Return exactly this JSON object:
{
  "meeting_type": "job_interview|engineering_discussion|client_meeting|sales|finance|planning|management|negotiation|brainstorming|podcast|other",
  "outcome": "short factual assessment",
  "what_went_well": [],
  "mistakes": [],
  "missed_opportunities": [],
  "communication_issues": [],
  "important_decisions": [],
  "action_items": [],
  "risks": [],
  "lessons": [],
  "next_time": []
}

Finding objects use:
{"issue":"","timestamp":"MM:SS","evidence":"","why_it_matters":"","better_approach":""}

Keep only material findings. Do not omit schema fields."#;

const ASK_PROMPT: &str = r#"Answer only from the supplied meeting transcript. If the answer is absent, say: Not discussed in this meeting. Cite relevant timestamps in square brackets. Never fabricate names, decisions, motives, or facts.

Writing style: a sharp, busy professional wrote this. Correct, efficient, human. Keep every fact; invent nothing.
- Contractions are fine. Short, direct sentences. One idea each. No padding.
- Plain, competent, adult voice. No slang, no emoji, no stiffness. State things; don't cushion them.
- Cut filler: -ing padding ("highlighting", "fostering"), sales language, vague sourcing, legacy claims ("testament to", "evolving landscape"), overused AI words ("crucial", "delve", "intricate", "showcase", "underscore").
- No em/en dashes: use a period, comma, or colon. No bold-label lists, no title case, no curly quotes, no chatbot sign-offs ("hope this helps", "let me know"). No "not just X, it's Y". No forced triads. End on the last real fact.

Format — structured Markdown, not a wall of text:
- Open with one direct sentence that answers the question.
- Use short sections with ## headings when the answer has distinct parts.
- Use tight bullet lists (one line per bullet where possible) for enumerations: decisions, action items, reasons, risks.
- Bold only key names or terms inline; never bold whole sentences.
- Return Markdown only. No preamble, no closing remarks."#;

pub fn derive_title(transcript: &str) -> String {
    let words = transcript
        .split_whitespace()
        .map(|word| word.trim_matches(|character: char| !character.is_alphanumeric()))
        .filter(|word| word.len() > 1)
        .take(6)
        .collect::<Vec<_>>();
    if words.len() < 2 {
        return "Untitled Meeting".to_string();
    }
    let title = words.join(" ");
    let mut characters = title.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => "Untitled Meeting".to_string(),
    }
}

pub async fn generate_intelligence(
    transcript_segments: &[MeetingSegment],
    settings: &crate::settings::AppSettings,
) -> Result<MeetingIntelligence, String> {
    let transcript = meaningful_transcript(transcript_segments)?;
    let user_content = format!("{ANALYSIS_PROMPT}\n\nTranscript:\n{transcript}");
    let output = crate::ai_cleanup::generate_with_gemini_model(
        MODEL,
        AiCleanupThinkingLevel::Low,
        SYSTEM_PROMPT,
        &user_content,
        settings,
    )
    .await?;
    let json = extract_json(&output)?;
    serde_json::from_str(json)
        .map_err(|error| format!("Intelligence returned invalid JSON: {error}. Please try again."))
}

pub async fn ask_anything(
    question: &str,
    transcript_segments: &[MeetingSegment],
    settings: &crate::settings::AppSettings,
) -> Result<String, String> {
    let transcript = meaningful_transcript(transcript_segments)?;
    let user_content = format!("Transcript:\n{transcript}\n\nQuestion: {}", question.trim());
    crate::ai_cleanup::generate_with_gemini_model(
        MODEL,
        AiCleanupThinkingLevel::Low,
        ASK_PROMPT,
        &user_content,
        settings,
    )
    .await
}

fn meaningful_transcript(segments: &[MeetingSegment]) -> Result<String, String> {
    let transcript = segments
        .iter()
        .filter(|segment| !segment.text.trim().is_empty())
        .map(format_segment)
        .collect::<Vec<_>>()
        .join("\n\n");
    if transcript.trim().is_empty() {
        Err("This meeting has no transcript to analyze.".to_string())
    } else {
        Ok(transcript)
    }
}

fn extract_json(output: &str) -> Result<&str, String> {
    let start = output
        .find('{')
        .ok_or_else(|| "Intelligence did not return JSON. Please try again.".to_string())?;
    let end = output
        .rfind('}')
        .filter(|end| *end >= start)
        .ok_or_else(|| "Intelligence returned incomplete JSON. Please try again.".to_string())?;
    Ok(&output[start..=end])
}

fn format_segment(segment: &MeetingSegment) -> String {
    let total_seconds = segment.start_ms.max(0) / 1_000;
    let hours = total_seconds / 3_600;
    let minutes = (total_seconds % 3_600) / 60;
    let seconds = total_seconds % 60;
    let timestamp = if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    };
    format!(
        "{} [{}]: {}",
        segment.speaker,
        timestamp,
        segment.text.trim()
    )
}

#[cfg(test)]
mod tests {
    use super::{derive_title, extract_json, meaningful_transcript};
    use crate::meeting::manager::{MeetingIntelligence, MeetingSegment};

    #[test]
    fn omitted_optional_sections_default_to_empty_arrays() {
        let parsed = serde_json::from_str::<MeetingIntelligence>(
            r#"{"meeting_type":"other","outcome":"No outcome","mistakes":[]}"#,
        )
        .unwrap();
        assert!(parsed.what_went_well.is_empty());
        assert!(parsed.action_items.is_empty());
    }

    #[test]
    fn empty_transcripts_never_reach_ai() {
        let segments = vec![MeetingSegment {
            speaker: "You".to_string(),
            start_ms: 0,
            end_ms: 1_000,
            text: "  ".to_string(),
        }];
        assert!(meaningful_transcript(&segments).is_err());
    }

    #[test]
    fn local_title_requires_no_ai() {
        assert_eq!(
            derive_title("frontend architecture release planning tomorrow"),
            "Frontend architecture release planning tomorrow"
        );
    }

    #[test]
    fn extracts_json_without_markdown_dependency() {
        assert_eq!(
            extract_json("```json\n{\"outcome\":\"ok\"}\n```").unwrap(),
            "{\"outcome\":\"ok\"}"
        );
    }
}
