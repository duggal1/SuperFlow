//! Brutal validation for the deterministic post-STT English cleanup pipeline.
//!
//! Exercises the two public pipeline stages:
//!   - `superflow_grammar::correct`  (protected-span masking + Harper curated
//!     rules + Superflow custom `Sf*` rule families + exact span restoration)
//!   - `audio_toolkit::transcript_cleanup::normalize_transcript` (fillers,
//!     duplicates, restarts, self-corrections, modifier stacking)
//!
//! Run with: `cargo test --test brutal_validation -- --nocapture`

use superflow_app_lib::audio_toolkit::transcript_cleanup::normalize_transcript;
use superflow_app_lib::superflow_grammar::correct;

/// Pipeline order A: Harper first, then disfluency cleanup (the order wired into
/// `post_process_transcription_text`).
fn pipeline_a(raw: &str) -> String {
    normalize_transcript(&correct(raw))
}

/// Pipeline order B: disfluency cleanup first, then Harper (the order in the
/// task's architecture diagram). Compared to prove which is safer.
fn pipeline_b(raw: &str) -> String {
    correct(&normalize_transcript(raw))
}

fn show(tag: &str, raw: &str) {
    let cleanup = normalize_transcript(raw);
    let grammar = correct(raw);
    let a = pipeline_a(raw);
    let b = pipeline_b(raw);
    println!(
        "\n[{tag}]
RAW STT : {raw}
CLEANUP : {cleanup}
GRAMMAR : {grammar}
A(h→c)  : {a}
B(c→h)  : {b}"
    );
}

#[test]
fn brutal_validation_report() {
    // --- Real probe from the task (must NOT be hardcoded; exposes patterns) ---
    show(
        "PROBE: restart + clause dup + modal stacking",
        "You Promise me that you will do things for me that I never thought I could it could have and I could have it couldn't be possible",
    );

    show(
        "self-correction (I mean)",
        "I can I mean I can't do that",
    );
    show(
        "self-correction (no alt)",
        "we should ship Tuesday no Wednesday",
    );
    show(
        "self-correction (I mean, modal)",
        "it could have I mean it couldn't have happened",
    );

    show("abandoned start", "I could I mean couldn't we could be doing this but I don't I don't think it actually it actually works right");

    show("dup verb", "we have have finished");
    show("dup word", "the the issue is clear");
    show("dup phrase", "it actually it actually works");
    show("intensifier stack", "really extremely very extremely important");
    show("intensifier keep", "high-quality deterministic grammar engine");

    show(
        "USER: comma-stuttered don't",
        "No, I could have, I should have, but I don't, don't, don't, don't, don't wanna do it. What do I do it? Do it",
    );

    // --- Brutal-set survivors from prior ultra_brutal run (350w) ---
    show("quantifier", "there is many problem with the build");
    show("demonstrative", "this are a test of the system");
    show("irregular seen", "i seen the error in the log");
    show("compound subj", "me and him was reviewing the code");
    show("compound mod", "a long term plan for a well known issue");
    show("couple of", "a couple of problem remain");
    show("perfect", "we have went home after we have wrote the test");

    // --- Clean English must be unchanged ---
    for clean in [
        "The build is green and the tests pass.",
        "We should ship on Wednesday.",
        "I saw the error in the log.",
        "Let's meet Thursday next week.",
    ] {
        let a = pipeline_a(clean);
        let b = pipeline_b(clean);
        assert_eq!(a, clean, "A regressed clean text: {clean} -> {a}");
        assert_eq!(b, clean, "B regressed clean text: {clean} -> {b}");
    }

    // --- Deterministic: same input -> identical output ---
    for raw in [
        "the the issue is clear",
        "we should ship Tuesday no Wednesday",
        "really extremely very extremely important",
        "You Promise me that you will do things for me that I never thought I could it could have and I could have it couldn't be possible",
    ] {
        assert_eq!(pipeline_a(raw), pipeline_a(raw), "A non-deterministic: {raw}");
        assert_eq!(pipeline_b(raw), pipeline_b(raw), "B non-deterministic: {raw}");
    }

    // --- Technical preservation ---
    let tech = "fix getUserById in src-tauri/src/audio_toolkit/transcript_cleanup.rs and ping @sarah about localhost:1420 and .env.local";
    let a = pipeline_a(tech);
    for tok in [
        "getUserById",
        "src-tauri/src/audio_toolkit/transcript_cleanup.rs",
        "@sarah",
        "localhost:1420",
        ".env.local",
    ] {
        assert!(a.contains(tok), "A lost tech token {tok}: {a}");
    }
}
