# SuperFlow — Main Features

> SuperFlow is a **free, open-source, fully offline** speech-to-text desktop app
> (macOS/Windows/Linux, built on Tauri) that has grown into a **local-first AI
> command surface**: dictate anywhere, auto-format into email and Slack, drive
> multi-agent coding teams, and clean up transcripts — **all on your machine,
> no cloud, no API key, no data leaving the device.**

Every feature below maps to a real module in this repo and to a real pain point.

---

## 1. Core: Offline Speech-to-Text (the privacy anchor)

**Code:** `src-tauri/src/managers/`, `src-tauri/src/mlx/`, `transcribe-cpp` / `transcribe-rs`

- Press a shortcut → speak → transcription is pasted straight into the focused text field.
- Local VAD (Silero) filters silence; local Whisper (Small/Medium/Turbo/Large) or Parakeet V3 (CPU, auto language detect) does the recognition.
- GPU acceleration when available; works on CPU-only hardware.

**Pain point it kills:** every mainstream dictation tool phones your voice to a
remote server. SuperFlow's entire reason to exist is _"your voice stays on your
computer"_ (`README.md`). No transcripts, audio, or context ever leave the device.

---

## 2. SuperFlow Commands & Remote Control

**Code:** `src-tauri/src/commands/`, `src-tauri/src/cli.rs`, single-instance plugin, Raycast extension

- CLI / remote-control flags on a running instance:
  - `superflow --toggle-transcription` — start/stop recording
  - `superflow --toggle-post-process` — record + post-process on/off
  - `superflow --cancel` — abort current operation
- Startup flags: `--start-hidden`, `--no-tray`, `--debug`.
- External triggers via Unix signals / hotkey daemons (Wayland-friendly).
- Raycast extension: start/stop, browse history, switch model/language, manage dictionary.

**Pain point it kills:** voice tools that only work from their own window. You
should be able to toggle dictation from any hotkey, any launcher, or a script.

---

## 3. Email Voice (Gmail) — dictate finished drafts

**Code:** `src-tauri/src/gmail_voice/` (`context.rs`, `generator.rs`, `action.rs`, `ax.rs`, `bridge.rs`, `grammar.rs`)

- Detects Gmail as the active surface, captures bounded context (compose box, thread), and **generates a finished email** from a spoken instruction.
- Respects safety states: `NotGmail`, `ContextUnreliable`, `SessionChanged` → refuses instead of misfiring.
- Outcomes: `Drafted` · `Sent` · `Cancelled` · `Failed` — driven through macOS Accessibility (AX), never by faking keystrokes blindly.
- Generation is **local**; no mail content is uploaded.

**Pain point it kills:** long emails are painful to type and dangerous to dictate
raw — ASR output pasted into Gmail is a wall of unpunctuated text. SuperFlow
turns speech into a coherent draft/send locally.

---

## 4. Slack Auto-Formatting (deterministic, local)

**Code:** `src-tauri/src/audio_toolkit/slack_formatting.rs`, `formatter.rs`, `normalization.rs`

This is a **layout/cleanup layer, not an LLM**. Pipeline:

```
ASR → spelling/tech-name normalization → punctuation → slack_formatting → paste into Slack
```

- Shared logic across **browser Slack and the native Slack.app** (`SlackSurface::Browser | MacOSNative | Unknown`).
- Converts "first… second… third…" → numbered lists; safe comma lists → bullets.
- Wraps obvious code/path/flag tokens in Slack inline-code backticks.
- Paragraph sizing (target 24 / max 36 words) and greeting splitting.
- No network, no DOM inspection, no synthetic key events — pure, deterministic, unit-testable.

**Pain point it kills:** dictated Slack messages arrive as one giant unreadable
block. Auto-formatting makes voice-to-Slack actually presentable without a
human rewrite pass.

---

## 5. Intelligence Awareness — context-conditioned composition

**Code:** `src-tauri/src/intelligence/` (`router.rs`, `prompt.rs`, `validation.rs`), `src-tauri/src/context/` (`classify.rs`, `types.rs`)

- Classifies the frontmost surface at record time into `Surface`: `Gmail | Slack | Terminal | Editor | Other` (by URL host, native bundle id, or title markers — conservative, never mis-routes).
- When you dictate inside an aware surface with the toggle on, the transcript is treated as an **instruction**, and the configured local intelligence turns it into **finished text using bounded context** (developer surfaces emit execution-ready agent prompts; Gmail/Slack emit finished prose).
- `compose_aware_reply` returns an `AwarenessOutcome` with validation; unbounded context is refused.

**Pain point it kills:** raw transcripts need a full rewrite before they're
usable. Awareness composes the right output _for the surface you're in_, locally.

---

## 6. Editor / Edit Mode Awareness

**Code:** `src-tauri/src/context/classify.rs` (`EDITOR_BUNDLE_PREFIXES`), `src-tauri/src/file_refs.rs`

- Recognizes code editors (VS Code, VSCodium, Cursor) and can inspect the focused text (capped; degrades to `None` under macOS Secure Input).
- Smart file references resolve terminal/editor paths so dictated commands and agent prompts point at the right file.
- Surfaces where Secure Input is active are handled gracefully (no crash, no false context).

**Pain point it kills:** dictating code or file edits into an editor usually
lands in the wrong place or as gibberish. Edit Mode makes the editor an aware,
inspectable surface.

---

## 7. Voice Terminal Orchestration (lightweight agent launch)

