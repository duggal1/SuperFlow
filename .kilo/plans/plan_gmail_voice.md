# Gmail Voice Reply / Draft / “Send It” — Implementation Plan

## 0. What already exists (reuse, don’t rebuild)

Investigated the real codebase before designing:

| Need | Existing home | Reuse |
|------|---------------|-------|
| App/Gmail detection | `context/types.rs` (`Surface::Gmail`), `context/capture.rs`, `context/browser.rs` (AX URL/title), `context/focused_text.rs` | Gmail surface + browser URL already classify today |
| Snapshot contract | `ContextSnapshot { surface, app_name, bundle_id, url, title, focused_text, captured_at_ms }` | Pass this into the Gmail workflow |
| Spoken-command pattern | `voice_terminal/mod.rs` + `voice_terminal/grammar.rs` | Mirror `try_handle_voice_command(transcription)->bool` and `grammar::parse` (deterministic tokenizer, not `contains()`) |
| LLM call | `llm_client::send_chat_completion_with_schema(provider, key, model, user, system, …)` (used by `intelligence/router.rs`) | Call Gemini with a **dedicated** Gmail prompt, not the generic awareness prompt |
| Compose-and-paste | `actions.rs:539-570` calls `compose_aware_reply` then pastes `final_text` | New Gmail command **pre-empts** this block and does its own insertion |
| Paste/input | `utils::paste`, `clipboard.rs`, `enigo` | Insert generated body with **auto-submit off** |
| AX read FFI | `context/browser.rs`, `context/focused_text.rs` (AXUIElement FFI, `AX_SUCCESS`, CFString helpers) | Reuse the same FFI style for context extraction + a new **AX press** helper |

Net: we add a **stateful Gmail command subsystem** and route it in `actions.rs` *before* the generic awareness block. Slack is untouched (per spec §25).

---

## 1. Architecture (matches spec §20, bottom-heavy)

```
Speech/Transcription finalize (actions.rs)
        ↓  try_handle_gmail_command(final_text, snapshot, settings) -> bool
GmailVoiceCommand parser (gmail_voice/grammar.rs)   ← deterministic, mirrors voice_terminal/grammar.rs
        ↓  only if Surface::Gmail AND compose/reply editor focused
GmailContextProvider (gmail_voice/context.rs)      ← NEW AX extraction (sender/subject/body/thread)
        ↓
GmailGenerator (gmail_voice/generator.rs)          ← NEW dedicated Gemini prompt
        ↓
GmailActionController (gmail_voice/action.rs)      ← NEW validate → paste(body) → optional send
        ↓
GmailSend (AX press on Send button, fallback verified Gmail Cmd+Enter)
```

Single new top-level module `src-tauri/src/gmail_voice/` with `mod.rs` orchestrator + `grammar.rs`, `context.rs`, `generator.rs`, `action.rs`, `session.rs`.

---

## 2. New files

### `gmail_voice/grammar.rs` — deterministic parser (pure, unit-tested)
Mirror `voice_terminal/grammar.rs` shape exactly: `normalize()` (lowercase, strip punctuation), alias tables, no `transcript.contains("send it")`.

```rust
pub enum GmailIntent { Reply, Compose }
pub enum TerminalAction { None, Send }
pub struct GmailVoiceCommand {
    pub intent: GmailIntent,
    pub instruction: String,        // hook phrase + "send it" stripped
    pub recipient_hint: Option<String>,
    pub terminal_action: TerminalAction,
}

// Category alias tables (spec §21), trivially extensible:
const REPLY_ALIASES: &[&str] = &["reply to this email","please reply to this email","reply to this","please reply"];
const COMPOSE_ALIASES: &[&str] = &["draft an email to","draft me an email to","write an email to","write me an email to","compose an email to"];
const SEND_ALIASES: &[&str] = &["send it","send it now","send this","send this email"];

pub fn parse(transcription: &str) -> Option<GmailVoiceCommand>
```
Rules:
- Detect **leading** reply/compose alias (longest alias wins). The alias text is **not** part of `instruction`.
- `instruction` = text after the alias, with any trailing `send` alias removed. Terminal `send` is only recognized as a **trailing, sentence-final** phrase (parse from the end, require it sits after the instruction and is not embedded mid-sentence). This satisfies spec §11 false-positive cases.
- No intent found → `None` → caller treats transcript as ordinary dictation (so "send it" outside a Gmail command is just text — spec §9).
- Unit tests covering spec §11 / §12 examples (e.g. `"Tell Alex I'll send it tomorrow."` → intent None; `"…Send it."` → Reply + Send).

