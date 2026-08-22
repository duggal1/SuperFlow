# Fix: Result card (copyable transcript) only appears when dictation lands nowhere pasteable

## Root cause

The copyable transcript result card (`src/overlay/RecordingOverlay.tsx`, driven by the
`show-transcript-result` event) is **only** shown by the backend when `utils::paste()`
returns `Err` (`src-tauri/src/actions.rs:881-889`).

Today `paste()` (`src-tauri/src/clipboard.rs:765`) returns `Ok` in every real case: the
clipboard / direct / external-script methods inject keystrokes into the void when no
editable field is focused and never error out. So the paste "succeeds", the overlay is
hidden, and the result card is **never triggered** — exactly the reported "it is not
rendering at all" when the cursor is over a non-pasteable area.

The frontend card (transcript text + copy button + 10s auto-dismiss + feedback sound)
already exists and works once the event fires. This is a **backend-only** fix: detect
that there is no pasteable target and surface the existing card instead of pasting.

## Goal

- Cursor NOT in an editable input / terminal / text field → do NOT paste, show the
  existing copyable result card (with the `Stop` feedback sound).
- Cursor IS in a pasteable field → paste normally, never show the card.
- Keep it fast (single AX round-trip, zero added latency on the pasteable path) and
  reliable.

## Plan (macOS, always-on, no new setting)

### 1. New detection helper — `src-tauri/src/clipboard.rs` (macOS only)

Add `fn is_pasteable_target_focused() -> bool` mirroring the AX FFI already in
`src-tauri/src/context/focused_text.rs`:

- `AXUIElementCreateSystemWide()` → `AXFocusedUIElement`.
- Read `AXRole` of the focused element.
- Return `true` when the role is a pasteable/editable text role: `AXTextField`,
  `AXTextArea`, `AXComboBox`, `AXSearchField`, `AXScrollArea` (terminals), and
  `AXWebArea`/`AXTextArea` inside browsers.
- Return `false` on any AX error, missing element, or unrecognized role
  (this is the "cursor over random non-pasteable area" case).
- Also return `false` when `crate::secure_input::is_enabled_now()` is true — a password
  field is focused; we must NOT paste and must NOT surface the transcript (privacy).

Must run on the main thread (AX is main-thread bound — same constraint as
`context/capture.rs`). The paste closure in `actions.rs` already runs via
`run_on_main_thread`, so this is satisfied.

### 2. Gate the paste decision — `src-tauri/src/actions.rs` (the `Ok(final_text)` branch, ~line 857-898)

Inside the existing `run_on_main_thread` closure, before `utils::paste(...)`, branch on
`clipboard::is_pasteable_target_focused()`:

- **Pasteable** → call `utils::paste(...)` exactly as today. `Ok` → hide overlay + `Stop`
  sound (existing). `Err` (genuine paste failure) → existing `paste-error` toast + result
  card (existing).
- **Not pasteable (nowhere)** → do NOT call `utils::paste`. Instead:
  `play_feedback_sound(&ah_clone, SoundType::Stop)` then
  `utils::show_result_overlay(&ah_clone, final_text)` then
  `change_tray_icon(&ah_clone, TrayIconState::Idle)`. No `paste-error` toast.
- **Secure input active** → hide quietly: `utils::hide_recording_overlay` + idle tray
  icon. No paste, no card (privacy).

Keep `PasteMethod::None` unchanged (it already skips paste and hides; do not show the card).

### 3. Verify the frontend card renders (no code change expected)

Confirm `src/overlay/RecordingOverlay.tsx`:
- `show-transcript-result` listener sets `resultText` + `isVisible(true)` (lines 142-155).
- The `resultText` branch renders the `.sresult` card with the `Copy` button and
  `handleCopy` (lines 249-307).
- No secondary bug preventing the copy button from appearing. If a render bug exists,
  fix it; otherwise leave as-is (user: "we already have everything").

## Files to change

- `src-tauri/src/clipboard.rs` — add `is_pasteable_target_focused()` (+ `#[cfg(test)]`
  unit test: no focused app / unrecognized role → `false`).
- `src-tauri/src/actions.rs` — branch on the detection before `utils::paste`.

## Validation

1. `cargo build` (or `bun run build` is TS only — backend check via `cargo check` in
   `src-tauri`).
2. `bun run lint` and `cargo fmt -- --check`.
3. Manual (macOS, dev build `bun run dev`):
   - Transcribe with focus on the desktop / a non-editable window → result card appears
     with full transcript + working **Copy** button + sound; nothing is pasted.
   - Transcribe with focus in a text input / terminal → text is pasted; no card.
   - Transcribe while a password field is focused (Secure Input) → nothing pasted, no card.
   - `PasteMethod::None` → no paste, no card (unchanged).
4. Add/extend a Playwright or unit test covering the "nowhere" path if a harness exists
   in `tests/` / `playwright.config.ts`.

## Out of scope

- New user-facing setting/toggle (always-on per decision).
- Windows/Linux detection (macOS only for now; reliable-paste receipts are the future
  cross-platform signal but not required here).
- Changing the card's auto-dismiss timing or styling.
