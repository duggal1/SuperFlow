use crate::managers::history::HistoryEntry;
use chrono::{DateTime, Local};

/// Final text for an entry: the post-processed pass when one exists, otherwise
/// the raw transcription.
fn final_text(entry: &HistoryEntry) -> &str {
    match &entry.post_processed_text {
        Some(text) if !text.trim().is_empty() => text,
        _ => &entry.transcription_text,
    }
}

/// Entries that carry an actual transcript (empty rows are failed recordings).
fn exported_entries(entries: &[HistoryEntry]) -> Vec<&HistoryEntry> {
    entries
        .iter()
        .filter(|e| !final_text(e).trim().is_empty())
        .collect()
}

fn format_timestamp(timestamp: i64) -> String {
    DateTime::from_timestamp(timestamp, 0)
        .map(|utc| {
            utc.with_timezone(&Local)
                .format("%B %e, %Y at %l:%M %p")
                .to_string()
        })
        .unwrap_or_else(|| format!("Recording {timestamp}"))
}

/// All transcripts as a clean Markdown document: one `##` section per entry,
/// chronological, with a light document header and hairline separators.
pub fn format_markdown(entries: &[HistoryEntry]) -> String {
    let rows = exported_entries(entries);
    let mut out = String::new();

    out.push_str("# Transcripts\n\n");
    if rows.is_empty() {
        out.push_str("_No transcriptions yet._\n");
        return out;
    }

    out.push_str(&format!(
        "_{} {} · exported {}_\n",
        rows.len(),
        if rows.len() == 1 { "entry" } else { "entries" },
        Local::now().format("%B %e, %Y"),
    ));

    for entry in rows {
        out.push_str("\n---\n\n");
        out.push_str(&format!("## {}\n\n", entry.title.trim()));
        out.push_str(&format!(
            "_{}_ \n\n{}\n",
            format_timestamp(entry.timestamp),
            final_text(entry).trim()
        ));
    }

    out
}

/// All transcripts as plain text: title line, date line, blank line, text,
/// with a rule between entries.
pub fn format_plain_text(entries: &[HistoryEntry]) -> String {
    let rows = exported_entries(entries);
    const RULE: &str =
        "--------------------------------------------------------------------------------\n";
    let mut out = String::from("SUPERFLOW TRANSCRIPTS\n");

    if rows.is_empty() {
        out.push_str("\nNo transcriptions yet.\n");
        return out;
    }

    out.push_str(&format!(
        "Exported {}\n\n",
        Local::now().format("%B %e, %Y")
    ));

    for entry in rows {
        out.push_str(RULE);
        out.push('\n');
        out.push_str(&format!("{}\n", entry.title.trim()));
        out.push_str(&format!("{}\n\n", format_timestamp(entry.timestamp)));
        out.push_str(&format!("{}\n\n", final_text(entry).trim()));
    }
    out.push_str(RULE);

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(timestamp: i64, text: &str, post_processed: Option<&str>) -> HistoryEntry {
        HistoryEntry {
            id: timestamp,
            file_name: format!("{timestamp}.wav"),
            timestamp,
            saved: false,
            title: format!("Recording {timestamp}"),
            transcription_text: text.to_string(),
            post_processed_text: post_processed.map(str::to_string),
            post_process_prompt: None,
            post_process_requested: false,
            word_count: 0,
            audio_duration_secs: None,
            avg_wpm: None,
            time_saved_secs: None,
        }
    }

    #[test]
    fn markdown_skips_empty_and_prefers_post_processed() {
        let entries = [
            entry(100, "raw one", Some("clean one")),
            entry(200, "   ", None),
            entry(300, "two", None),
        ];

        let md = format_markdown(&entries);

        assert!(md.starts_with("# Transcripts"));
        assert!(md.contains("clean one"));
        assert!(!md.contains("raw one"));
        assert!(md.contains("## Recording 300"));
        assert_eq!(md.matches("\n---\n").count(), 2);
    }

    #[test]
    fn markdown_handles_no_transcripts() {
        let md = format_markdown(&[]);
        assert!(md.contains("No transcriptions yet."));
    }

    #[test]
    fn plain_text_contains_titles_and_final_text() {
        let entries = [entry(100, "hello world", None), entry(200, "   ", None)];

        let txt = format_plain_text(&entries);

        assert!(txt.starts_with("SUPERFLOW TRANSCRIPTS"));
        assert!(txt.contains("Recording 100"));
        assert!(txt.contains("hello world"));
        assert!(!txt.contains("Recording 200"));
    }
}
