pub mod audio;
pub mod catalog;
pub mod constants;
pub mod formatter;
pub mod lang_id;
pub mod normalization;
pub mod programming_syntax;
pub mod punctuation;
pub mod styling;
pub mod tech_lexicon;
pub mod text;
pub mod utils;
pub mod vad;

pub use audio::{
    is_microphone_access_denied, is_no_input_device_error, list_input_devices, list_output_devices,
    read_wav_samples, save_wav_file, verify_wav_file, AudioRecorder, CpalDeviceInfo, VadPolicy,
};
pub use lang_id::detect_output_language;
pub use text::{
    apply_custom_words, join_path_tokens, normalize_transcription_output, remove_filler_words,
    OutputLanguageEvidence,
};
pub use utils::get_cpal_host;
pub use vad::{SileroVad, VoiceActivityDetector};

/// Vocabulary-only normalization applied AFTER S1-mini cleanup (T2.5).
///
/// Canonical brand/framework spellings, spoken Tailwind classes, and explicit
/// technical tokens are restored here so they never corrupt the model's input
/// as pre-rewritten prose. Everything is meaning-preserving: near-exact alias
/// hits only, ambiguous bare English words denylisted in the programming
/// catalog, plus whitespace/stutter hygiene and spoken-path rejoining.
pub fn apply_post_cleanup_vocabulary(text: &str, tech_lexicon_enabled: bool) -> String {
    if text.is_empty() {
        return text.to_string();
    }
    let corrected = if tech_lexicon_enabled {
        let corrected = tech_lexicon::apply(text);
        let corrected = styling::apply(&corrected);
        programming_syntax::apply(&corrected)
    } else {
        text.to_string()
    };
    join_path_tokens(&normalize_transcription_output(&corrected))
}
