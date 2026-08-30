# Plan: Deterministic Email Subject + Greeting Formatting

**Status:** Draft — awaiting approval  
**Scope:** `src-tauri/src/audio_toolkit/formatter.rs:972` `extract_email_subject` + `format_email_for_surface`  
**No frontend toggle** — backend always runs, every Parakeet transcription. Never on Gemini (`ai_cleanup`).

## Problem

Dictated email currently fails deterministically when user says:

* `Subject <subject> Hey Sarah, <body>` → subject not reliably moved to Subject field, greeting not canonicalized.
* `Hey Mike, <body> subject <subject>` → subject stays in body, not moved to top. Greeting may be "Hey Mike and whatever" etc.
* One-flow: `Subject X Hey Mike ... subject Y` in same utterance without `\n` — must dedup, put subject on top, remove from bottom, keep only canonical `Hi Mike,` in body.

Existing `extract_email_subject` handles Case A (subject first → greeting) `formatter.rs:1073`, Case B (own last line) `formatter.rs:1103`, Case C (final clause after `. ,`) `formatter.rs:1124`, but not mid-flow and not greeting-first + subject-later in same sentence.

## Goal

Deterministic, <5ms, no LLM:

```
Input: "Subject updated rollout timeline Hey Sarah, the email formatting doesn't work at all. Thanks"
→ Subject: "Updated rollout timeline"
→ Body: "Hi Sarah,\n\nThe email formatting doesn't work at all.\n\nThanks,\n[Author]"

Input: "Hey Mike, quick update subject is updated rollout timeline can you check"
→ Subject: "Updated rollout timeline"
→ Body: "Hi Mike,\n\nQuick update can you check\n\nTalk soon,\n[Author]"
```

Both orders produce identical top Subject field and canonical `Hi [Name],` greeting, subject removed from body.

## Solution (deterministic)

### 1. Subject marker

* Marker `subject` case-insensitive, trim `: , .` `formatter.rs:1000` `canonical_marker`.
* Optional `line` bigram: `subject line` → consume `line`.
* Subject text = next 1–12 words (`SUBJECT_MAX_WORDS` `formatter.rs:991`) until:
  * greeting starter (`hey/hi/hello/dear` `SUBJECT_BODY_STARTERS` `formatter.rs:995`) **or**
  * sentence terminator (`.!?`) **or**
  * 12-word cap.
* Clean via `clean_subject_text()` `formatter.rs:1019` (drop doubled `subject`, leading `is`, trailing punctuation, capitalize first).

### 2. Greeting canonicalization (already `extract_email_envelope` `formatter.rs:1585` + `canonicalize_email_greeting` `formatter.rs:1475`)

* Opener `hey/hi/hello/dear` + 1–2 name tokens `is_name_like` `formatter.rs:1580`, reject `everyone/team` `GREETING_REJECT_FIRST`.
* If `EmailFormatContext.recipient_name` present, replace ASR name exactly (no fuzzy) — `Hey Voytek → Hey Wojciech`.
* Output `Hi Mike,` always `recapitalize` + comma.

### 3. New combined extraction `extract_subject_and_greeting(text) -> (Option<subject>, Option<greeting>, body_without_both)`

* Scan for **all** `subject` marker positions (not just first/last). For each, try to extract subject span per rule 1. Score by: marker at start > marker at end > mid-flow; longer subject (2–12w) > 1w; not `prose_continuation` (`is/was/will/the` `formatter.rs:1139`).
* Pick best subject span. Remove exactly that word range from text (byte `Span`).
* On remaining text, run `extract_email_envelope` to get greeting. If found, remove it too and keep canonical greeting separately.
* If no greeting found but subject was at start and next words after subject removal start with greeting, handle one-flow: `subject X hey mike ...` → after removing `subject X`, the remaining starts `hey mike` → greeting extracted.
* If greeting was at start and subject was at tail/mid, removal yields `Hey Mike, body` with subject gone — then `format_email_for_surface` will prepend canonical greeting as before.
* Dedup: if both `subject` at start and duplicate `subject` at tail (user said twice), keep first, drop second (same marker scan).

### 4. `format_email_for_surface(text, ctx)`

```
parsed = extract_subject_and_greeting(text)
subject = parsed.subject
greeting = parsed.greeting or canonicalized from ctx
body = parsed.body
// then existing EmailFormatContext logic: greeting/body/closing/signature
text = format_layout_with_email(body, ctx with greeting)
```

Keeps existing `only_final_thanks_becomes_closing` and `default_signoff` logic.

### 5. Guards (fail-closed)

* Subject <1w or >12w → no extraction.
* Body after removal <3 words → no extraction (existing `formatter.rs:1094` check).
* `the subject is unclear` → first word after marker is `is` → `prose_continuation` → no extraction (already `formatter.rs:1139`).
* `hey team` → `GREETING_REJECT_FIRST` → not email (`is_email_message` `formatter.rs:1192`).
* Ambiguous mid-flow with no greeting and no clear subject → leave untouched.

## Files to Change

* `src-tauri/src/audio_toolkit/formatter.rs:972` — extend `extract_email_subject` or replace with `extract_subject_and_greeting`; add helper `find_best_subject_span`.
* `src-tauri/src/audio_toolkit/formatter.rs:1165` `format_email_for_surface` — call new extractor before `format_layout_with_email`.
* Tests `src-tauri/src/audio_toolkit/formatter.rs:2890` `email_format_tests` — add 8 cases:
  * `subject X hey sarah, body thanks` → subject X, Hi Sarah
  * `hey mike, body subject X` → subject X on top, Hi Mike
  * `hey mike, body subject X hey mike` dup → one subject
  * `subject line X: hey alex` → subject X
  * `the subject is unclear` → no subject
  * `hey team` → no email

## Performance

Current `extract_email_subject` <0.1ms. New scan is one extra linear pass + `token_spans` — target `<1ms` on 500w, total post-STT still `<50ms` (measured `ultra_brutal_validation` `13ms` warm).

## Verification

* `cargo test --lib formatter::email_format_tests -- --nocapture`
* `cargo run --bin ultra_brutal_validation --release` — Gmail template must show `Hi Sarah,` + `Subject: Updated rollout timeline` and `Slack` must not become email.
* Manual dictation: "Subject updated rollout timeline Hey Sarah, the email formatting doesn't work at all thanks" → check `FormattedEmail.subject` and `text`.

## Out of Scope

* No frontend toggle (`TechLexicon.tsx` unchanged).
* No Gemini path — `managers/transcription.rs:2204` `post_process_transcription_text` only (Parakeet), not `ai_cleanup`.
* No LLM, no network.
