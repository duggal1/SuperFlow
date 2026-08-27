//! Optional native Apple-Silicon MLX ASR bridge.
//!
//! Additive and gated: nothing here touches the shipped transcribe-cpp / ONNX
//! engines. When `settings.experimental_mlx_enabled` is on, model descriptors
//! are seeded from [`MlxVariant`] and transcription shells out to
//! `mlx_voice.py` running inside a uv-managed venv (`shell.sh`). Weights come
//! from Hugging Face into the shared HF cache; Metal inference happens outside
//! this process, so a failure can never take the app down.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::managers::model::{MlxVariant, ModelInfo, ModelSource};

/// Report returned by the toggle-driven environment pre-warm. Surfaced on the
/// Advanced MLX card so enabling feels instant and verifiable.
#[derive(Serialize, Type)]
pub struct WarmReport {
    pub ok: bool,
    /// Wall-clock milliseconds the cold import + first Metal op took.
    pub duration_ms: u64,
    /// Human-readable detail (versions on success, fix-up hint on failure).
    pub detail: String,
}

/// Real readiness probe — the single source of truth for whether the MLX
/// runtime actually works. Spawns the venv python and genuinely imports
/// `mlx.core`, executes a real Metal matmul (`mx.eval`), and imports
/// `mlx_audio.stt`. Returns `(python_version, warm_seconds)` on success.
///
/// This is shared by `warm_runtime_blocking` (toggle pre-warm) and
/// `runtime_info_blocking` (models-page check) so both surfaces report the
/// exact same real status — no file-only heuristic, no mock.
fn probe_metal_readiness(rt: &MlxRuntime) -> Option<(String, f64)> {
    let mut cmd = Command::new(&rt.python);
    cmd.arg("-c").arg(
        "import sys,json,time;\
         t=time.time();\
         import mlx.core as mx;\
         x=mx.random.normal((256,256))@mx.random.normal((256,256));\
         mx.eval(x);\
         import mlx_audio.stt as _stt;\
         import parakeet_mlx as _parakeet;\
         print(json.dumps({'python':sys.version.split()[0],\
         'seconds':round(time.time()-t,3)}))",
    );

    let (stdout, stderr) = run_piped(cmd, UTIL_TIMEOUT).ok()?;
    #[derive(Deserialize)]
    struct W {
        python: String,
        seconds: f64,
    }
    match serde_json::from_str::<W>(stdout.trim()) {
        Ok(w) => Some((w.python, w.seconds)),
        Err(_) => {
            log::warn!(
                "MLX metal readiness probe produced no JSON: {}",
                truncate(stderr.trim(), 200)
            );
            None
        }
    }
}

fn ready_status(python: &str, seconds: f64) -> String {
    format!("Metal ready · Python {} · warm {:.1}s", python, seconds)
}

/// Pre-warms the Python runtime so the first real transcription pays no cold
/// start: imports mlx + mlx_audio dylibs into memory/page cache and executes
/// one small Metal op. Called automatically when the user enables the MLX
/// toggle (Advanced → Experimental → Apple MLX Engine).
pub fn warm_runtime_blocking() -> WarmReport {
    let started = Instant::now();

    let (rt, diag) = discover_runtime();
    let Some(rt) = rt else {
        return WarmReport {
            ok: false,
            duration_ms: 0,
            detail: format!(
                "runtime not found — {} · run: bash src-tauri/src/mlx/shell.sh",
                diag.message()
            ),
        };
    };

    match probe_metal_readiness(&rt) {
        Some((python, seconds)) => WarmReport {
            ok: true,
            duration_ms: started.elapsed().as_millis() as u64,
            detail: ready_status(&python, seconds),
        },
        None => WarmReport {
            ok: false,
            duration_ms: started.elapsed().as_millis() as u64,
            detail: "Metal readiness probe failed — re-run: bash src-tauri/src/mlx/shell.sh"
                .to_string(),
        },
    }
}

