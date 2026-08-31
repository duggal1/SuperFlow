/// Resolve the local speaker label from the user's profile without guessing.
pub fn resolve_you_name(settings: &crate::settings::AppSettings) -> String {
    let Ok(specification) = serde_json::from_str::<serde_json::Value>(&settings.user_specification)
    else {
        return "You".to_string();
    };

    specification
        .get("identity")
        .and_then(|identity| identity.get("full_name"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .or_else(|| {
            specification
                .get("identity")
                .and_then(|identity| identity.get("preferred_name"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())
        })
        .or_else(|| {
            specification
                .get("preferredName")
                .and_then(serde_json::Value::as_str)
        })
        .map(str::to_string)
        .unwrap_or_else(|| "You".to_string())
}

pub fn timestamped_segments(
    speaker: &str,
    transcript: &str,
    duration_ms: i64,
) -> Vec<crate::meeting::manager::MeetingSegment> {
    let text = transcript.trim();
    if text.is_empty() {
        return Vec::new();
    }

    let mut sentences = Vec::new();
    let mut start = 0;
    for (index, character) in text.char_indices() {
        if matches!(character, '.' | '?' | '!' | '\n') {
            let end = index + character.len_utf8();
            let sentence = text[start..end].trim();
            if !sentence.is_empty() {
                sentences.push(sentence);
            }
            start = end;
        }
    }
    let remainder = text[start..].trim();
    if !remainder.is_empty() {
        sentences.push(remainder);
    }
    if sentences.is_empty() {
        sentences.push(text);
    }

    let total_weight = sentences
        .iter()
        .map(|sentence| sentence.chars().count().max(1) as i64)
        .sum::<i64>();
    let sentence_count = sentences.len();
    let duration_ms = duration_ms.max(1);
    let mut consumed_weight = 0_i64;

    sentences
        .into_iter()
        .enumerate()
        .map(|(index, sentence)| {
            let start_ms = duration_ms.saturating_mul(consumed_weight) / total_weight;
            consumed_weight += sentence.chars().count().max(1) as i64;
            let end_ms = if index + 1 == sentence_count {
                duration_ms
            } else {
                (duration_ms.saturating_mul(consumed_weight) / total_weight).max(start_ms + 1)
            };
            crate::meeting::manager::MeetingSegment {
                speaker: speaker.to_string(),
                start_ms,
                end_ms: end_ms.min(duration_ms),
                text: sentence.to_string(),
            }
        })
        .collect()
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn meeting_system_audio_start() -> bool;
    fn meeting_system_audio_stop() -> bool;
    fn meeting_system_audio_is_capturing() -> bool;
}

pub fn start_system_audio() -> bool {
    if let Ok(mut samples) = SYSTEM_SAMPLES.lock() {
        samples.clear();
    }
    #[cfg(target_os = "macos")]
    unsafe {
        meeting_system_audio_start()
    }
    #[cfg(not(target_os = "macos"))]
    false
}

pub fn take_system_samples() -> Vec<f32> {
    SYSTEM_SAMPLES
        .lock()
        .map(|mut samples| std::mem::take(&mut *samples))
        .unwrap_or_default()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn meeting_system_audio_samples(
    samples: *const f32,
    sample_count: usize,
    channel_count: usize,
    sample_rate: usize,
) {
    if samples.is_null() || sample_count == 0 || channel_count == 0 || sample_rate < 16_000 {
        return;
    }
    let input = unsafe { std::slice::from_raw_parts(samples, sample_count) };
    let frame_count = sample_count / channel_count;
    let step = sample_rate as f64 / 16_000.0;
    let Ok(mut output) = SYSTEM_SAMPLES.lock() else {
        return;
    };
    let mut source_frame = 0.0_f64;
    while (source_frame as usize) < frame_count {
        let frame = source_frame as usize;
        let base = frame * channel_count;
        let mono = input[base..base + channel_count].iter().sum::<f32>() / channel_count as f32;
        output.push(mono);
        source_frame += step;
    }
}

pub fn stop_system_audio() -> bool {
    #[cfg(target_os = "macos")]
    unsafe {
        meeting_system_audio_stop()
    }
    #[cfg(not(target_os = "macos"))]
    false
}

#[allow(dead_code)]
pub fn is_system_audio_capturing() -> bool {
    #[cfg(target_os = "macos")]
    unsafe {
        meeting_system_audio_is_capturing()
    }
    #[cfg(not(target_os = "macos"))]
    false
}

#[cfg(test)]
mod tests {
    use super::{resolve_you_name, timestamped_segments};

    #[test]
    fn always_returns_a_non_empty_local_speaker_label() {
        let settings = crate::settings::get_default_settings();
        assert!(!resolve_you_name(&settings).trim().is_empty());
    }

    #[test]
    fn profile_preferred_name_labels_the_local_speaker() {
        let mut settings = crate::settings::get_default_settings();
        settings.user_specification =
            r#"{"identity":{"full_name":"","preferred_name":"Harshit"}}"#.to_string();
        assert_eq!(resolve_you_name(&settings), "Harshit");
    }

    #[test]
    fn transcript_sentences_receive_progressive_timestamps() {
        let segments = timestamped_segments(
            "You",
            "First decision. Second decision! Final question?",
            90_000,
        );
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].start_ms, 0);
        assert!(segments[1].start_ms > segments[0].start_ms);
        assert!(segments[2].start_ms > segments[1].start_ms);
        assert_eq!(segments[2].end_ms, 90_000);
    }
}
use once_cell::sync::Lazy;
use std::sync::Mutex;

static SYSTEM_SAMPLES: Lazy<Mutex<Vec<f32>>> = Lazy::new(|| Mutex::new(Vec::new()));
