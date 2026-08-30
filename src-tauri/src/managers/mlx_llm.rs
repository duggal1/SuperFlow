//! Persistent local MLX LLM inference.
//!
//! When the user enables "Local AI LLM" in settings, every prompt that would
//! have gone to Gemini is routed here instead — same system prompt, same user
//! content, same pipeline. Inference runs through the existing `mlx_voice.py`
//! runtime in a long-lived `llm-serve` subprocess: the selected model is
//! loaded once (weights come from the shared Hugging Face cache) and reused
//! for every subsequent request. Switching models restarts the single session.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};

use super::mlx::find_runtime;

/// Hard wall-clock limit for one local generation. Big low-bit models on
/// small-RAM machines can be slow; this mirrors the MLX transcribe timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(600);

struct LlmSession {
    model: String,
    child: Child,
    stdin: ChildStdin,
    responses: mpsc::Receiver<String>,
    _reader: thread::JoinHandle<()>,
    _stderr_drain: thread::JoinHandle<()>,
    stderr_tail: Arc<Mutex<String>>,
}

impl LlmSession {
    fn send(&mut self, system: &str, user: &str, max_tokens: u32) -> Result<()> {
        let request = serde_json::json!({
            "system": system,
            "user": user,
            "max_tokens": max_tokens,
        });
        writeln!(self.stdin, "{}", request).context("local LLM stdin closed")?;
        self.stdin.flush().ok();
        Ok(())
    }

    /// Wait for one JSON line response: `{"text": ...}` or `{"error": ...}`.
    fn recv(&self) -> Result<String> {
        let deadline = Instant::now() + REQUEST_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                bail!(
                    "local LLM request timed out after {}s",
                    REQUEST_TIMEOUT.as_secs()
                );
            }
            match self
                .responses
                .recv_timeout(remaining.min(Duration::from_millis(500)))
            {
                Ok(line) => {
                    #[derive(serde::Deserialize)]
                    struct Reply {
                        #[serde(default)]
                        text: Option<String>,
                        #[serde(default)]
                        error: Option<String>,
                    }
                    let reply: Reply = serde_json::from_str(&line).with_context(|| {
                        format!("bad local LLM reply: {}", &line[..line.len().min(200)])
                    })?;
                    if let Some(text) = reply.text {
                        return Ok(text.trim().to_string());
                    }
                    if let Some(error) = reply.error {
                        bail!("local LLM error: {error}");
                    }
                    // Not a reply (e.g. the startup `{"ready": ...}` beacon)
                    // — keep waiting for the actual response line.
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let tail = self
                        .stderr_tail
                        .lock()
                        .map(|tail| tail.clone())
                        .unwrap_or_default();
                    let tail: String = tail.chars().rev().take(500).collect();
                    let tail: String = tail.chars().rev().collect();
                    bail!("local LLM process exited; stderr: {tail}");
                }
            }
        }
    }
}

impl Drop for LlmSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn spawn_session(model: &str) -> Result<LlmSession> {
    let rt = find_runtime().ok_or_else(|| {
        anyhow!(
            "MLX runtime not found — run:\n    bash src-tauri/src/mlx/shell.sh\nthen restart SuperFlow."
        )
    })?;

    let mut child = Command::new(&rt.python)
        .arg(&rt.script)
        .arg("llm-serve")
        .arg("--llm")
        .arg(model)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn mlx_voice.py llm-serve")?;

    let stdin = child.stdin.take().context("no llm-serve stdin")?;
    let stdout = child.stdout.take().context("no llm-serve stdout")?;
    let stderr = child.stderr.take().context("no llm-serve stderr")?;

    let (tx, rx) = mpsc::channel::<String>();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(line) if !line.trim().is_empty() => {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
                Ok(_) => continue,
                Err(_) => break,
            }
        }
    });

    // Keep the stderr pipe drained so a chatty child can never block on a
    // full pipe buffer; remember the tail for diagnostics.
    let stderr_tail: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let tail_handle = Arc::clone(&stderr_tail);
    let stderr_drain = thread::spawn(move || {
        let mut drained = String::new();
        let _ = std::io::BufReader::new(stderr).read_to_string(&mut drained);
        if let Ok(mut tail) = tail_handle.lock() {
            *tail = drained;
        }
    });

    Ok(LlmSession {
        model: model.to_string(),
        child,
        stdin,
        responses: rx,
        _reader: reader,
        _stderr_drain: stderr_drain,
        stderr_tail,
    })
}

/// Generate one completion with the selected local MLX model. Blocking; call
/// from a `spawn_blocking` context. The subprocess (and its loaded model)
/// survives across calls — nothing is reloaded per request.
pub fn generate_blocking(
    system_prompt: &str,
    user_content: &str,
    model: &str,
    max_tokens: u32,
) -> Result<String, String> {
    static SESSION: Mutex<Option<LlmSession>> = Mutex::new(None);

    let mut guard = SESSION
        .lock()
        .map_err(|error| format!("local LLM lock: {error}"))?;

    let needs_spawn = match guard.as_ref() {
        Some(session) => session.model != model,
        None => true,
    };
    if needs_spawn {
        *guard = Some(spawn_session(model).map_err(|error| format!("{error:#}"))?);
    }

    let session = guard.as_mut().expect("session just spawned");
    match session
        .send(system_prompt, user_content, max_tokens)
        .map_err(|error| format!("{error:#}"))
        .and_then(|()| session.recv().map_err(|error| format!("{error:#}")))
    {
        Ok(text) => Ok(text),
        Err(first_error) => {
            // One fresh-process retry: the child may have been OOM-killed or
            // left the runtime in a bad state. `send`/`recv` cleared the dead
            // session above, so this spawns a clean one.
            let mut session = spawn_session(model)
                .map_err(|error| format!("{first_error}; retry failed: {error:#}"))?;
            session
                .send(system_prompt, user_content, max_tokens)
                .map_err(|error| format!("{error:#}"))?;
            let result = session.recv().map_err(|error| format!("{error:#}"));
            *guard = Some(session);
            result
        }
    }
}
