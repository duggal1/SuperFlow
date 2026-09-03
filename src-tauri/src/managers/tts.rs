//! Pocket-TTS voice engine (CrispASR `pocket-tts` backend, CPU-only).
//!
//! Runs the `crispasr` CLI binary against the `pocket-tts-english-novc-f16.gguf`
//! checkpoint (Kyutai pocket-tts, FlowLM + Mimi decoder — a custom GGUF
//! architecture that llama.cpp cannot load). The `novc` checkpoint is a
//! single default-voice model: no voice embeddings, no reference audio.
//!
//! Threading: pinned to 4 threads (`-t 4`) — the four performance cores on
//! Apple Silicon. The binary never touches the GPU: CPU is CrispASR's default
//! and Kyutai's own recommendation for this model. The OS may schedule up to
//! all eight cores if the workload demands it; we guarantee a minimum of four
//! perf cores.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use futures_util::StreamExt;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Emitter, Manager};

const MODEL_FILE: &str = "pocket-tts-english-novc-f16.gguf";
const MODEL_URL: &str =
    "https://huggingface.co/cstr/pocket-tts-GGUF/resolve/main/pocket-tts-english-novc-f16.gguf";
/// Performance-core count passed to CrispASR (`-t`). Four performance cores
/// on Apple Silicon; the efficiency cores stay free for the rest of the app.
/// The OS may burst to all eight cores under load; this is the guaranteed minimum.
const TTS_THREADS: u32 = 4;
const FIXED_SEED: u32 = 42;
const DOWNLOAD_EVENT: &str = "tts-download-progress";

#[derive(Serialize, Deserialize, Clone, Type)]
pub struct TtsStatus {
    pub engine_available: bool,
    pub model_downloaded: bool,
    pub model_size_bytes: u64,
    pub downloading: bool,
}

#[derive(Serialize, Clone, Type)]
pub struct TtsDownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percent: u8,
}

/// One Pocket-TTS preset voice (Kyutai catalog). The `novc` checkpoint ships
/// a single default voice; these eight presets are the selection layer.
/// Selection persists locally; the engine receives `--voice` only when it
/// resolves to a real GGUF pack / reference WAV on disk, otherwise the
/// default-voice path runs unchanged.
#[derive(Serialize, Deserialize, Clone, Type)]
pub struct TtsVoice {
    pub id: String,
    pub name: String,
}

pub const POCKET_VOICES: [(&str, &str); 8] = [
    ("alba", "Alba"),
    ("marius", "Marius"),
    ("javert", "Javert"),
    ("jean", "Jean"),
    ("fantine", "Fantine"),
    ("cosette", "Cosette"),
    ("eponine", "Eponine"),
    ("azelma", "Azelma"),
];

pub const DEFAULT_VOICE: &str = "alba";
const SELECTED_VOICE_FILE: &str = "selected_voice.txt";

pub struct TtsManager {
    model_path: PathBuf,
    downloading: Arc<AtomicBool>,
}

