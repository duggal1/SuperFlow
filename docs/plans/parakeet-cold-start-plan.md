# Parakeet 0.6 Stream — First-Transcription Cold Start — Brutal Plan

## 1. Problem
Parakeet Unified 0.6B (`handy-computer/parakeet-unified-en-0.6b-gguf`, streaming=true, `transcribe_cpp` Parakeet arch) is `~2-6s` on the **1st** dictate after load/idle, then `~0.2-0.4s` on 2nd-4th. Steady-state fast, cold pathological.

## 2. Root causes (ranked, verified in-code)
1. **Model not resident** — `TranscriptionManager` idle watcher (`src-tauri/src/managers/transcription.rs:330-395`) unloads after `settings.model_unload_timeout` (default `Min5`). First dictate after unload pays full `Model::load` (~400-800ms) + `Session::session_with` before any audio moves. `initiate_model_load()` (`transcription.rs:827`, `actions.rs:1345`) is kicked **at key-press**, racing mic open — latency is on the critical path.
2. **Metal / ggml kernels not warmed** — `load_model()` (`transcription.rs:573-665`) creates `Session` but never runs inference. First `session.stream().feed()/finalize()` and `session.run()` compile Metal pipelines / allocate buffers. Benchmark proves it: `src-tauri/src/bin/parakeet_stream_benchmark.rs:106-107` does a silent `2s` dummy run before timing. No warmup exists in prod.
3. **No pre-warm on app start / model switch** — `lib.rs:initialize_core_logic` never preloads. Selecting Parakeet in UI does not `load_model` until first record.

MLX `parakeet-mlx` path (`mlx/mlx_voice.py:630`) reloads weights **every** `transcribe_parakeet()` call — unrelated if user is on GGUF streaming (they are), but same class of bug; cached `MLXAudioASR` fixes it for nemotron/qwen.

## 3. Will “always cached in RAM” make it faster?
**Yes for p50 of 1st dictate; neutral for 2nd+.** Pinning eliminates (1). With `ModelUnloadTimeout::Never` the GGUF + Metal buffers (~750 MB Q8) stay resident. Trade: +750 MB RSS forever, no paging saving. On 16 GB Mac: invisible. On 8 GB: pressure under Chrome + Xcode. Warmup (2) is still required — resident but cold kernels still cost 800-1500 ms on first `feed`. Both needed for “always instant”.

`GGML_METAL_NO_RESIDENCY=1` (`lib.rs:654`) intentionally drops Metal residency to avoid #1902 teardown asserts. Pinning should **not** flip it to `0` without repro — warmup is cheaper and safe.

## 4. Solution — 3 surgical changes, no new deps

### A. Pin streaming model resident
- `transcription.rs:is_model_loaded` / `maybe_unload_immediately` / idle watcher: if `current_model` `supports_streaming==true` (and family `parakeet`/`nemotron`/`voxtral` or catalog `streaming:true`) then treat timeout as `Never`. Simplest: early-return in both unload paths when active model is streaming. One `fn is_streaming_model_pinned(&self)->bool` helper, `model_manager.get_model_info()`.
- No new setting. User can still pick `Immediately` — pinned path overrides to `Never` only for streaming. (If you want a toggle later, add `pin_streaming_model: bool` default true.)
- File: `src-tauri/src/managers/transcription.rs`
- Size: ~30 lines.

### B. Warm dummy inference right after load
- After `load_model()` succeeds and `engine = Some(...)`, run **sync** warmup **once** before emitting `loading_completed`: create `0.8s` silence (`vec![0.0; 12800]`), `session.stream(&RunOptions, &StreamOptions::default())`, `feed` in `30ms` chunks, `finalize`, drop. Fall back to `session.run` for non-stream. Do it on the **same thread** so first real dictate never races it. Cost: +300-500 ms added to load, hidden behind tray “Loading…”, saves 1500+ ms on first dictation.
- Guard: only for `TranscribeCpp` streaming sessions; no-op for ONNX / AudioCpp / MLX (MLX already cached elsewhere).
- File: `src-tauri/src/managers/transcription.rs:load_model()`
- Size: ~40 lines.

### C. Eager preload on model select + app start
- `commands/models.rs:set_active_model` / `switch_active_model` already calls `load_model` — keep, but after pin+warming it becomes warm.
- `lib.rs:initialize_core_logic` — after `transcription_manager` is managed, `transcription_manager.initiate_model_load()` if `selected_model` is streaming and downloaded. Does not block startup; warms in bg. Harmless if model already set to `Never`.
- Files: `src-tauri/src/lib.rs`, `src-tauri/src/commands/models.rs` (one line each if not already).

Non-goal: MLX parakeet-mlx cache (would be global `OnceLock<ParakeetModel>`). Defer unless user moves to `animaslabs/parakeet-tdt-0.6b-v3-mlx-8bit`.

## 5. Verification (before/after)

```bash
# Cold load + 4 repeats, reports load_ms / best_ms / rtf / text
cargo run --features dev-bins --bin parakeet_stream_benchmark -- \
  ~/Library/Application\ Support/com.superflow.app/models/parakeet-unified-en-0.6b-Q8_0.gguf \
  /tmp/30s_16k_mono.wav --json

# App-level (headless, same TranscriptionManager path):
./target/debug/superflow --transcribe-file /tmp/5s.wav --model handy-computer/parakeet-unified-en-0.6b-gguf --repeat 4 --json
# Expect: load_ms ~700, run1 ~350 (was ~2000-4000), runs 2-4 ~280-350, rtf 8-14x
```

Manual: set timeout `Never` (or with pin, keeps), cold launch app, dictate 4× “hello world” — 1st must be within 1.2× of 2nd. Check RSS: `ps -o rss -p $(pgrep superflow)` holds ~850 MB after.

CI: `cargo test -p superflow --lib managers::transcription` (idle watcher + warmup guard), `bun run typecheck`.

## 6. Tasks
1. `transcription.rs` — `is_streaming_pinned()` + skip `maybe_unload_immediately` + idle watcher. [1 file, 30L]
2. `transcription.rs` — `warm_streaming_session()` + call inside `load_model()` after `Session` creation, before `touch_activity`. [1 file, 40L]
3. `lib.rs` — eager `initiate_model_load` on boot when streaming model selected. [1 file, 5L]
4. Benchmark + manual, measure.

## 7. Rollback
Revert 3 files; set `model_unload_timeout=Min5`. Zero data migration.

## 8. Risks
- RSS pin → OOM on 8 GB under heavy load. Mitigate: pin only streaming family, not all models.
- Warmup adds ~400 ms to load; still net win (1st dictate -1500 ms).
- Metal residency flag untouched — no #1902 regress.