/// Longest we let one transcription subprocess run before killing it.
const TRANSCRIBE_TIMEOUT: Duration = Duration::from_secs(600);
/// Longest for environment/LLM probes and cleanup calls.
const UTIL_TIMEOUT: Duration = Duration::from_secs(120);

/// Resolved runtime layout: venv python + the mlx_voice.py script.
#[derive(Debug, Clone)]
pub struct MlxRuntime {
    pub python: PathBuf,
    pub script: PathBuf,
}

impl MlxRuntime {
    /// True only when both resolved components actually exist on disk.
    pub fn is_complete(&self) -> bool {
        self.python.exists() && self.script.exists()
    }
}

/// Why a runtime could not be located, used to produce precise, actionable
/// diagnostics instead of the old vague "python or mlx_voice.py missing".
#[derive(Debug, Default, Clone)]
pub struct RuntimeDiagnosis {
    pub python_found: bool,
    pub script_found: bool,
    /// Concrete paths that were probed, for operator debugging.
    pub python_candidates: Vec<String>,
    pub script_candidates: Vec<String>,
}

impl RuntimeDiagnosis {
    /// Single human-readable reason the runtime is not usable yet.
    pub fn message(&self) -> String {
        match (self.python_found, self.script_found) {
            (true, true) => "runtime ready".to_string(),
            (false, false) => {
                "runtime incomplete — MLX python (venv) and mlx_voice.py both missing".to_string()
            }
            (false, true) => "runtime incomplete — MLX python (venv) is missing".to_string(),
            (true, false) => "runtime incomplete — mlx_voice.py is missing".to_string(),
        }
    }
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Walk up from the directory containing `start`, testing each relative
/// candidate at every ancestor. Used to locate repo/bundle artifacts at
/// runtime without relying on `CARGO_MANIFEST_DIR` (which is a compile-time
/// env var and is never present in the running process).
fn search_up(start: &Path, candidates: &[&str]) -> Option<PathBuf> {
    let mut cur = start.parent().map(|p| p.to_path_buf());
    while let Some(dir) = cur {
        for rel in candidates {
            let candidate = dir.join(rel);
            if candidate.exists() {
                return Some(candidate);
            }
        }
        cur = dir.parent().map(|p| p.to_path_buf());
    }
    None
}

/// Find the venv python, returning the first existing candidate plus the full
/// probe list so diagnostics can show exactly what was looked at.
fn discover_python() -> (Option<PathBuf>, Vec<PathBuf>) {
    let mut probed = Vec::new();

    if let Some(p) = std::env::var_os("SUPERFLOW_MLX_PYTHON") {
        let p = PathBuf::from(p);
        probed.push(p.clone());
        if p.exists() {
            return (Some(p), probed);
        }
    }

    if let Some(h) = home() {
        for rel in ["mlx-voice/.venv/bin/python", "mlx-voice/.venv/bin/python3"] {
            let p = h.join(rel);
            probed.push(p.clone());
            if p.exists() {
                return (Some(p), probed);
            }
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(found) = search_up(
            &exe,
            &[
                "src-tauri/.venv/bin/python",
                ".venv/bin/python",
                "Resources/.venv/bin/python",
            ],
        ) {
            return (Some(found), probed);
        }
    }

    (None, probed)
}

/// Find mlx_voice.py, returning the first existing candidate plus the full
/// probe list so diagnostics can show exactly what was looked at.
fn discover_script() -> (Option<PathBuf>, Vec<PathBuf>) {
    let mut probed = Vec::new();

    if let Some(p) = std::env::var_os("SUPERFLOW_MLX_SCRIPT") {
        let p = PathBuf::from(p);
        probed.push(p.clone());
        if p.exists() {
            return (Some(p), probed);
        }
    }

    if let Some(h) = home() {
        let p = h.join("mlx-voice/mlx_voice.py");
        probed.push(p.clone());
        if p.exists() {
            return (Some(p), probed);
        }
    }

    // Dev checkout (target/<profile>/superflow) and bundled app
    // (Contents/MacOS/superflow) both resolve relative to the executable.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(found) = search_up(
            &exe,
            &[
                "src-tauri/src/mlx/mlx_voice.py",
                "src/mlx/mlx_voice.py",
                "Resources/mlx_voice.py",
            ],
        ) {
            return (Some(found), probed);
        }
    }

