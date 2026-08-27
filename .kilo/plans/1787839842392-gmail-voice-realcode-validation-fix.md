# Gmail Voice Real-Context + Deterministic "Send It" — Validation + Fix Plan

## Verdict: NOT 1000% complete. Most of the plan is absent from current RealCode.

I re-read the actual current files (not prior claims). Only two changes from the last
attempt survived on disk: `audio.rs:1023` (500 ms context wait) and the `mod.rs:160`
editor-relax. **Everything else is still the original/brittle code.** Below is the proof
and the exact fix list.

---

## 1. Unified Gmail command routing — BROKEN
- `gmail_voice::handle` (`mod.rs:71-88`) is the **original strict version**: it does
  `grammar::parse(transcription)` on the RAW transcript and gates on
  `snapshot.surface != Surface::Gmail`. There is **no hook stripping** inside `handle`.
- `normalize_gmail_instruction` / `strip_voice_command_hook` are defined ONLY in
  `actions.rs:132` (local) and used once in the **transcribe-binding hook path**
  (`actions.rs:1514-1621`, guarded by `binding_id == "transcribe" && !post_process`).
- The **direct path** (`process_transcription_output_with_context` → `handle(transcription,...)`
  at `actions.rs:549`) and **hands-free** (`binding_id == "hands_free_transcribe"`, which
  skips the hook path entirely) both call `handle` with the raw "Hey SuperFlow …" text.
- Result: in hands-free and direct paths, `parse` fails on the hook prefix → `NotHandled`
  → transcript pasted verbatim. Only the transcribe binding with the hook works.

**Fix:** Move a single `pub(crate) fn strip_voice_command_hook` + `pub fn normalize_gmail_instruction`
into `gmail_voice/mod.rs`; call `normalize_gmail_instruction(transcription, &settings.voice_command_hook)`
as the first step of `handle`; have `actions.rs:132` delegate to it (one source of truth).
Keep `gmail_voice_hook_context` but it becomes redundant — fine.

## 2. Gmail detection + snapshot — PARTIAL
- 500 ms wait: **DONE** (`audio.rs:1023`).
- `classify` (`context/classify.rs:94`) is still strict: requires `mail.google.com`/`gmail.com`
  URL or a title ending exactly `- Gmail`/`– Gmail` for a *known* browser. Firefox and a few
  edge titles fall through to `Other`.
- `is_gmail_like()` helper: **ABSENT** (`types.rs` has no such method).
- `capture_live_target` (`ax.rs:180-250`) STILL does the strict re-check:
  - `ax.rs:184` `return Err("frontmost application changed")` — kills the command if an overlay/
    notification briefly takes focus between stop and agent run.
  - `ax.rs:196` `return Err("frontmost surface is not verified Gmail")` — re-classifies and
    discards the already-verified snapshot.

**Fix:** (a) add `Surface::is_gmail_like()` (returns `== Gmail`, extensible); (b) in
`capture_live_target`, do NOT error on frontmost mismatch — keep a `log::warn!` and proceed
against `expected_pid`; remove the `classify(...) != Gmail` hard error (the caller already
gated on the snapshot). Use `frontmost_tab(Some(expected_bundle_id), pid)` for url/title only
as a best-effort hint.

## 3. Real email context extraction — BRITTLE (multiple fail-closed)
- `ensure_message_body_editor` (`ax.rs`) requires `AXTextArea`/`AXTextEditor` + label containing
  `"message body"` — localized Gmail fails.
- `nearest_compose_container` requires a Send button ancestor within 18 parents — Gmail DOM
  reparent breaks it.
- These are inherent AX constraints; leave as-is (they are correct for English Gmail) but note
  they are the main remaining real-world failure surface.

## 4. Thread context — CODE LIVE, but starved by #8 below
- `extract_thread_context` (`ax.rs:520`) EXISTS and excludes chrome text + dupes + truncates
  8000 chars. `generator.rs:52` emits `OPTIONAL_THREAD_CONTEXT` when present.
- BUT `extract_reply_context` (`ax.rs:412-435`) calls `unique_subject(thread_nodes)?` and
  `latest_structured_sender(thread_nodes)?` — **both fail-closed**. If either errors, the whole
  capture returns `ContextUnreliable` and thread is never reached. So thread is effectively
  DEAD for any thread where subject/sender aren't perfectly exposed.

