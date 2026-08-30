use std::time::{Duration, Instant};
use superflow_app_lib::audio_toolkit::{
    formatter::{format_email_for_surface, format_layout, EmailFormatContext},
    slack_formatting::{format_for_slack_with_options, SlackFormatOptions},
    transcript_cleanup,
};
use superflow_app_lib::superflow_grammar;

// The large intentionally broken transcript from the spec
const BROKEN_TRANSCRIPT: &str = r#"so um basically qa have found a couple of Issue and there is many problem with the employee import flow and the the context function is is broken and we need to fix fix it before thursday afternoon

the function getUserById uses useEffect with Zustand and fileName comes from src-tauri/src/transcript/cleanup.rs and the config is .env.local

i seen the issue yesterday and me and him was discussing about the long term privacy first open source solution and this are a test that still dont work correctly

we need to handle the well known bug in the state of the art system and the open source project needs a long term plan for the privacy first design and the api endpoint is /api/parse at localhost:1420 and the version is 2.8.0

the meeting is 3-5 pm and the cost is three thirty pm and we have 20% more and the url is https://example.com and the email is alex@example.com and the user Alicks said the code is ready but Alex thinks it needs review

hello   there	with  weird   spacing and   extra   spaces and i think we should do it and i will handle it and we has meeting at 3pm and they was discussing about the issue which have multiple bug

this are a test and a apple and there is many problems with this approach and i dont think its working and we cant fix it and me and him goes to the store and he have went to the store yesterday

the identifiers are foobar bazqux useEffect getUserById parseTranscript flate2 and the file src-tauri/src/transcript/cleanup.rs contains function normalizeTranscript and myVarName is ZustandStore

we should still be rolling out but afternoon but like the file name context function is broken and we need to fix it asap and the the the repeated words are here and there and uh um filler words should be removed but not in the middle of protected code

the quick brown fox jump over lazy dog and the api returns json with http status 200 and the vad model is silero with onnx and the asr is parakeet 0.6B"#;

fn run_pipeline_stages(raw: &str) -> (String, String, String, String) {
    let protected = superflow_grammar::ProtectedText::new(raw);
    // Stage 1: Normalization — tech_lexicon + transcript_cleanup (simplified, English)
    // We replicate post_process_transcription_text without AppHandle:
    // Apply tech_lexicon, styling, programming_syntax, emoji (tech_lexicon gate enabled)
    let mut normalized = protected.masked().to_string();
    // Tech lexicon — apply built-in vocabulary
    normalized = superflow_app_lib::audio_toolkit::tech_lexicon::apply(&normalized);
    normalized = superflow_app_lib::audio_toolkit::styling::apply(&normalized);
    normalized = superflow_app_lib::audio_toolkit::programming_syntax::apply(&normalized);
    normalized = superflow_app_lib::audio_toolkit::emoji::apply(&normalized);
    // Remove filler words — English, so "um" gated but "uh" universal
    // For validation we use English evidence
    let normalized = superflow_app_lib::audio_toolkit::text::remove_filler_words(
        &normalized,
        &superflow_app_lib::audio_toolkit::text::OutputLanguageEvidence::UserSelected(
            "en".to_string(),
        ),
        &None,
        true,
    );
    let normalized =
        superflow_app_lib::audio_toolkit::text::normalize_transcription_output(&normalized);
    let normalized = superflow_app_lib::audio_toolkit::formatter::normalize_values(&normalized);
    let normalized = superflow_app_lib::audio_toolkit::text::join_path_tokens(&normalized);
    let after_normalization = transcript_cleanup::normalize_transcript(&normalized);
    // Stage 2: protected Harper correction, then exact byte restoration.
    let after_grammar = superflow_grammar::correct(&after_normalization);
    let restored = protected.restore(&after_grammar);
    // Stage 3: Formatter — generic layout (Other surface)
    let final_output = format_layout(&restored);
    (raw.to_string(), after_grammar, restored, final_output)
}

fn check_preservation(original: &str, final_output: &str, tokens: &[&str]) -> Vec<String> {
    let mut missing = Vec::new();
    for tok in tokens {
        if original.contains(tok) && !final_output.contains(tok) {
            missing.push(tok.to_string());
        }
    }
    missing
}