    (None, probed)
}

/// Resolve the runtime, verifying each component exists on disk. Returns the
/// runtime when both the venv python and mlx_voice.py are found, plus a
/// diagnosis describing exactly what was missing otherwise.
pub fn discover_runtime() -> (Option<MlxRuntime>, RuntimeDiagnosis) {
    let (python, python_candidates) = discover_python();
    let (script, script_candidates) = discover_script();

    let diagnosis = RuntimeDiagnosis {
        python_found: python.is_some(),
        script_found: script.is_some(),
        python_candidates: python_candidates
            .into_iter()
            .map(|p| p.display().to_string())
            .collect(),
        script_candidates: script_candidates
            .into_iter()
            .map(|p| p.display().to_string())
            .collect(),
    };

    match (python, script) {
        (Some(python), Some(script)) => (Some(MlxRuntime { python, script }), diagnosis),
        _ => (None, diagnosis),
    }
}

/// Locate the runtime, env-var overrides first, then the `shell.sh` install
/// layout (`$HOME/mlx-voice`), then the repo/bundle (resolved at runtime from
/// the executable's location). Returns `None` unless both components exist.
pub fn find_runtime() -> Option<MlxRuntime> {
    discover_runtime().0
}

pub fn runtime_available() -> bool {
    find_runtime().is_some()
}

/// Weights on disk? `snapshot_download` layout:
/// `~/.cache/huggingface/hub/models--{org}--{repo}/snapshots/<sha>/...`
fn weights_dir(variant: MlxVariant) -> Option<PathBuf> {
    let repo = variant.hf_repo_id();
    let (org, name) = repo.split_once('/')?;
    let hub = home()?.join(".cache/huggingface/hub");
    let snapshots = hub
        .join(format!("models--{}--{}", org, name))
        .join("snapshots");

    let entries = std::fs::read_dir(&snapshots).ok()?;
    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .max_by_key(|entry| {
            entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        })
        .filter(|entry| {
            // A snapshot only counts once it actually holds tensors.
            std::fs::read_dir(entry.path())
                .map(|files| {
                    files
                        .flatten()
                        .any(|f| f.file_name().to_string_lossy().ends_with(".safetensors"))
                })
                .unwrap_or(false)
        })
        .map(|entry| entry.path())
}

pub fn weights_downloaded(variant: MlxVariant) -> bool {
    weights_dir(variant).is_some()
}

fn pretty_name(variant: MlxVariant) -> String {
    match variant {
        MlxVariant::Qwen17B8Bit => "Qwen3-ASR 1.7B (MLX 8-bit)".to_string(),
        MlxVariant::Qwen06B8Bit => "Qwen3-ASR 0.6B (MLX 8-bit)".to_string(),
        MlxVariant::ParakeetUnified => "Parakeet TDT 0.6B v3 (MLX INT8)".to_string(),
        MlxVariant::Nemotron => "Nemotron 3.5 ASR Streaming 0.6B (MLX 8-bit)".to_string(),
        MlxVariant::Cohere => "Cohere Transcribe 03-2026 (MLX 8-bit)".to_string(),
        MlxVariant::Ark06B => "ARK ASR 0.6B (MLX)".to_string(),
    }
}

fn accuracy_score(variant: MlxVariant) -> f32 {
    match variant {
        MlxVariant::Qwen17B8Bit => 0.96,
        MlxVariant::Qwen06B8Bit => 0.93,
        // Sonic Speech reports 0.82% LibriSpeech WER for this exact INT8
        // checkpoint. The card rounds 1 - WER to the nearest whole percent.
        MlxVariant::ParakeetUnified => 0.9918,
        MlxVariant::Nemotron => 0.94,
        MlxVariant::Cohere => 0.92,
        MlxVariant::Ark06B => 0.93,
    }
}