### `gmail_voice/session.rs` — short-lived state (spec §13)
```rust
pub struct GmailSession {
    pub mode: GmailIntent,
    pub recipient_name: Option<String>,
    pub recipient_email: Option<String>,
    pub subject: Option<String>,
    pub source_message: Option<String>,   // email being replied to
    pub thread_context: Option<String>,
    pub generated_body: Option<String>,
    pub editor_verified: bool,
    pub active: bool,
    pub created_at_ms: u64,
}
```
Invalidate (set `active=false`, drop) on: Gmail loses focus, compose/reply closes, tab changes, thread changes, generation fails, body insert fails, message sent, cancelled, or **timeout** (e.g. 90s). Re-checked after every async step.

### `gmail_voice/context.rs` — `GmailContextProvider` (NEW AX extraction, spec §3-5)
Reuses the AX FFI declared in `context/browser.rs`/`focused_text.rs`. Walks the frontmost Gmail window:
- **Browser**: from `AXFocusedWindow` → find the message thread pane (role `AXGroup`/`AXStaticText` hierarchy) → read header (sender name + `Name <email>`), subject (page title or header), and the latest message body.
- Captures `sender` (display name + email parsed from `Name <addr>`), `subject`, `source_message`, optional `thread_context`, current `To`/`Cc`.
- **Hard fail** if minimum required (sender name, sender email, subject, source message) cannot be read deterministically — return `Err(ContextError::Unreliable)` so the workflow does **not** generate against fake context (spec §4, §23). No guessing, no OCR/screenshots unless unavoidable.
- For **Compose**, extracts the focused compose editor + (best-effort) the `To` field value as `recipient_hint`.

### `gmail_voice/generator.rs` — `GmailGenerator` (NEW dedicated prompt, spec §6-7)
Builds the exact structured prompt from spec §6:
```
MODE: reply|compose
RECIPIENT: <structured identity from Gmail, never from Gemini>
SUBJECT: ...
EMAIL_BEING_REPLIED_TO: <real extracted text>
OPTIONAL_THREAD_CONTEXT: ...
USER_INSTRUCTION: <command.instruction>
```
System prompt from spec §6 (write only the email body; preserve dates/times/names/commitments; never invent facts or change recipient; no markdown fences/metadata). Calls `llm_client::send_chat_completion_with_schema` with the user-configured post-process provider/key/model (same plumbing as `intelligence/router.rs`, but its **own** system prompt — not `intelligence/prompt.rs`). Returns only the email body; validated non-empty and stripped of fences.

### `gmail_voice/action.rs` — `GmailActionController` (spec §8, §14-15)
- `insert_body(session, body)`: focus the compose/reply editor, paste `body` via `utils::paste` with **auto-submit = false** (so it never sends). Re-validate editor is still the same Gmail editor afterward (spec §15).
- `execute_send(session)`: only if `session.active && context_valid && generated_valid && editor_found`. Priority (spec §14): (1) **AX press** the Gmail “Send” button located in the compose window by role/title; (2) fallback to the **verified Gmail shortcut Cmd+Enter** via `enigo` (Gmail-specific, NOT global Enter); (3) never a blind `Enter`.
- Add a small **AX press helper** (`ax_press_button_by_label`) in this module reusing the AX FFI from `context/browser.rs`. (No AX press helper exists today — this is the only new AX-write primitive.)

### `gmail_voice/mod.rs` — orchestrator
```rust
/// Returns true when the utterance was a Gmail command and was handled
/// (inserted/sent). Caller must then SKIP generic awareness + normal paste.
pub async fn try_handle_gmail_command(
    transcription: &str,
    snapshot: &ContextSnapshot,
    settings: &AppSettings,
) -> bool
```
Flow: `parse` → if `None` return false (ordinary dictation). If `Some(cmd)`:
1. Gate: `snapshot.surface == Surface::Gmail` AND a compose/reply editor is focused (AX check). Else return false (don’t hijack; spec §3).
2. `GmailContextProvider::capture()` on **main thread** (AX). On `Unreliable` → log + toast “couldn’t read the email”, return true (suppress paste, do nothing else — fail safe, spec §23).
3. Spawn async: build `GmailSession`, call `GmailGenerator`, then if `terminal_action == Send` → re-validate session (race protection) → `execute_send`.
4. Return **true** so `actions.rs` skips the generic awareness block and the downstream `utils::paste`.

---

## 3. Files to modify