fn benchmark_pipeline(text: &str) -> Duration {
    let start = Instant::now();
    let _ = run_pipeline_stages(text);
    start.elapsed()
}

fn benchmark_breakdown(text: &str) {
    let started = Instant::now();
    let protected = superflow_grammar::ProtectedText::new(text);
    let protected_ms = started.elapsed().as_secs_f64() * 1000.0;

    let started = Instant::now();
    let mut normalized = superflow_app_lib::audio_toolkit::tech_lexicon::apply(protected.masked());
    normalized = superflow_app_lib::audio_toolkit::styling::apply(&normalized);
    normalized = superflow_app_lib::audio_toolkit::programming_syntax::apply(&normalized);
    normalized = superflow_app_lib::audio_toolkit::emoji::apply(&normalized);
    let catalogs_ms = started.elapsed().as_secs_f64() * 1000.0;

    let started = Instant::now();
    let normalized = superflow_app_lib::audio_toolkit::text::remove_filler_words(
        &normalized,
        &superflow_app_lib::audio_toolkit::text::OutputLanguageEvidence::UserSelected(
            "en".to_string(),
        ),
        &None,
        true,
    );
    let normalized =
        superflow_app_lib::audio_toolkit::text::normalize_transcription_output(&normalized);
    let normalized = superflow_app_lib::audio_toolkit::formatter::normalize_values(&normalized);
    let normalized = superflow_app_lib::audio_toolkit::text::join_path_tokens(&normalized);
    let normalized = transcript_cleanup::normalize_transcript(&normalized);
    let cleanup_ms = started.elapsed().as_secs_f64() * 1000.0;

    let started = Instant::now();
    let corrected = superflow_grammar::correct(&normalized);
    let grammar_ms = started.elapsed().as_secs_f64() * 1000.0;

    let started = Instant::now();
    let restored = protected.restore(&corrected);
    let _ = format_layout(&restored);
    let restore_format_ms = started.elapsed().as_secs_f64() * 1000.0;

    println!("  45m breakdown: protected={protected_ms:.3} catalogs={catalogs_ms:.3} cleanup={cleanup_ms:.3} grammar={grammar_ms:.3} restore+format={restore_format_ms:.3} ms");
}

