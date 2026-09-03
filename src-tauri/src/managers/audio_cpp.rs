use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Manager};

const FAMILY: &str = "granite5asr";
const SAMPLE_RATE: &str = "16000";
const CHANNELS: &str = "1";
const STDERR_TAIL_LINES: usize = 20;
static NEXT_OUTPUT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub struct AudioCppEngine {
    binary: PathBuf,
    model: PathBuf,
    staged_model: Option<PathBuf>,
    backend: &'static str,
}

impl AudioCppEngine {
    pub fn load(app_handle: &AppHandle, model: &Path) -> Result<Self> {
        let binary = resolve_binary(app_handle)?;
        let output = Command::new(&binary)
            .args(["--list-loaders", "--json"])
            .output()
            .with_context(|| format!("start audio.cpp runtime at {}", binary.display()))?;
        if !output.status.success() {
            bail!(
                "audio.cpp runtime check failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        if !String::from_utf8_lossy(&output.stdout).contains(FAMILY) {
            bail!("audio.cpp runtime does not include the {FAMILY} loader");
        }

        let (model, staged_model) = stage_symlinked_gguf(model)?;
        Ok(Self {
            binary,
            model,
            staged_model,
            backend: "best",
        })
    }

    pub fn backend(&self) -> &'static str {
        self.backend
    }

    pub fn start_stream(&self) -> Result<AudioCppLiveSession> {
        AudioCppLiveSession::spawn(&self.binary, &self.model, self.backend)
    }

    pub fn transcribe(&self, audio: &[f32]) -> Result<String> {
        let mut session = self.start_stream()?;
        session.feed(audio)?;
        session.finalize()
    }
}

impl Drop for AudioCppEngine {
    fn drop(&mut self) {
        if let Some(path) = self.staged_model.take() {
            let _ = fs::remove_file(path);
        }
    }
}

pub struct AudioCppLiveSession {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    output_path: PathBuf,
    /// Latest streaming snapshot the runtime has reported on stdout
    /// (`text_output=` lines), plus the last snapshot already surfaced to the
    /// caller so `take_partial` only yields fresh text.
    partial: Arc<Mutex<String>>,
    emitted: Arc<Mutex<String>>,
    /// Tail of the runtime's stderr (it logs verbosely — ggml/Metal pipeline
    /// compilation) kept for error reporting. Drained by a reader thread so a
    /// full pipe can never wedge the runtime.
    stderr_tail: Arc<Mutex<Vec<String>>>,
}

impl AudioCppLiveSession {
    fn spawn(binary: &Path, model: &Path, backend: &str) -> Result<Self> {
        let output_path = unique_output_path();
        let mut child = Command::new(binary)
            .args([
                "--task",
                "asr",
                "--family",
                FAMILY,
                "--backend",
                backend,
                "--mode",
                "streaming",
                "--audio",
                "-",
                "--input-format",
                "f32le",
                "--input-rate",
                SAMPLE_RATE,
                "--input-channels",
                CHANNELS,
                "--model",
            ])
            .arg(model)
            .arg("--text-out")
            .arg(&output_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("start audio.cpp runtime at {}", binary.display()))?;
        let stdin = child
            .stdin
            .take()
            .context("audio.cpp runtime did not expose stdin")?;

        let partial = Arc::new(Mutex::new(String::new()));
        let emitted = Arc::new(Mutex::new(String::new()));
        let stderr_tail = Arc::new(Mutex::new(Vec::new()));

        // Drain stdout continuously: streaming snapshots arrive as
        // `key=value` lines (`text_output=…`), and an unread pipe would fill
        // up and block the runtime mid-stream.
        if let Some(stdout) = child.stdout.take() {
            let partial_reader = Arc::clone(&partial);
            thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    if let Some(text) = line.strip_prefix("text_output=") {
                        *partial_reader.lock().unwrap() = normalize_transcript(text);
                    }
                }
            });
        }
        // Drain stderr the same way — the runtime logs every Metal kernel
        // compilation there, which is far more than one pipe buffer holds.
        if let Some(stderr) = child.stderr.take() {
            let stderr_reader = Arc::clone(&stderr_tail);
            thread::spawn(move || {
                for line in BufReader::new(stderr).lines() {
                    let Ok(line) = line else { break };
                    let mut tail = stderr_reader.lock().unwrap();
                    if tail.len() == STDERR_TAIL_LINES {
                        tail.remove(0);
                    }
                    tail.push(line);
                }
            });
        }

        Ok(Self {
            child: Some(child),
            stdin: Some(stdin),
            output_path,
            partial,
            emitted,
            stderr_tail,
        })
    }

    /// The newest streaming snapshot, if one has arrived since the last call.
    pub fn take_partial(&self) -> Option<String> {
        let current = self.partial.lock().unwrap().clone();
        if current.is_empty() {
            return None;
        }
        let mut last = self.emitted.lock().unwrap();
        if *last == current {
            return None;
        }
        *last = current.clone();
        Some(current)
    }

    pub fn feed(&mut self, audio: &[f32]) -> Result<()> {
        let mut bytes = Vec::with_capacity(audio.len() * size_of::<f32>());
        for sample in audio {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        self.stdin
            .as_mut()
            .context("audio.cpp stream is already finalized")?
            .write_all(&bytes)
            .context("write PCM to audio.cpp")
    }

    pub fn finalize(mut self) -> Result<String> {
        self.stdin.take();
        let mut child = self
            .child
            .take()
            .context("audio.cpp process is already finalized")?;
        let status = child.wait().context("wait for audio.cpp transcription")?;
        // Reader threads exit on EOF once the child is gone; the text-out file
        // is complete at this point.
        if !status.success() {
            let stderr = self.stderr_tail.lock().unwrap().join("\n");
            let _ = fs::remove_file(&self.output_path);
            bail!("audio.cpp transcription failed: {}", stderr.trim());
        }
        let text = fs::read_to_string(&self.output_path)
            .with_context(|| format!("read {}", self.output_path.display()))?;
        let _ = fs::remove_file(&self.output_path);
        Ok(normalize_transcript(&text))
    }

    pub fn cancel(mut self) {
        self.stdin.take();
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_file(&self.output_path);
    }
}