/// Relative speed score (0–1, higher = faster) derived from each model's real
/// parameter count / architecture — bigger checkpoints decode slower.
fn speed_score(variant: MlxVariant) -> f32 {
    match variant {
        MlxVariant::Qwen17B8Bit => 0.60,
        MlxVariant::Qwen06B8Bit => 0.85,
        MlxVariant::ParakeetUnified => 0.90,
        MlxVariant::Nemotron => 0.85,
        MlxVariant::Cohere => 0.70,
        MlxVariant::Ark06B => 0.80,
    }
}

/// Descriptor shown on the model page. The primary tensor file is downloaded
/// through the app's resumable Hugging Face path; the Python loader fetches the
/// repository's small configuration/tokenizer files on first load.
pub fn make_model_info(variant: MlxVariant) -> ModelInfo {
    ModelInfo {
        id: variant.id(),
        name: pretty_name(variant),
        description: variant.blurb().to_string(),
        filename: "model.safetensors".to_string(),
        source: ModelSource::HuggingFace {
            repo_id: variant.hf_repo_id().to_string(),
            revision: "main".to_string(),
        },
        size_mb: variant.approximate_size_mb(),
        is_downloaded: weights_downloaded(variant),
        is_downloading: false,
        partial_size: 0,
        is_directory: true,
        engine_type: crate::managers::model::EngineType::Mlx(variant),
        accuracy_score: accuracy_score(variant),
        speed_score: speed_score(variant),
        supports_translation: false,
        is_recommended: false,
        supported_languages: variant.supported_languages(),
        supports_language_selection: false,
        is_custom: false,
        supports_streaming: variant.supports_streaming(),
        supports_language_detection: false,
    }
}

// ---------------------------------------------------------------------------
// Subprocess I/O
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct MlxJsonOut {
    text: String,
}

fn truncate(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

fn drain<R: std::io::Read>(pipe: Option<R>) -> String {
    pipe.map(|mut p| {
        let mut buf = String::new();
        let _ = p.read_to_string(&mut buf);
        buf
    })
    .unwrap_or_default()
}

/// Run `cmd` with piped stdout/stderr, enforcing a hard wall-clock timeout.
/// Returns (stdout, stderr) on exit-status success.
///
/// Both pipes are drained on worker threads WHILE we wait on the child.
/// Draining only after exit deadlocks against chatty children: mlx-audio
/// writes tqdm progress to stderr, and once the OS pipe buffer (~64 KB) fills,
/// the child blocks forever and so does our wait loop.
fn run_piped(mut cmd: Command, timeout: Duration) -> Result<(String, String)> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().context("failed to spawn MLX subprocess")?;

    let out_pipe = child.stdout.take();
    let err_pipe = child.stderr.take();
    let out_thread = std::thread::spawn(move || drain(out_pipe));
    let err_thread = std::thread::spawn(move || drain(err_pipe));

    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let mut exited: Option<std::process::ExitStatus> = None;
    loop {
        match child.try_wait()? {
            Some(status) => {
                exited = Some(status);
                break;
            }
            None => {
                if Instant::now() > deadline {
                    let _ = child.kill();
                    exited = child.wait().ok();
                    timed_out = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(150));
            }
        }
    }

    // Pipes close when the child dies (or is killed), so these return promptly.
    let out = out_thread.join().unwrap_or_default();
    let err = err_thread.join().unwrap_or_default();

    if timed_out {
        anyhow::bail!("MLX subprocess timed out after {}s", timeout.as_secs());
    }
    match exited {
        Some(status) if status.success() => Ok((out, err)),
        Some(status) => Err(anyhow!(
            "MLX subprocess failed ({}); stderr: {}",
            status,
            truncate(&err, 2000)
        )),
        None => Err(anyhow!(
            "MLX subprocess produced no exit status; stderr: {}",
            truncate(&err, 2000)
        )),
    }
}

fn parse_json_out(stdout: &str) -> Result<String> {
    // mlx_voice.py prints exactly one JSON object per --json invocation.
    let value: MlxJsonOut = serde_json::from_str(stdout.trim_start())
        .with_context(|| format!("bad MLX JSON output: {}", truncate(stdout, 400)))?;
    Ok(value.text)
}