**Fix:** make `unique_subject` and `latest_structured_sender` tolerant (see #8). Thread then
flows end-to-end.

## 5. Contacts — DOES NOT EXIST
- No `contacts/` module, no `resolve_contact_email`, no `mod contacts` in `lib.rs`, no
  `NSContactsUsageDescription` in `Info.plist`. Grep confirms zero matches.
- `populate_session_context` (`mod.rs:269-276`) for Compose still only does
  `literal_recipient_email(recipient_hint)` → for `"Alex"` returns `None` → `can_send()` false
  → "Send It" refused.
- `draft an email to alex@company.com` works (literal). `draft an email to Alex` cannot send.

**Recommendation (smallest reliable):** do **not** add a separate Contacts database/entitlement.
Instead reuse existing AX: after generating, write the hint into the Gmail **To** field via
`populate_compose`, let Gmail resolve the name to a chip (`Alex <alex@company.com>`), then
**read back** the resolved email from `compose_recipient()` and store it as
`session.recipient_email`. This needs no TCC entitlement and uses code already present
(`set_compose_recipient`, `compose_recipient`). If read-back is empty, fall back to the literal
or refuse — never invent. (Native `CNContactStore`/`AddressBook` is the secondary, heavier
option requiring `com.apple.security.personal-information.addressbook` + permission prompt;
skip unless AX read-back proves insufficient.)

## 6. Deterministic "Send It" — PARTIAL (only `send it`)
- `SEND_ALIASES_TERMINAL` (`grammar.rs:105`) is back to `&["send it"]`. So `send it now`,
  `send this email`, `send this`, `send`, `please send this` are **not** terminal actions.
- `send it` (case-insensitive) + `and send it` conjunction boundary + false-positive guards
  (`…to send`, `…will send`, `…send it tomorrow`) ARE present and correct.
- Note conflict: a prior instruction demanded "only send it"; the full plan wants the richer
  set. **Decision needed** (see Open Questions) — but the fix list below restores the full set
  per the plan; if the product requirement is truly only `send it`, drop tasks 6b.

## 7. Send safety + fallback — WORKING
- `action.rs:48-94` `execute_send` already (a) AXPress on the Send button, (b) on
  `Send button` error falls back to Cmd+Enter via Enigo, guarded by `verify` + `can_send`.
  This is real code, not a comment. Keep.
- `PasteMethod::None`: `insert_body` (`action.rs:27-46`) uses `clipboard::paste_exact` (AX
  paste). If paste is disabled it becomes `InsertFailed` — acceptable (AX set could be a future
  enhancement, out of scope).

## 8. Subject + sender robustness — BROKEN (fail-closed)
- `unique_subject` (`ax.rs:456-479`): requires **exactly one** `AXHeading`; errors on 0 or 2.
- `latest_structured_sender` (`ax.rs:481-491`): requires `Name <email>`; bare `alex@co.com` or
  `From: alex@co.com` fails.

**Fix (both in `ax.rs`):**
- `unique_subject`: dedupe ignoring a leading `Re:` prefix; if still >1, prefer a `Re:` subject
  or the longest; if 0, fall back to `window_title_subject(window)` (strip ` - Gmail` and unread
  badge). Return `Option<String>`, caller `.ok_or_else(...)`.
- `latest_structured_sender`: after the structured `Name <email>` scan, add a fallback that
  finds any node exposing a single email (`extract_single_email`) and uses the text before `<`
  as display name. Keep the guard so arbitrary text is not treated as sender.

## 9. Existing editor content — UNSAFE RELAX
- `mod.rs:160-162` now only `log::warn!` and proceeds when the editor is non-empty. `insert_body`
  then pastes — if the editor had `Hello`/signature/quote, the paste **appends/corrupts** rather
  than the old safe `InsertFailed`. This is a blind overwrite/append, not safe.
- **Fix:** when non-empty, deterministically clear the editor (`set_string_value(editor, "")` via
  AX) before insert, OR rely on `paste_exact` replacing the selection — verify `paste_exact`
  semantics first. Log a `warn` with reason code. Do not destroy content silently without a log.

## 10. Observability — SILENT on NotHandled
- `handle` returns `NotHandled` with **no log**. The call sites (`actions.rs:549`, `:1546`)
  only log on `Failed`. So `surface != Gmail`, parse-fail, and race failures are invisible.
- **Fix:** add `log::warn!(target:"gmail_voice", …)` in `handle` for: non-Gmail surface, parse
  fail, and (already) the race warning in `capture_live_target`. Keep concise, reason-coded.

---

## Executable fix list (in order)
1. `gmail_voice/mod.rs`: add `strip_voice_command_hook` + `normalize_gmail_instruction`; strip
   hook as first step of `handle`; add `NotHandled` warn logs.
2. `actions.rs:132`: delegate to `crate::gmail_voice::strip_voice_command_hook` (one source).
3. `context/types.rs`: add `Surface::is_gmail_like()`.
4. `gmail_voice/ax.rs:180-250`: relax `capture_live_target` (warn + proceed on frontmost
   mismatch; drop strict `classify` error); add `window_title_subject`.
5. `gmail_voice/ax.rs`: tolerant `unique_subject` (Option + Re: dedupe + title fallback) and
   `latest_structured_sender` (bare-email fallback).
6. `gmail_voice/grammar.rs`: restore `SEND_ALIASES_TERMINAL` to
   `["send it now","send this email","send it","send this","send"]` + add tests; keep
   false-positive guards. (Skip if product req is only `send it` — see Open Q.)
7. `gmail_voice/mod.rs` + `ax.rs`/`context.rs`: implement To-field contact read-back for Compose
   (write hint → read resolved chip → `session.recipient_email`). No new contacts DB.
8. `gmail_voice/mod.rs:160` + `action.rs`: make editor handling deterministic (clear before
   insert) with a logged reason.
9. `cargo check --lib` + `cargo test --lib gmail_voice` + `actions` hook test green.

## Open questions
- Q1 (product req): is the terminal send set **only `send it`** or the **full plan set**?
  Recommendation: full set (restores `send it now`/`send this email`/`send this`/`send`).
- Q2: for #9, is clearing a non-empty editor acceptable, or should we refuse when the user
  already typed content? Recommendation: clear + log (voice command owns the body).

## Validation matrix (post-fix, must pass)
- H1 `Hey SuperFlow, reply … tell him I'll join at 10 AM` → Drafted, recipient=Alex, no send.
- H2 `… Tell him I'll send the files tomorrow. Send it.` → Sent.
- H3 `… and send it` (no period) → Sent.
- H4 `draft an email to Alex … and send it` → To resolved via chip → Sent.
- H5 `send it now!` (2nd utterance, DraftReady) → Sent.
- F1/F2 embedded `send it` → NotHandled (content preserved).
- Race: overlay takes focus during capture → still Drafted/Sent (no `frontmost application changed`).