impl TtsManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let dir = crate::portable::app_data_dir(app_handle)
            .map_err(|e| anyhow::anyhow!("Failed to get app data dir: {e}"))?
            .join("models")
            .join("pocket-tts");
        std::fs::create_dir_all(&dir)?;
        Ok(Self {
            model_path: dir.join(MODEL_FILE),
            downloading: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Resolve the `crispasr` binary. Order:
    /// 1. Bundled resource (`resources/crispasr/crispasr` — release builds).
    /// 2. `CRISPASR_BIN` env override.
    /// 3. Filesystem candidates: next to the exe, CWD-relative layouts, and
    ///    the sibling `../CrispASR/build/bin/crispasr` workspace build no
    ///    matter how deep the exe lives (dev target dir, .app bundle, etc).
    fn engine_path(app_handle: &AppHandle) -> Option<PathBuf> {
        let mut checked: Vec<String> = Vec::new();
        let mut consider = |p: PathBuf| -> Option<PathBuf> {
            // Normalize `..` segments so logs show the real location.
            let norm = p.canonicalize().unwrap_or(p);
            checked.push(norm.to_string_lossy().to_string());
            if norm.exists() {
                Some(norm)
            } else {
                None
            }
        };

        // 1. Bundled resource (release builds ship resources/crispasr/crispasr).
        if let Ok(resource) = app_handle
            .path()
            .resolve("resources/crispasr/crispasr", tauri::path::BaseDirectory::Resource)
        {
            if let Some(p) = consider(resource) {
                log::info!("pocket-tts: using bundled crispasr at {}", p.display());
                return Some(p);
            }
        }
        // 2. Explicit env override.
        if let Ok(env_path) = std::env::var("CRISPASR_BIN") {
            if !env_path.trim().is_empty() {
                if let Some(p) = consider(PathBuf::from(env_path)) {
                    log::info!("pocket-tts: using CRISPASR_BIN at {}", p.display());
                    return Some(p);
                }
            }
        }
        // 3. Filesystem candidates.
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.push(dir.join("crispasr"));
                candidates.push(dir.join("resources/crispasr/crispasr"));
                // Walk up several levels so target/debug, .app/Contents/MacOS,
                // and other layouts all reach the workspace sibling checkout.
                let mut anchor = dir.to_path_buf();
                for _ in 0..7 {
                    candidates.push(anchor.join("../CrispASR/build/bin/crispasr"));
                    match anchor.parent() {
                        Some(parent) => anchor = parent.to_path_buf(),
                        None => break,
                    }
                }
            }
        }
        if let Ok(cwd) = std::env::current_dir() {
            candidates.push(cwd.join("resources/crispasr/crispasr"));
            candidates.push(cwd.join("src-tauri/resources/crispasr/crispasr"));
            candidates.push(cwd.join("../CrispASR/build/bin/crispasr"));
        }
        for c in candidates {
            if let Some(p) = consider(c) {
                log::info!("pocket-tts: using crispasr at {}", p.display());
                return Some(p);
            }
        }

        log::warn!(
            "pocket-tts: crispasr engine not found; checked: {}",
            checked.join(" | ")
        );
        None
    }

    fn voice_dir(app_handle: &AppHandle) -> Result<PathBuf> {
        Ok(crate::portable::app_data_dir(app_handle)
            .map_err(|e| anyhow::anyhow!("Failed to get app data dir: {e}"))?
            .join("models")
            .join("pocket-tts"))
    }

    pub fn voices() -> Vec<TtsVoice> {
        POCKET_VOICES
            .iter()
            .map(|(id, name)| TtsVoice {
                id: id.to_string(),
                name: name.to_string(),
            })
            .collect()
    }

    pub fn selected_voice(&self) -> String {
        let path = self
            .model_path
            .parent()
            .map(|d| d.join(SELECTED_VOICE_FILE));
        let saved = path
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty());
        match saved {
            Some(v) if POCKET_VOICES.iter().any(|(id, _)| *id == v) => v,
            _ => DEFAULT_VOICE.to_string(),
        }
    }

    pub fn set_voice(&self, voice: String) -> Result<()> {
        let voice = voice.trim().to_string();
        let lowered = voice.to_lowercase();
        let is_preset = POCKET_VOICES.iter().any(|(id, _)| *id == lowered);
        let is_pack = (lowered.ends_with(".gguf") || lowered.ends_with(".wav"))
            && std::path::Path::new(&voice).exists();
        if !is_preset && !is_pack {
            anyhow::bail!("unknown Pocket-TTS voice: {voice}");
        }
        let voice = if is_preset { lowered } else { voice };
        let path = self
            .model_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("voice dir missing"))?
            .join(SELECTED_VOICE_FILE);
        std::fs::write(path, format!("{voice}\n"))?;
        info!("pocket-tts: selected voice -> {voice}");
        Ok(())
    }

    pub fn status(&self, app_handle: &AppHandle) -> TtsStatus {
        let model_downloaded = self.model_path.exists();
        let model_size_bytes = model_downloaded
            .then(|| std::fs::metadata(&self.model_path).map(|m| m.len()).unwrap_or(0))
            .unwrap_or(0);
        TtsStatus {
            engine_available: Self::engine_path(app_handle).is_some(),
            model_downloaded,
            model_size_bytes,
            downloading: self.downloading.load(Ordering::Relaxed),
        }
    }

    /// Download the GGUF checkpoint with streaming progress events. Idempotent:
    /// a second call while a download is in flight is a no-op. Fully cached on
    /// success so subsequent synthesis has zero latency / no GPU fallback.
    pub async fn download_model(&self, app_handle: AppHandle) -> Result<()> {
        if self.model_path.exists() {
            return Ok(());
        }
        if self
            .downloading
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(());
        }
        let result = self.download_inner(app_handle.clone()).await;
        self.downloading.store(false, Ordering::SeqCst);
        result
    }

    async fn download_inner(&self, app_handle: AppHandle) -> Result<()> {
        info!("pocket-tts: downloading {MODEL_URL}");
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60 * 30))
            .build()?;
        let mut request = client.get(MODEL_URL);
        if let Ok(token) = std::env::var("HF_TOKEN") {
            if !token.trim().is_empty() {
                request = request.bearer_auth(token.trim());
            }
        }
        let response = request.send().await?.error_for_status()?;
        let total = response.content_length().unwrap_or(0);
        let tmp_path = self.model_path.with_extension("gguf.part");
        // remove stale partial
        let _ = tokio::fs::remove_file(&tmp_path).await;
        let mut file = tokio::fs::File::create(&tmp_path).await?;
        let mut downloaded: u64 = 0;
        let mut last_percent: i16 = -1;
        let mut stream = response.bytes_stream();
        use tokio::io::AsyncWriteExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            let percent = if total > 0 {
                ((downloaded as f64 / total as f64) * 100.0) as i16
            } else {
                0
            };
            if percent != last_percent {
                last_percent = percent;
                let _ = app_handle.emit(
                    DOWNLOAD_EVENT,
                    TtsDownloadProgress {
                        downloaded_bytes: downloaded,
                        total_bytes: total,
                        percent: percent.clamp(0, 100) as u8,
                    },
                );
            }
        }
        file.flush().await?;
        drop(file);
        tokio::fs::rename(&tmp_path, &self.model_path).await?;
        info!("pocket-tts: model cached at {}", self.model_path.display());
        let _ = app_handle.emit(
            DOWNLOAD_EVENT,
            TtsDownloadProgress {
                downloaded_bytes: total,
                total_bytes: total,
                percent: 100,
            },
        );
        Ok(())
    }

    /// Synthesize `text` to a WAV file and return its path. Fully local and
    /// CPU-only; a fresh call overwrites the previous preview output.
    pub fn synthesize(&self, app_handle: &AppHandle, text: String) -> Result<PathBuf> {
        let engine = Self::engine_path(app_handle)
            .ok_or_else(|| anyhow::anyhow!("crispasr engine not found (bundle missing and CRISPASR_BIN unset)"))?;
        if !self.model_path.exists() {
            anyhow::bail!("pocket-tts model is not downloaded yet");
        }
        let text = text.trim().to_string();
        if text.is_empty() {
            anyhow::bail!("nothing to say");
        }
        if text.len() > 2000 {
            anyhow::bail!("text too long (max 2000 chars)");
        }
        let out_dir = crate::portable::app_data_dir(app_handle)
            .map_err(|e| anyhow::anyhow!("Failed to get app data dir: {e}"))?
            .join("tts-preview");
        std::fs::create_dir_all(&out_dir)?;
        let out_path = out_dir.join("preview.wav");
        let _ = std::fs::remove_file(&out_path);

        // Voice conditioning: `--voice` takes a GGUF pack / reference WAV
        // path, not a preset name — so it is passed only when the selection
        // resolves to a real file. Otherwise the default-voice path runs
        // unchanged (zero regression risk on the novc checkpoint).
        // CPU-only: CrispASR's default is CPU; we force 4 perf-core threads.
        let voice = self.selected_voice();
        let voice_path = std::path::Path::new(&voice);
        let use_voice_flag = voice != DEFAULT_VOICE
            && (voice.ends_with(".gguf") || voice.ends_with(".wav"))
            && voice_path.exists();
        let mut cmd = std::process::Command::new(&engine);
        cmd.arg("--backend")
            .arg("pocket-tts")
            .arg("-m")
            .arg(&self.model_path)
            .arg("-t")
            .arg(TTS_THREADS.to_string());
        if use_voice_flag {
            cmd.arg("--voice").arg(&voice);
        }
        let output = cmd
            .arg("--tts")
            .arg(&text)
            .arg("--tts-output")
            .arg(&out_path)
            .arg("--seed")
            .arg(FIXED_SEED.to_string())
            .output()?;

        if !output.status.success() || !out_path.exists() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let combined = format!("{stderr}\n{stdout}");
            let tail: String = combined
                .lines()
                .rev()
                .take(10)
                .collect::<Vec<_>>()
                .join("\n")
                .chars()
                .rev()
                .take(1500)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            warn!("pocket-tts synthesis failed: {tail}");
            anyhow::bail!("TTS engine failed: {tail}");
        }
        Ok(out_path)
    }
}

pub fn progress_event_name() -> &'static str {
    DOWNLOAD_EVENT
}