/// Transcribe one 16 kHz mono WAV file with the given MLX model alias.
/// Optionally polishes the raw transcript with the local mlx-lm cleanup LLM.
pub fn transcribe_wav(
    wav_path: &Path,
    variant_alias: &str,
    language: Option<&str>,
    with_cleanup: bool,
) -> Result<String> {
    let (rt, diag) = discover_runtime();
    let rt = rt.ok_or_else(|| {
        anyhow!(
            "MLX runtime not found — {}. Run:\n    bash src-tauri/src/mlx/shell.sh\nthen restart SuperFlow.",
            diag.message()
        )
    })?;

    let mut args = vec![
        rt.script.to_string_lossy().to_string(),
        "--json".to_string(),
        "transcribe".to_string(),
        wav_path.to_string_lossy().to_string(),
        "--model".to_string(),
        variant_alias.to_string(),
    ];
    if let Some(lang) = language {
        args.push("--language".to_string());
        args.push(lang.to_string());
    }

    let mut cmd = Command::new(&rt.python);
    cmd.args(&args);
    let (stdout, stderr) = run_piped(cmd, TRANSCRIBE_TIMEOUT)?;
    if !stderr.trim().is_empty() {
        log::debug!("mlx_voice stderr: {}", truncate(&stderr, 1000));
    }
    let raw = parse_json_out(&stdout)?;

    if !with_cleanup || raw.is_empty() {
        return Ok(raw);
    }

    // Local LLM polish via mlx-lm; any failure falls back to the raw text.
    let mut clean_cmd = Command::new(&rt.python);
    clean_cmd.arg("--json");
    clean_cmd.arg("clean");
    clean_cmd.arg(&raw);
    match run_piped(clean_cmd, UTIL_TIMEOUT) {
        Ok((out, _)) => match parse_json_out(&out) {
            Ok(cleaned) => Ok(cleaned),
            Err(e) => {
                log::warn!("MLX cleanup unavailable, returning raw transcript: {e:#}");
                Ok(raw)
            }
        },
        Err(e) => {
            log::warn!("MLX cleanup failed, returning raw transcript: {e:#}");
            Ok(raw)
        }
    }
}

// ---------------------------------------------------------------------------
// Diagnostics consumed by the model page
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Type)]
struct PyProbe {
    python_version: String,
    mlx_version: String,
    mlx_audio_version: String,
}

#[derive(Serialize, Type)]
pub struct MlxRuntimeInfo {
    pub available: bool,
    pub status: String,
    pub python_path: Option<String>,
    pub script_path: Option<String>,
    pub python_candidates: Vec<String>,
    pub script_candidates: Vec<String>,
    pub instructions: &'static str,
    pub probe: Option<PyProbe>,
}

fn probe_python(rt: &MlxRuntime) -> Option<PyProbe> {
    let mut cmd = Command::new(&rt.python);
    cmd.arg("-c").arg(
        "import json,sys;import importlib.metadata as m;\
         print(json.dumps({'python_version':sys.version.split()[0],\
         'mlx_version':m.version('mlx'),'mlx_audio_version':m.version('mlx-audio')}))",
    );
    let (stdout, _) = run_piped(cmd, UTIL_TIMEOUT).ok()?;
    serde_json::from_str(stdout.trim()).ok()
}

/// Blocking diagnostics used by the `get_mlx_runtime_info` command.
///
/// `available` and `status` reflect the *real* Metal readiness probe result
/// (same probe the toggle warm-up uses) — not a file-existence heuristic — so
/// the models page and the experimental toggle always agree.
// ---------------------------------------------------------------------------
// Live incremental streaming (Rust ↔ Python JSONL bridge)
// ---------------------------------------------------------------------------

/// One live hypothesis emitted by the Python `live` subprocess.
#[derive(Debug, Clone, Deserialize)]
pub struct MlxLiveEvent {
    pub committed: String,
    #[serde(default)]
    pub tentative: String,
    #[serde(default)]
    pub is_final: bool,
}

