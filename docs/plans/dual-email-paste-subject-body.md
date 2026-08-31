# Plan: Dual Parallel Paste — Subject → Subject Field, Body → Body

**Status:** Draft — brutal, no bullshit
**Scope:** `src-tauri/src/actions.rs:1938` `paste_with_email_subject` + `src-tauri/src/gmail_voice/ax.rs:416` `set_compose_subject` + `src-tauri/src/audio_toolkit/formatter.rs:1055` subject extraction
**Goal:** Dictated `subject` **never** appears in body, always lands in Gmail `Subject` input, body lands in body — without relying on cursor being on Subject.

## Brutal Truth: Current Logic Is Broken

**Current `clipboard.rs:779` `paste_with_email_subject`:**

```rust
paste_exact(subject) // assumes focus is in Subject
sleep 150
send Tab // assumes Gmail Subject→Body order
sleep 150
paste(body)
```

_If focus is already in body (user clicked body, or Gmail opened compose with body focused), `paste_exact(subject)` types subject **into body** — that's your bug: "subject data in the body text area instead of the subject input box"._

- It also hallucinates when user says `Subject X Hey Mike ...` in one flow — `extract_email_subject` `formatter.rs:1055` correctly strips subject from body, but paste still needs Subject-field focus.\*

**Why it "works" when you say only `subject whatever`:** Gmail compose opens with Subject focused by default, so Tab path accidentally works for single-field dictation. Multi-field one-flow exposes the race.

## What "Dual Parallel" Actually Means

Not `Tab` trick. Two independent writes via Accessibility (AX), no cursor assumption:

```
FormattedEmail { subject: Some("Updated rollout timeline"), text: "Hi Mike,\n\nThe email..." }

if is_email && subject.is_some() && surface == Gmail {
  // 1. Find Gmail compose container via AX (already exists for gmail_voice)
  // 2. Find Subject field via AXRole AXTextField/AXComboBox label "subject" (ax.rs:458 find_compose_field)
  // 3. Set subject directly via AX set_string_value (ax.rs:416 set_compose_subject) — no focus needed
  // 4. Paste body via normal paste (or via AX body field if found)
} else {
  // Not Gmail or no subject or AX failed → fallback: paste body only (subject already stripped)
}
```

This is **exactly** what `gmail_voice` already does for AI compose: `gmail_voice/ax.rs:416` `set_compose_subject` + `find_compose_field` `458` with `AXRole` + `semantic_label` `subject`, polling + verification `element_text == subject`. It does **not** need cursor on subject.

## Difficulty: 6/10

- **Not 0-2 dead simple:** Need to wire `RecordingContext` (`context::capture::RecordingContext` `actions.rs:511` `process_transcription_output_with_context`) into paste path to get `compose_container`/`pid` for AX. Current dictation paste `actions.rs:1891` only has `AppHandle`, not AX container. Need to plumb `context` through to `paste_with_email_subject` or re-derive via `AXUIElementCreateApplication` + `AXFocusedWindow` + `descendants` search (as `gmail_voice/ax.rs:475` does for reply). Handle timing (Gmail subject chip async), permissions, multiple compose windows, non-Chrome browsers, not-Gmail surfaces.
- **Not 9-10 impossible:** The AX helpers already exist, tested, and handle `set_string_value` + read-back verification `ax.rs:420`. `paste_with_email_subject` can be replaced with `try_ax_dual_paste` → fallback to legacy Tab → fallback to body-only. No new Rust crate needed, no LLM.

**If we can't find Subject field (not Gmail, no AX permission, Outlook/Apple Mail):** fail-closed → paste body only (subject already removed from body, so no duplication). Never hallucinate subject into body.

## Plan — Extremely Clean, Deterministic

### 1. Extract (already done, keep)

`formatter.rs:1055` `extract_email_subject` Case D now handles `Subject X Hey Mike` and `Hey Mike ... subject X` one-flow (12w cap, `is` skip, `the subject is unclear` guard). `format_email_for_surface` returns `FormattedEmail{subject, text}` with subject stripped from `text`.

### 2. Route (already done)

`actions.rs:704` `email_subject_out` travels separately as `ProcessedTranscription.subject`. `is_email_message` `formatter.rs:1292` + `EmailFormatContext.is_email` ensures only email surface gets subject.

### 3. Paste — REPLACE `paste_with_email_subject` with dual AX

**File:** `src-tauri/src/clipboard.rs:779`

```rust
pub fn paste_with_email_subject(subject, body, app) -> Result<(), String> {
  // Try AX dual paste first (no cursor needed)
  if let Ok(()) = try_ax_subject_then_body(&subject, &body, &app) {
    return Ok(());
  }
  // Fallback 1: legacy Tab path (subject focused)
  paste_exact(subject.clone())?; sleep 150; send_tab()?; sleep 150; paste(body)
}

fn try_ax_subject_then_body(subject, body, app) -> Result<(), String> {
  // 1. Get Gmail compose container via context or AX search
  let container = find_gmail_compose_container()?; // reuse gmail_voice::ax::descendants + find_compose_field
  // 2. Set subject directly
  gmail_voice::ax::set_compose_subject(&container, subject)?; // ax.rs:416, verifies read-back
  // 3. Paste body via normal paste (focus is already in body after compose, or we set body via AX too)
  // Optionally: find body field (AXTextArea role "AXTextArea" label "message body") and set, but paste is more reliable for rich text
  std::thread::sleep(50);
  paste(body, app) // reuses reliable paste
}
```

_Reuse `gmail_voice/ax.rs:458` `find_compose_field` for `subject` and `to` (already handles `AXTextField`/`AXComboBox` label `subject`). Add `find_compose_body` similarly if needed, but body paste via `paste` is fine — Gmail body is already focused after subject set._

### 4. Context Plumbing

`actions.rs:1891` `run_on_main_thread` paste closure currently only has `final_text` + `email_subject`. Add `context: Option<RecordingContext>` (already captured at `process_transcription_output_with_context:511`) so `paste_with_email_subject` can call `find_gmail_compose_container` without re-querying AX. If `context` is `None` or `surface != Gmail`, skip AX and fallback.

### 5. Tests (deterministic, no Gmail needed)

- Unit: `formatter.rs:3184` `subject_midflow_after_greeting_is_extracted` already passes; add `paste_with_email_subject` mock test that asserts `subject` not in `body` after `format_email_for_surface`.
- Integration: `cargo test --lib formatter::email_format_tests` + manual: `Subject updated rollout timeline Hey Mike, the email formatting doesn't work Thanks` → verify `subject` field in Gmail UI (AX read-back `element_text == subject`).

### 6. Failure Modes (fail-closed, never hallucinate)

- `subject` empty after trim → `paste(body)` only.
- `find_compose_field` `None` → fallback Tab or body-only.
- `set_string_value` read-back mismatch → fallback Tab.
- Not Gmail surface → `paste(body)` only (subject already stripped, so no duplication).
- AX permission denied → fallback.

## Why Not Just Keep Tab?

Tab is `2/10` simple but `8/10` fragile — depends on focus, 150ms sleeps, Gmail DOM order changes, and user clicking body before paste. AX `set_string_value` is `6/10` but deterministic: writes directly to Subject field regardless of focus, verified by read-back.

**We absolutely can do without cursoring on subject — `gmail_voice` already does.** Difficulty **6/10**: moderate, not dead simple, not impossible. Ship it.

## Out of Scope

- No frontend toggle — backend always.
- No LLM — `LanguageTool` as oracle only.
- Not for non-Gmail clients — fallback to body-only.