- **`src-tauri/src/actions.rs`** (~line 500-594, inside `TranscribeAction::finalize`): insert Gmail handling **before** the `if settings.intelligence_awareness_enabled` block. If `try_handle_gmail_command` returns `true`, set a `gmail_handled` guard that (a) skips the generic `compose_aware_reply` Gmail/Slack branch and (b) skips the terminal `utils::paste` call later in the function. When it returns `false`, behavior is unchanged (existing awareness + paste preserved). This is the only integration seam and keeps Slack + non-command Gmail dictation working.
- **`src-tauri/src/settings.rs`**: add `experimental_gmail_voice_enabled: bool` (default `false`) + serde, mirroring `experimental_mlx_enabled`. *Decision: dedicated toggle (not reusing `intelligence_awareness_enabled`) so existing awareness users aren’t surprised and Gmail voice can be tested independently.*
- **`src-tauri/src/shortcut/mod.rs`**: add `change_experimental_gmail_voice_enabled_setting` (mirror `change_experimental_mlx_enabled_setting`).
- **`src-tauri/src/lib.rs`** + **`src-tauri/src/commands/`**: register the new setting command if a UI toggle needs it (reuse existing pattern).
- **`src-tauri/src/context/mod.rs`** or expose `pub(crate)` the AX FFI helpers from `browser.rs`/`focused_text.rs` so `gmail_voice/context.rs` + `action.rs` can reuse them without duplicating FFI declarations.
- **Frontend**:
  - `src/stores/settingsStore.ts` + settings UI (new small `GmailVoiceToggle` mirroring `MlxToggles.tsx`) gated under Advanced → Experimental.
  - Reuse the existing `toast` system to surface Gmail outcomes (“Reply drafted”, “Sent”, “Couldn’t read the email”, “Send skipped — context changed”).
  - `bindings.ts` regenerated via specta.

---

## 4. Phased delivery (spec §24)

- **Phase 0 — scaffold**: setting flag, `gmail_voice/` module skeleton, `grammar.rs` + unit tests, `session.rs`, AX helper exposure. No behavior change.
- **Phase 1 — Reply generation (no send)**: hook detection, Gmail/editor gate, context extraction, Gemini generation with dedicated prompt, insert via `utils::paste` (no submit), leave unsent. Tests: parse, Gmail detect, sender/subject/body extraction, prompt construction, generation, insertion.
- **Phase 2 — Compose generation (no send)**: compose hooks, recipient resolution (AX `To` prefill when confident; if ambiguous → do **not** send), compose editor detection, body insertion, leave unsent.
- **Phase 3 — Send command**: terminal-action detection, false-positive prevention (parse-based), full state validation, AX Send (fallback Gmail Cmd+Enter), session invalidation. Only after Phase 1+2 stable.
- **Phase 4 — Edge cases (spec §24)**: “send it” mid-sentence vs end, Gmail focused/unfocused, reply/compose/no-editor, tab switch during generation (invalidate), draft close during generation, recipient change during generation, ambiguous recipient, multiple tabs/windows, long/short/HTML/quoted/signature/CC threads, reply-all, draft-only vs send.

---

## 5. Safety guarantees (spec §9, §11, §15, §23)

- `send it` is **only** a command inside an active `GmailSession` created by a reply/compose hook. Otherwise it is ordinary text.
- Deterministic parsing (`grammar::parse`), never `contains("send it")`.
- Context must be **reliably** extracted or the action fails safe (no generation, no send, no invented recipient).
- Recipient identity comes from Gmail’s structured data; Gemini is forbidden from changing it.
- Race protection: session re-validated after Gemini generation and before send; if Gmail/tab/editor/thread changed, abort (no paste, no send).
- Send never issues a global `Enter`; it targets the Gmail Send button via AX or the verified Gmail-only Cmd+Enter.

---

## 6. Risks / open decisions

- **AX extraction reliability** is the riskiest piece (Gmail DOM differs across Chrome/Safari and new/classic UI). Mitigation: best-effort traversal + hard-fail; improve extractor across Phase 1→4; never guess.
- **Send execution** depends on locating the Send button per browser; AX-press primary, Gmail Cmd+Enter fallback. Will verify the shortcut is Gmail-scoped.
- **Setting gate**: proceeding with a dedicated `experimental_gmail_voice_enabled` toggle (recommended). Say the word if you’d rather reuse `intelligence_awareness_enabled`.
- Provider for generation: reusing the configured post-process provider via `llm_client` (consistent with `compose_aware_reply`) rather than hard-coding Gemini — the prompt is Gemini-shaped but the client stays provider-agnostic. Flag if you want Gemini hard-pinned.
