# Plan: Email signature & closing behavior (local, deterministic)

## Goal
Gmail/Outlook output must always end with a proper sign-off + identity pulled from the
already-stored local spec (`user_specification`), handling:
1. **Missing ending** → auto-append the configured default sign-off (`Talk soon`) + name.
2. **Sign-off spoken, no name** → append name (+ optional title/company).
3. **Multi-line signature** → `Name` then `Title, Company` per `include_*` flags.
4. **Informal / ASR variants** → `tanks`→`thanks`, `thx/ty/tks`→`thanks`; conservative list expansion.
5. **Smart duplicate reduction** → `cheers thanks` / `talk soon thanks` collapse to ONE sign-off; stray tokens stripped from body.
6. Capitalization/structure already handled by `canonical_signoff`; keep it.

Everything stays local; no network. The generic transcription formatter is untouched.

## Files
- `src-tauri/src/audio_toolkit/formatter.rs` — `EmailFormatContext`, `extract_email_closing`, `format_layout_with_email`, `EMAIL_SIGNOFFS`, tests.
- `src-tauri/src/actions.rs` — `parse_user_spec` + Gmail branch (~line 736) populates new fields.
- `src/components/settings/general/specifications/shared-tab.tsx` — default `signoff` → `"Talk soon"` (field already exists/editable).

## Step 1 — Extend `EmailFormatContext` (formatter.rs ~line 922)
Add fields (keep `#[derive(Debug, Clone, Copy, Default)]` — all `Option<&str>`/bool are Copy):
- `author_title: Option<&'a str>`
- `author_company: Option<&'a str>`
- `include_title: bool`
- `include_company: bool`
- `default_signoff: Option<&'a str>`  (used only when no sign-off is spoken)

## Step 2 — Signature builder (new `fn build_email_signature`)
```
name                                  -> "Harpreet Duggal"
+ include_title                       -> "Harpreet Duggal\nFounder"
+ include_title & include_company     -> "Harpreet Duggal\nFounder, Superflow"
+ include_company only                -> "Harpreet Duggal\nSuperflow"
```
Returns `Option<String>` (None when `author_name` is None).

## Step 3 — Use builder in `extract_email_closing` (~line 1035)
Replace the single `signature: context.author_name` with `build_email_signature(&context)`.
Backward compatible: existing tests have `include_*` default `false`, so signature stays one line.

## Step 4 — Trailing / duplicate sign-off stripping
When a sign-off is matched at the tail, scan *backwards* over the preceding few tokens
within the 6-token window; if they are also sign-off/alias tokens, extend `body` end
before them. `thanks cheers` → body ends before `thanks`, closing = `Cheers,`.

## Step 5 — Aliases + conservative expansion
- Alias map: `tanks|thx|ty|tks|tx` → `thanks`.
- Expand `EMAIL_SIGNOFFS` modestly: add `talk to you soon`, `all the best`, `warm regards`.
- Keep existing false-positive guards: 6-token tail window + `SIGNOFF_CONTINUATIONS`
  (`thanks for`, `thanks again`, …) so mid-body `thanks for` is never a closing.

## Step 6 — Default ending in `format_layout_with_email` (~line 2105)
After `closing = extract_email_closing(...)`:
- If `closing` is `None` **and** `context.is_email && context.default_signoff.is_some()`
  **and** `build_email_signature(&context).is_some()`:
  synthesize `closing = ParsedEmailClosing { body: working.clone(), closing: "{DefaultSignoff},", signature: build_email_signature(...) }`.
- When a real sign-off exists, `closing` is `Some` → we never synthesize → no double ending.
(The existing `if let Some(parsed) = closing { … push closing/signature }` block already appends it.)

## Step 7 — Wire spec in `actions.rs` Gmail branch (~line 736)
```
let signature = spec.email.signature_name.clone().or_else(|| spec.identity.full_name.clone());
let ctx = EmailFormatContext {
    is_email: true,
    recipient_name: None,
    author_name: signature.as_deref(),
    author_title: if spec.email.include_job_title { none_if_empty(spec.identity.job_title) } else { None },
    author_company: if spec.email.include_company { none_if_empty(spec.identity.company) } else { None },
    include_title: spec.email.include_job_title,
    include_company: spec.email.include_company,
    default_signoff: signature.as_deref().map(|_| spec.email.signoff.as_str()),
};
```
(`default_signoff` set only when a name exists, so we never invent an ending for an unknown author.)

## Step 8 — Frontend default
`shared-tab.tsx` `emptySpecification().email.signoff` → `"Talk soon"`. The editable
"Sign-off" field stays; `include_job_title`/`include_company` already default `true`.

## Step 9 — Tests (formatter.rs `#[cfg(test)]`)
Add:
- default ending, name only → ends `"Talk soon,\nHarpreet Duggal"`.
- default ending, title+company → `"Talk soon,\nHarpreet Duggal\nFounder, Superflow"`.
- `cheers thanks` → single `Cheers,` + name; body has no stray `thanks`.
- `tanks` → `Thanks,` + name.
Keep the existing 8 email tests green (they use `ctx()` with no `default_signoff`/flags → unchanged).

## Verification
- `cd src-tauri && cargo test` (formatter email tests)
- `bun run typecheck` and `bun run lint` (frontend)