**Code:** `src-tauri/src/voice_terminal/` (`grammar.rs`, `tmux.rs`, `ghostty.rs`, `prompts.rs`)

- Speak a command like _"open four Claude Code terminals to fix the auth bug"_.
- A deterministic grammar (`AgentKind`: `Claude | Codex | OpenCode`, optional count + mission) parses it; otherwise normal dictation is never hijacked.
- Launches a **local tmux-backed agent team**: N worker panes (batched 8 per Ghostty tab), plus one **BRAIN supervisor** pane when a count was spoken. Each pane boots the agent CLI through the user's login shell; workers get the faithful mission, the brain gets the mission + worker roster.
- Mechanism ported from the proven Terminal-kit (`sp`) tmux surface: real tmux splits + buffer pastes instead of fragile Cmd+D keystrokes.

**Pain point it kills:** spinning up and briefing a multi-agent coding team by
hand is a manual, error-prone tab-dance. One spoken sentence stands the team up
locally.

---

## 8. AI Agent Orchestration — Sapphire (`sp`) control plane

**Code:** `Terminal-kit/` (crate `sapphire-agent-factory`, bin `sp`)

Sapphire is the **full** local control plane that runs coding agents (Claude, Codex, Qwen, Forge) as a supervised team instead of isolated tabs. Key capabilities:

- **Supervised multi-agent launch** — one supervisor + N workers in isolated PTYs; your terminal stays the control UI, the team grid opens in a second terminal.
- **Mission planning & decomposition** — deterministic keyword workstreams + live supervisor plan override (`BEGIN_SAPPHIRE_PLAN_JSON`), role-based activation of 15 enterprise role templates.
- **Real-time watchdog** — stall detection (3-rung escalation), zombie detection (`kill -0`), crash-loop detection with exponential backoff, auto-respawn hooks.
- **Sapphire Control Protocol** — `SAPPHIRE_STATUS` / `SAPPHIRE_MAIL` / `SAPPHIRE_ACK` / `SAPPHIRE_LEASE` directives parsed from terminal noise.
- **Engineering-team mail** — durable SQLite-routed inter-agent messages with nudge queue (non-destructive delivery), scavenge claim/release, idempotent ack.
- **File-ownership leases** — conflict detection downgrades the challenger to `Contradictory` and notifies the supervisor.
- **16-state lifecycle + Problems View** — `p`/`P` TUI tab filtering only workers needing attention.
- **Durable state under `.sp/`** — SQLite (10 tables) + transcripts; `sp status | sessions | resume | replay | watch | summary` make any run an audit trail.
- **100% local / private** — no network of its own, no API key; all state/transcripts/mail stay on disk.

**Pain points it kills:** operator-as-coordinator tax, no durable mission state, silent stalls, cross-agent file conflicts, blind "I'm done" claims, zombie sessions, crash loops, and cloud-billing/privacy drag.

---

## 9. Local Cleanup & AI Cleanup (post-processing without the cloud)

**Code:** `src-tauri/src/ai_cleanup/` (`client.rs`, `credentials.rs`, `prompt.rs`), `src-tauri/src/local_cleanup/` (`metrics.rs`)

- **Local cleanup:** deterministic, on-device transcript cleanup and metrics — no outbound calls.
- **AI cleanup:** optional local-model cleansing of transcripts (e.g. PII/formatting) using locally-stored credentials; never a remote API by default.

**Pain point it kills:** "smart" post-processing elsewhere means sending your
text to a third party. SuperFlow cleans up locally, so the privacy guarantee
holds end-to-end.

---

## 10. Privacy & Locality — the cross-cutting guarantee

Every feature above shares one hard invariant:

- **No cloud:** speech recognition, formatting, awareness composition, agent orchestration, and cleanup all run on-device.
- **No API key required** for the core product; agents launched by Sapphire are local CLIs.
- **No data leaves the machine:** SuperFlow state lives in its app-data dir; Sapphire state lives under `.sp/` (SQLite + transcripts + mail).

**Pain point it kills:** the category-wide fear that your voice, emails, chat
messages, and code context are being harvested. SuperFlow makes "private by
architecture" the default, not a setting.

---

## Feature → Pain-point matrix

| #   | Feature                      | Real pain point                                                 |
| --- | ---------------------------- | --------------------------------------------------------------- |
| 1   | Offline STT                  | Cloud dictation leaks your voice                                |
| 2   | Commands / remote control    | Voice tools trapped in their own window                         |
| 3   | Gmail Voice                  | Emails are slow to type, ugly to dictate                        |
| 4   | Slack auto-format            | Dictated Slack = unreadable wall of text                        |
| 5   | Intelligence Awareness       | Raw transcripts need a full rewrite                             |
| 6   | Editor / Edit Mode           | Dictated code/edits land wrong or as gibberish                  |
| 7   | Voice Terminal Orchestration | Manual multi-agent tab setup is error-prone                     |
| 8   | Sapphire (`sp`)              | No supervision, durability, or conflict control for agent teams |
| 9   | Local / AI Cleanup           | "Smart" cleanup means shipping text to a 3rd party              |
| 10  | Privacy & Locality           | Category-wide fear of data harvesting                           |

---

## One-line summary

> SuperFlow is the **local-first** way to talk to your computer: dictate anywhere,
> auto-format into Gmail and Slack, stand up supervised AI coding teams, and
> clean transcripts — **all on your machine, all private, none of it in the cloud.**
