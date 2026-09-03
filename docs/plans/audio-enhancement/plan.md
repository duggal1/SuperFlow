# Audio Enhancement — Denoise + AGC for Dictation

## Goal

Make dictation robust in loud environments and with quiet speech, by inserting a
real-time enhancement stage between microphone capture and the VAD/ASR pipeline:

1. **Kill background noise** (coffee-shop chatter, office, TV, party/DJ as far as
   physically possible) before audio reaches VAD and the ASR model.
2. **Catch whispered speech** — denoised + normalized audio raises Silero VAD
   confidence and gives ASR a usable signal even at whisper level.
3. **Make the user's voice louder and crisper** — automatic gain control lifts
   far-from-laptop / quiet speech to a consistent loudness without clipping.

## Current pipeline (measured, not assumed)

`recorder.rs` consumer thread:

```
cpal callback (device rate, mono mixdown)
  → FrameResampler (device rate → 16 kHz, 30 ms = 480-sample frames)
  → handle_frame(): VAD policy (Silero + SmoothedVad) → emit
  → processed_samples → WAV → transcription
```

Today nothing touches the samples beyond channel mixdown and resampling. There
is no denoiser and no gain stage anywhere (`grep` for agc/denoise/gain confirms;
only the visualizer has a display-only gain constant).

## Design

### Stage order and why

```
raw chunk (device rate)
  → [ENHANCER]
      1. rubato resample device rate → 48 kHz        (rubato FftFixedIn, already a dep)
      2. RNNoise denoise, 480-sample frames @ 48 kHz (nnnoiseless, pure Rust, BSD-3)
      3. AGC: target RMS −20 dBFS, gain ∈ [1.0, +24 dB], slow smoothing, ±0.98 peak clamp
  → existing FrameResampler (48 kHz → 16 kHz, 30 ms frames)   // rebuilt per session
  → Silero VAD → emit (unchanged code path)
```

- Denoise runs **before** VAD on purpose: Silero sees clean, normalized audio, so
  whispered speech clears the 0.3 threshold and residual noise does not.
- The existing `FrameResampler` is simply constructed with an input rate of
  48 000 when the enhancer is active, so 48k→16k + framing is reused, not duplicated.
- Enhancement applies only inside `if recording` — always-on mic mode keeps
  zero idle cost. The visualizer keeps consuming raw audio.
- WAV history and crash-journal contain enhanced audio, so re-transcription and
  crash recovery benefit automatically.

### Noise suppression engine

`nnnoiseless` 0.5.2 (crates.io) — pure-Rust port of RNNoise:

- `DenoiseState::new()` → `process_frame(&mut out[480], &in[480])`.
- Input must be 48 kHz, f32 values in the 16-bit PCM range (±32767), so the
  enhancer scales ×32768 on the way in and ÷32768 on the way out.
- One-frame lookahead: discard the first output frame per session.
- No new C toolchain, no new build features; ~real-time factor on one core.

### AGC (whisper fix + loudness fix)

Per 10 ms denoised frame:

- `rms` of the frame; `gain = TARGET_RMS / max(rms, floor)`, clamped to
  `[1.0, MAX_GAIN]` (never attenuates; TARGET_RMS = 0.1 ≈ −20 dBFS;
  MAX_GAIN ≈ ×15.8 = +24 dB).
- Gain is smoothed (EMA) to avoid pumping; output hard-clamped at ±0.98.
- Whisper at −45 dBFS → boosted ~+20 dB into ASR's sweet spot; a normal voice
  close to the mic gets ~0 dB extra.

### Session lifecycle

- `Enhancer::new(in_rate)` / `reset()` / `finish()` mirror `FrameResampler`:
  reset recreates the denoiser state and clears buffers so no cross-recording
  leakage (same invariant the resampler tests already enforce).
- Enabled flag travels per-recording on `Cmd::Start`, so toggling the setting
  applies on the next dictation without reopening the mic stream.

## Changes

| File | Change |
| --- | --- |
| `src-tauri/Cargo.toml` | add `nnnoiseless = "0.5.2"` |
| `src-tauri/src/audio_toolkit/audio/enhance.rs` | **new** `Enhancer` + unit tests |
| `src-tauri/src/audio_toolkit/audio/mod.rs` | export `Enhancer` |
| `src-tauri/src/audio_toolkit/audio/recorder.rs` | `Cmd::Start` gains `enhance: bool`; consumer builds enhancer + 48k resampler on Start; all three push paths (in-flight chunk, stop-drain, finish) route through the enhancer when active |
| `src-tauri/src/managers/audio.rs` | pass `settings.audio_enhancement_enabled` into `rec.start(...)` |
| `src-tauri/src/settings.rs` | `audio_enhancement_enabled: bool` (default **true**), default fn, `Default` impl, settings JSON test fixture |
| `src-tauri/src/shortcut/mod.rs` | `change_audio_enhancement_setting` command |
| `src-tauri/src/lib.rs` | register command in `collect_commands!` |
| `src/bindings.ts` | regenerated via debug-run specta export |
| `src/stores/settingsStore.ts` | updater entry |
| `src/components/settings/AudioEnhancement.tsx` | **new** toggle (mirrors `VoiceActivityDetection.tsx`) |
| settings UI sections using VAD toggle | add the new toggle beside it |
| `src/i18n/locales/en/translation.json` | `settings.advanced.audioEnhancement.title/description` (other locales fall back to en, `fallbackLng: "en"`) |

## Verification

- `enhance.rs` unit tests (`cargo test`):
  - noise reduction: tone + broadband noise → noise-floor RMS drops ≥ 10 dB
    after enhancement, tone preserved;
  - AGC: −40 dBFS input emerges near target RMS; 0 dBFS input never clips;
  - scale safety: output ∈ [−1, 1], finite, length ≈ input length;
  - reset: no cross-session leakage (same style as resampler tests);
  - recorder wiring: `run_consumer` with enhancement on emits processed samples.
- `cd src-tauri && cargo test && cargo clippy`; `bun run typecheck`; `bun run lint`; `bun run format`.
- Manual: dictate in a noisy room and a quiet room; compare history WAVs.

## Honest limits

- RNNoise crushes broadband/babble/stationary noise (café, office, road) but a
  blaring DJ set with music is only *substantially* reduced, not eliminated —
  that is the physics of the model class. VAD gating + AGC absorb the rest.
- Adds ~10 ms pipeline latency (one 48 kHz denoise frame) — negligible for dictation.