impl Drop for AudioCppLiveSession {
    fn drop(&mut self) {
        self.stdin.take();
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_file(&self.output_path);
    }
}

fn unique_output_path() -> PathBuf {
    let id = NEXT_OUTPUT_ID.fetch_add(1, Ordering::Relaxed);
    env::temp_dir().join(format!(
        "superflow-audiocpp-{}-{id}.txt",
        std::process::id()
    ))
}

fn normalize_transcript(text: &str) -> String {
    text.replace('\u{2581}', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn stage_symlinked_gguf(model: &Path) -> Result<(PathBuf, Option<PathBuf>)> {
    if !model
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Ok((model.to_path_buf(), None));
    }

    let source = fs::canonicalize(model)
        .with_context(|| format!("resolve audio.cpp model {}", model.display()))?;
    let id = NEXT_OUTPUT_ID.fetch_add(1, Ordering::Relaxed);
    let staged = env::temp_dir().join(format!(
        "superflow-audiocpp-model-{}-{id}.gguf",
        std::process::id()
    ));
    fs::hard_link(&source, &staged).with_context(|| {
        format!(
            "stage audio.cpp model {} as {}",
            source.display(),
            staged.display()
        )
    })?;
    Ok((staged.clone(), Some(staged)))
}

fn resolve_binary(app_handle: &AppHandle) -> Result<PathBuf> {
    if let Some(path) = env::var_os("SUPERFLOW_AUDIOCPP_BIN") {
        return Ok(PathBuf::from(path));
    }

    let executable_name = if cfg!(windows) {
        "audiocpp_cli.exe"
    } else {
        "audiocpp_cli"
    };
    let mut candidates = Vec::new();
    if let Ok(current_exe) = env::current_exe() {
        if let Some(directory) = current_exe.parent() {
            candidates.push(directory.join(executable_name));
        }
    }
    if let Ok(resource_dir) = app_handle.path().resource_dir() {
        candidates.push(resource_dir.join(executable_name));
    }
    candidates.push(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(format!("audiocpp_cli-{}", target_triple())),
    );
    candidates.push(PathBuf::from("/opt/homebrew/bin/audiocpp_cli"));
    candidates.push(PathBuf::from("/usr/local/bin/audiocpp_cli"));

    if let Some(path) = candidates.into_iter().find(|path| path.is_file()) {
        return Ok(path);
    }

    if Command::new(executable_name)
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
    {
        return Ok(PathBuf::from(executable_name));
    }

    bail!("audio.cpp runtime is not installed or bundled")
}

fn target_triple() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "aarch64-apple-darwin";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "x86_64-apple-darwin";
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "x86_64-pc-windows-msvc.exe";
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    return "aarch64-pc-windows-msvc.exe";
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "x86_64-unknown-linux-gnu";
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return "aarch64-unknown-linux-gnu";
    #[allow(unreachable_code)]
    "unsupported-target"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_paths_are_unique() {
        assert_ne!(unique_output_path(), unique_output_path());
    }

    #[test]
    fn normalizes_sentencepiece_word_boundaries() {
        assert_eq!(normalize_transcript("▁hello▁world\n"), "hello world");
    }
}