/// Handle to a long-lived `mlx_voice.py live` child. Wraps stdin/stdout and
/// the stderr drain thread. Used by `TranscriptionManager::run_stream_worker`
/// for MLX live overlay.
pub struct MlxLiveSession {
    child: Child,
    stdin: Option<ChildStdin>,
    // Reader thread join handle + channel receiver for parsed events.
    reader_handle: Option<thread::JoinHandle<()>>,
    events: mpsc::Receiver<MlxLiveEvent>,
}

impl MlxLiveSession {
    /// Spawn `python mlx_voice.py live --model <alias> [--language <code>]`.
    /// Model is loaded before `ready` is emitted; we wait briefly for that line
    /// so a broken model fails fast instead of hanging a live overlay.
    pub fn spawn(variant_alias: &str, language: Option<&str>) -> Result<Self> {
        let (rt, diag) = discover_runtime();
        let rt = rt.ok_or_else(|| {
            anyhow!(
                "MLX runtime not found — {}. Run:\n    bash src-tauri/src/mlx/shell.sh\nthen restart SuperFlow.",
                diag.message()
            )
        })?;

        let mut cmd = Command::new(&rt.python);
        cmd.arg(rt.script.to_string_lossy().to_string())
            .arg("live")
            .arg("--model")
            .arg(variant_alias);
        if let Some(lang) = language {
            if !lang.is_empty() && lang != "auto" {
                cmd.arg("--language").arg(lang);
            }
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().context("failed to spawn MLX live subprocess")?;
        let stdin = child.stdin.take().context("MLX live child missing stdin")?;
        let stdout = child
            .stdout
            .take()
            .context("MLX live child missing stdout")?;
        let stderr = child.stderr.take();

        // Drain stderr to avoid pipe stall (mlx-audio tqdm, load logs).
        if let Some(pipe) = stderr {
            thread::spawn(move || {
                let mut reader = BufReader::new(pipe);
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok() && !line.is_empty() {
                    if !line.trim().is_empty() {
                        log::debug!("mlx live stderr: {}", truncate(line.trim(), 500));
                    }
                    line.clear();
                }
            });
        }

        // Wait for ready signal synchronously before returning — ensures
        // model is resident and stdin loop is reading before we start feeding.
        // Block up to 90s (large model load); worker thread can afford it.
        let mut stdout_reader = BufReader::new(stdout);
        let mut ready_line = String::new();
        let ready_deadline = Instant::now() + Duration::from_secs(90);
        let ready_ok = loop {
            if Instant::now() > ready_deadline {
                log::warn!("MLX live spawn timed out waiting for ready");
                break false;
            }
            ready_line.clear();
            // Use read_line with timeout via polling: set stdout non-blocking? Simpler block.
            // Spawn a helper thread to read with timeout, but for MVP block.
            match stdout_reader.read_line(&mut ready_line) {
                Ok(0) => {
                    // EOF — child died during load
                    log::warn!("MLX live child exited before ready");
                    break false;
                }
                Ok(_) => {
                    let trimmed = ready_line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if trimmed.contains("\"ready\"") {
                        log::debug!("mlx live ready: {}", truncate(trimmed, 200));
                        break true;
                    }
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
                        if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
                            // Model load error — fail fast so caller falls back to batch
                            let _ = child.kill();
                            let _ = child.wait();
                            anyhow::bail!("MLX live failed to start: {}", err);
                        }
                    }
                    // Not ready yet (should not happen), keep waiting
                    log::debug!(
                        "mlx live unexpected line before ready: {}",
                        truncate(trimmed, 200)
                    );
                    break true;
                }
                Err(e) => {
                    log::warn!("MLX live ready read failed: {e}");
                    break false;
                }
            }
        };
        if !ready_ok {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("MLX live did not become ready");
        }