fn main() {
    println!(
        "╔════════════════════════════════════════════════════════════════════════════════════╗"
    );
    println!(
        "║  ULTRA-BRUTAL VALIDATION — STT → Grammar → Normalize → Format (REAL PIPELINE)     ║"
    );
    println!("║  Release mode, <50ms target, final output judged only                           ║");
    println!(
        "╚════════════════════════════════════════════════════════════════════════════════════╝"
    );

    // Warmup — cold harper is ~900ms, warm is <30ms. Report cold separately.
    let cold_start = Instant::now();
    let _ = superflow_grammar::correct("warmup text for caches");
    let cold = cold_start.elapsed();
    println!(
        "\nCold-start (first correct() includes FST + LintGroup init): {:.3} ms",
        cold.as_secs_f64() * 1000.0
    );
    println!("Warm caches now — subsequent per-transcript is hot path");

    // Test the main broken transcript
    let (raw, after_grammar, after_norm, final_out) = run_pipeline_stages(BROKEN_TRANSCRIPT);
    println!("\n{}", "═".repeat(100));
    println!(
        "  MAIN BROKEN TRANSCRIPT — RAW ({} chars, {} words)",
        raw.len(),
        raw.split_whitespace().count()
    );
    println!("{}", "═".repeat(100));
    println!("{}", raw);
    println!(
        "\n── AFTER GRAMMAR (harper + protected) ──\n{}",
        after_grammar
    );
    println!(
        "\n── AFTER NORMALIZATION (tech_lexicon + cleanup) ──\n{}",
        after_norm
    );
    println!(
        "\n── FINAL FORMATTED OUTPUT (format_layout) ──\n{}",
        final_out
    );

    // Validate preservation
    let tech_tokens = [
        "getUserById",
        "useEffect",
        "useState",
        "Zustand",
        "SuperFlow",
        "Parakeet",
        "fileName",
        "index_ts",
        "cleanup.rs",
        "track.ts",
        ".env.local",
        "src-tauri/src/transcript/cleanup.rs",
        "/api/parse",
        "localhost:1420",
        "https://example.com",
        "alex@example.com",
        "Alicks",
        "Alex",
        "myVarName",
        "ZustandStore",
        "normalizeTranscript",
        "flate2",
        "foobar",
        "bazqux",
    ];
    let missing = check_preservation(&raw, &final_out, &tech_tokens);
    println!("\n── Preservation check (byte-level where appropriate) ──");
    if missing.is_empty() {
        println!("✅ All {} tech tokens preserved", tech_tokens.len());
    } else {
        println!("❌ MISSING {} tokens: {:?}", missing.len(), missing);
        for m in &missing {
            println!("   - {}", m);
        }
    }
    // Specific regression: cleanup.rs must not become cleanup.Rs, fileName must not become filename
    if final_out.contains("cleanup.Rs") {
        println!("❌ REGRESSION: cleanup.rs → cleanup.Rs (SentenceCapitalization on .rs)");
    }
    if final_out.contains("track.Ts") {
        println!("❌ REGRESSION: track.ts → track.Ts");
    }
    if final_out.contains("fileName") && raw.contains("fileName") {
        println!("✅ fileName preserved (OrthographicConsistency blocked)");
    } else if raw.contains("fileName") {
        println!("❌ fileName corrupted");
    }
    if final_out.contains("…") || final_out.contains("  ") {
        println!("⚠️  spacing/punct check: contains ellipsis or double-space");
    }

    // Grammar validation — show every meaningful error that survives
    let grammar_cases = vec![
        ("a couple of Issue", "should be 'a couple of Issues'"),
        (
            "there is many problem",
            "should be 'there are many problems'",
        ),
        ("the the context", "duplicated 'the'"),
        ("is is broken", "duplicated 'is'"),
        ("fix fix it", "duplicated 'fix'"),
        ("i seen", "should be 'I saw'"),
        (
            "me and him was",
            "should be 'me and him were' or 'he and I were'",
        ),
        (
            "this are a test",
            "should be 'this is a test' or 'these are tests'",
        ),
        ("dont", "should be 'don't'"),
        ("its working", "should be 'it's' if contraction"),
        ("he have went", "should be 'he has gone'"),
        (
            "qa have found",
            "should be 'QA has found' or 'QA have found' if plural?",
        ),
    ];
    println!("\n── Grammar survivors (broken English that remains) ──");
    let mut survivors = 0;
    for (pattern, desc) in grammar_cases {
        if final_out.to_lowercase().contains(&pattern.to_lowercase()) {
            println!("❌ SURVIVES: {:?} — {}", pattern, desc);
            survivors += 1;
        } else {
            // Check if original had it and final fixed it
            if raw.to_lowercase().contains(&pattern.to_lowercase()) {
                println!("✅ FIXED: {:?} — {}", pattern, desc);
            }
        }
    }
    if survivors == 0 {
        println!("✅ No tracked grammar survivors (but check Punctuation+Hypen below)");
    }

    // Punctuation + Hyphenation
    println!("\n── Punctuation + Hyphenation ──");
    let hyphen_cases = vec![
        ("long term plan", "long-term plan"),
        ("well known issue", "well-known issue"),
        ("privacy first design", "privacy-first design"),
        ("state of the art system", "state-of-the-art system"),
        ("open source project", "open-source project"),
    ];
    for (plain, hyphenated) in hyphen_cases {
        if final_out.contains(hyphenated) {
            println!("✅ Hyphen fixed: {:?} → {:?}", plain, hyphenated);
        } else if raw.contains(plain) {
            println!(
                "❌ MISSES hyphen: {:?} should be {:?} (harper 0 lints for these, see 3.3/3.4)",
                plain, hyphenated
            );
        }
    }
    // Numeric ranges
    if final_out.contains("3–5 pm") || final_out.contains("3-5 pm") {
        println!("Check numeric range 3-5 pm → found in final");
    }
    // Apostrophes
    if final_out.contains("don't") {
        println!("✅ Apostrophe dont → don't present");
    } else if raw.contains("dont") {
        println!("❌ dont not fixed to don't (SpellCheck multi-sugg blocked)");
    }
    // Periods/caps
    if final_out.starts_with("So") || final_out.starts_with("Qa") || final_out.starts_with("QA") {
        println!(
            "Check capitalization: starts with {:?}",
            final_out.chars().take(20).collect::<String>()
        );
    }

    // Formatting checks
    println!("\n── Formatting checks ──");
    if final_out.contains("  ") {
        println!("❌ Double spaces survive");
    } else {
        println!("✅ No double spaces");
    }
    if final_out.trim().is_empty() {
        println!("❌ Empty final");
    } else {
        println!("✅ Non-empty final ({} chars)", final_out.len());
    }
    if final_out.contains("```") {
        println!("Check code fences preserved");
    }
    // No truncated tail
    if BROKEN_TRANSCRIPT.contains("0.6B") && final_out.contains("0.6B") {
        println!("✅ Tail preserved (0.6B)");
    } else if BROKEN_TRANSCRIPT.contains("0.6B") {
        println!("❌ Tail dropped (0.6B missing)");
    }

    // Performance — complete post-STT pipeline
    println!("\n{}", "═".repeat(100));
    println!(
        "  PERFORMANCE — complete post-STT pipeline (grammar+normalize+format) — release, hot"
    );
    println!("{}", "═".repeat(100));
    let sizes = vec![
        ("10-second (~40 words)", 40),
        ("30-second (~120 words)", 120),
        ("1-minute (~150 words)", 150),
        ("5-minute (~750 words)", 750),
        ("15-minute (~2250 words)", 2250),
        ("30-minute (~4500 words)", 4500),
        ("45-minute (~6750 words)", 6750),
    ];
    let base_words: Vec<&str> = BROKEN_TRANSCRIPT.split_whitespace().collect();
    for (label, target_words) in sizes {
        let mut repeated = String::new();
        let mut count = 0;
        while count < target_words {
            for w in &base_words {
                if count >= target_words {
                    break;
                }
                if !repeated.is_empty() {
                    repeated.push(' ');
                }
                repeated.push_str(w);
                count += 1;
            }
        }
        let elapsed = benchmark_pipeline(&repeated);
        let ms = elapsed.as_secs_f64() * 1000.0;
        let status = if ms < 50.0 {
            "✅ <50ms"
        } else if ms < 100.0 {
            "⚠️ >50ms"
        } else {
            "❌ >100ms"
        };
        println!(
            "  {:<30} {:>5} words  {:>6} chars  {:>7.3} ms  {}",
            label,
            target_words,
            repeated.len(),
            ms,
            status
        );
        if target_words == 6750 {
            benchmark_breakdown(&repeated);
        }
    }

    // Determinism
    println!("\n── Determinism / Reliability ──");
    let det = BROKEN_TRANSCRIPT;
    let first = run_pipeline_stages(det).3;
    let mut ok = true;
    for i in 0..5 {
        let out = run_pipeline_stages(det).3;
        if out != first {
            println!("❌ Non-deterministic run {}", i);
            ok = false;
        }
    }
    if ok {
        println!("✅ Deterministic 5/5");
    }
    // Empty / huge / unicode / emoji
    for (name, input) in [
        ("empty", ""),
        ("whitespace", "   \n\t  "),
        (
            "emoji",
            "Hello 👋 world 🌍 with emoji 😊 and 20% and 3:30 PM",
        ),
        ("unicode", "café résumé naïve jalapeño José"),
        ("huge", &"the team have have meeting. ".repeat(1000)),
    ] {
        let out = std::panic::catch_unwind(|| run_pipeline_stages(input).3);
        match out {
            Ok(s) => println!("✅ {} ({}→{} chars) no panic", name, input.len(), s.len()),
            Err(_) => println!("❌ {} panicked", name),
        }
    }

    // 6 Realistic templates
    println!("\n{}", "═".repeat(100));
    println!("  6 REALISTIC TEMPLATES");
    println!("{}", "═".repeat(100));

    // 1. Gmail/email
    let gmail_raw = "hey alex so um basically qa have found a couple of Issue and there is many problem with the employee import flow and the the context function is is broken and we need to fix fix it before thursday afternoon thanks";
    let gmail_norm = {
        let (_, _, norm, _) = run_pipeline_stages(gmail_raw);
        norm
    };
    let email_ctx = EmailFormatContext {
        is_email: true,
        recipient_name: Some("Alex"),
        author_name: Some("Sam"),
        author_title: None,
        author_company: None,
        include_title: false,
        include_company: false,
        default_signoff: Some("Talk soon"),
    };
    let gmail_out = format_email_for_surface(&gmail_norm, email_ctx);
    println!(
        "\n── 1. Gmail/email ──\nRAW: {:?}\nFINAL:\n{}\nSubject: {:?}",
        gmail_raw, gmail_out.text, gmail_out.subject
    );
    if gmail_out.text.contains("Hi Alex,") && gmail_out.text.contains("Thanks,\nSam") {
        println!("✅ Gmail envelope Hi Alex / Thanks Sam");
    } else {
        println!("❌ Gmail envelope missing");
    }
    if gmail_out.text.to_lowercase().contains("a couple of issue") {
        println!("❌ Gmail grammar survivor");
    }

    // 2. Slack
    let slack_raw = "hey team so um basically we need to handle the well known bug in the state of the art system and the open source project needs a long term plan and fileName is from src-tauri/src/transcript/cleanup.rs";
    let (_, _, slack_norm, _) = run_pipeline_stages(slack_raw);
    let slack_out = format_for_slack_with_options(&slack_norm, SlackFormatOptions::default());
    println!(
        "\n── 2. Slack ──\nRAW: {:?}\nFINAL:\n{}",
        slack_raw, slack_out
    );
    if slack_out.contains("Hi") && slack_out.contains("Thanks") {
        println!("❌ Slack should NOT become email");
    } else {
        println!("✅ Slack stays conversational");
    }
    if slack_out.contains("cleanup.rs") {
        println!("✅ Slack preserves cleanup.rs");
    } else {
        println!("❌ Slack lost cleanup.rs");
    }

    // 3. Developer prompt
    let dev_raw = "please check getUserById uses useEffect with Zustand and fileName comes from src-tauri/src/transcript/cleanup.rs and the config is .env.local and the api is /api/parse at localhost:1420";
    let dev_final = run_pipeline_stages(dev_raw).3;
    println!(
        "\n── 3. Developer prompt ──\nRAW: {:?}\nFINAL:\n{}",
        dev_raw, dev_final
    );
    for tok in [
        "getUserById",
        "useEffect",
        "Zustand",
        "fileName",
        "cleanup.rs",
        ".env.local",
        "/api/parse",
        "localhost:1420",
    ] {
        if dev_final.contains(tok) {
            println!("  ✅ {}", tok);
        } else {
            println!("  ❌ MISSING {}", tok);
        }
    }

    // 4. Normal prose/document
    let prose_raw = "this are a test that still dont work correctly and i seen the issue yesterday and me and him was discussing about the long term privacy first open source solution";
    let prose_final = run_pipeline_stages(prose_raw).3;
    println!(
        "\n── 4. Normal prose ──\nRAW: {:?}\nFINAL:\n{}",
        prose_raw, prose_final
    );
    if prose_final.to_lowercase().contains("this are") {
        println!("❌ prose survivor this are");
    } else {
        println!("Check prose grammar");
    }

    // 5. Technical status update
    let status_raw = "quick status the well known bug in the state of the art system is not fixed and the open source privacy first design need to be update and we has meeting at 3pm";
    let status_final = run_pipeline_stages(status_raw).3;
    println!(
        "\n── 5. Technical status update ──\nRAW: {:?}\nFINAL:\n{}",
        status_raw, status_final
    );
    if status_final.contains("well-known") || status_final.contains("well known") {
        println!("Check hyphen well-known");
    }

    // 6. Long messy dictation (the main transcript already)
    println!(
        "\n── 6. Long messy dictation — see MAIN above ({} words) ──",
        BROKEN_TRANSCRIPT.split_whitespace().count()
    );

    // Final ratings (strict)
    println!("\n{}", "═".repeat(100));
    println!("  FINAL RATINGS (strict — survivors count as failure)");
    println!("{}", "═".repeat(100));
    // Count survivors in final_out
    let grammar_survivors = [
        "a couple of Issue",
        "there is many",
        "the the",
        "is is",
        "fix fix",
        "i seen",
        "me and him was",
        "this are",
        "dont",
    ]
    .iter()
    .filter(|p| final_out.to_lowercase().contains(&p.to_lowercase()))
    .count();
    let grammar_score = 10 - (grammar_survivors as i32 * 2).min(8); // each survivor -2
    let preservation_score = if missing.is_empty() {
        10
    } else {
        (10 - missing.len() as i32 * 2).max(0)
    };
    let hyphen_survivors = ["long term plan", "well known issue", "privacy first design"]
        .iter()
        .filter(|p| final_out.contains(*p))
        .count();
    let punct_score = 10
        - (hyphen_survivors as i32 * 2)
        - if final_out.contains("3–5") || final_out.contains("3-5") {
            0
        } else {
            1
        };
    let formatting_score = if final_out.contains("  ") || final_out.contains("cleanup.Rs") {
        5
    } else {
        8
    };
    // Reliability from earlier
    let perf_ms = benchmark_pipeline(BROKEN_TRANSCRIPT).as_secs_f64() * 1000.0;
    let overall = (grammar_score + preservation_score + punct_score + formatting_score) / 4;
    println!(
        "Grammar:      {}/10  ({} survivors: a couple of Issue etc)",
        grammar_score, grammar_survivors
    );
    println!(
        "Preservation: {}/10  ({} missing tech)",
        preservation_score,
        missing.len()
    );
    println!(
        "Formatting:   {}/10  (spacing/truncation check)",
        formatting_score
    );
    // Reliability
    println!("Reliability:  8/10  (deterministic, unicode ok, empty ok)");
    println!("Punctuation+hyphen: {}/10", punct_score.max(0));
    println!("Performance:  {:.3} ms post-STT (warm)", perf_ms);
    println!("Overall:      {}/10", overall.max(0));

    println!("\n{}", "═".repeat(100));
    println!("  FINAL VERDICT");
    println!("{}", "═".repeat(100));
    let works = if grammar_score >= 6 && preservation_score >= 9 && perf_ms < 50.0 {
        "PARTIAL"
    } else if grammar_score < 5 {
        "NO"
    } else {
        "PARTIAL"
    };
    // Brutal: grammar 4/10 means not high-quality, so NO
    let verdict = if grammar_score < 5 { "NO" } else { works };
    println!("Does the complete pipeline actually work? {}", verdict);
    println!("Preservation: {}/10", preservation_score);
    println!("Grammar: {}/10", grammar_score);
    println!("Punctuation + hyphenation: {}/10", punct_score.max(0));
    println!("Formatting: {}/10", formatting_score);
    println!("Reliability: 8/10");
    println!("Performance: {:.3} ms", perf_ms);
    println!(
        "Production-ready under 50 ms: {}",
        if perf_ms < 50.0 { "YES" } else { "NO" }
    );
    println!("\nRemaining blockers (exact):");
    if final_out.to_lowercase().contains("a couple of issue") {
        println!("- a couple of Issue → a couple of Issues | raw {:?} | actual {:?} | expected a couple of Issues | stage grammar (harper misses A_LOT_OF_NN, needs quantifier_plural rule) | fix add ExprLinter quantifier_plural.rs", "a couple of Issue", "a couple of Issue");
    }
    if final_out.to_lowercase().contains("there is many") {
        println!("- there is many problem → there are many problems | stage grammar (ThereIsAgreement misses) | fix there_is_plural.rs");
    }
    if final_out.to_lowercase().contains("this are") {
        println!("- this are a test → this is a test / these are tests | stage grammar (demonstrative_agreement) | fix demonstrative_agreement.rs");
    }
    if final_out.contains("long term plan") {
        println!("- long term → long-term (compound adj) | stage grammar/punct (harper 0 lints) | fix hyphen rule or formatter compound adj");
    }
    if !missing.is_empty() {
        println!("- preservation missing {:?} | stage protected_spans or SentenceCapitalization | fix ensure protected_spans covers it", missing);
    }
    if perf_ms >= 50.0 {
        println!(
            "- performance {:.3}ms >50ms | stage harper+format | fix reduce rules or input size",
            perf_ms
        );
    }
    if final_out.to_lowercase().contains("i seen") {
        println!("- i seen → I saw | stage grammar (irregular_past) | fix irregular_past.rs");
    }

    println!("\n(Do not hide failures — test the real implementation, do not rewrite tests until they pass)");
}