        let (tx, rx) = mpsc::channel::<MlxLiveEvent>();
        let reader_handle = thread::spawn(move || {
            for line in stdout_reader.lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(_) => break,
                };
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                // Ignore any stray ready (should already be consumed)
                if trimmed.contains("\"ready\"") {
                    log::debug!("mlx live ready (late): {}", truncate(trimmed, 200));
                    continue;
                }
                match serde_json::from_str::<MlxLiveEvent>(trimmed) {
                    Ok(ev) => {
                        let is_final = ev.is_final;
                        if tx.send(ev).is_err() {
                            break;
                        }
                        if is_final {
                            break;
                        }
                    }
                    Err(_) => {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
                            if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
                                log::warn!("MLX live error: {}", err);
                            }
                        }
                    }
                }
            }
        });

        Ok(Self {
            child,
            stdin: Some(stdin),
            reader_handle: Some(reader_handle),
            events: rx,
        })
    }

    /// Forward one PCM frame (16kHz float32) to the Python bridge. Encoding
    /// is a JSON array of floats (no extra crates, ~3-4KB per 512-sample frame).
    pub fn feed(&mut self, pcm: &[f32]) -> Result<()> {
        // Direct JSON array encoding avoids base64 crate; Python accepts both.
        let msg = serde_json::json!({ "type": "feed", "samples": pcm });
        let line = serde_json::to_string(&msg)?;
        if let Some(stdin) = self.stdin.as_mut() {
            writeln!(stdin, "{}", line).context("failed to write to MLX live stdin")?;
            stdin.flush().ok();
        } else {
            anyhow::bail!("MLX live stdin closed");
        }
        Ok(())
    }

    /// Non-blocking drain of any hypothesis already emitted by Python.
    pub fn try_recv_event(&self) -> Option<MlxLiveEvent> {
        self.events.try_recv().ok()
    }

    /// Blocking recv for the final hypothesis after `finalize()`. Waits up to `timeout`.
    pub fn recv_event_timeout(&self, timeout: Duration) -> Option<MlxLiveEvent> {
        self.events.recv_timeout(timeout).ok()
    }

    /// Signal end-of-utterance and wait for the final committed text. Returns
    /// the final string on success.
    pub fn finalize(mut self, timeout: Duration) -> Result<Option<String>> {
        let msg = serde_json::json!({ "type": "finalize" });
        let line = serde_json::to_string(&msg)?;
        if let Some(stdin) = self.stdin.as_mut() {
            if writeln!(stdin, "{}", line).is_err() {
                let _ = self.child.kill();
                return Ok(None);
            }
            let _ = stdin.flush();
        }
        // Close stdin to signal EOF
        self.stdin.take();

        // Wait for final event (reader thread will forward it before exiting).
        let ev = self.events.recv_timeout(timeout).ok();
        // Ensure child is reaped; reader thread will join after final.
        let _ = self.child.wait();
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }
        Ok(ev.filter(|e| e.is_final).map(|e| e.committed))
    }

    /// Abort without waiting for a final result.
    pub fn cancel(mut self) {
        if let Some(stdin) = self.stdin.as_mut() {
            let msg = serde_json::json!({ "type": "cancel" });
            if let Ok(line) = serde_json::to_string(&msg) {
                let _ = writeln!(stdin, "{}", line);
                let _ = stdin.flush();
            }
        }
        self.stdin.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for MlxLiveSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }
    }
}

pub fn runtime_info_blocking() -> MlxRuntimeInfo {
    let (rt, diag) = discover_runtime();
    let readiness = rt.as_ref().and_then(probe_metal_readiness);
    let available = readiness.is_some();
    let probe = if let (Some(rt), Some(_)) = (rt.as_ref(), readiness.as_ref()) {
        probe_python(rt)
    } else {
        None
    };

    let status = match readiness {
        Some((python, seconds)) => ready_status(&python, seconds),
        None => diag.message(),
    };

    MlxRuntimeInfo {
        available,
        status,
        python_path: rt.as_ref().map(|r| r.python.display().to_string()),
        script_path: rt.as_ref().map(|r| r.script.display().to_string()),
        python_candidates: diag.python_candidates,
        script_candidates: diag.script_candidates,
        instructions: "Run: bash src-tauri/src/mlx/shell.sh  ·  verify: python doctor.py",
        probe,
    }
}
