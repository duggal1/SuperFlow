//! Pocket-TTS voice engine backed by one warm, local CrispASR server.
//!
//! CrispASR stays resident so model initialization is never paid on a preview
//! click. Audio is returned as progressive 24 kHz PCM and never touches a
//! shared preview file.

use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures_util::StreamExt;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{ipc::Channel, AppHandle, Emitter, Manager};

const MODEL_FILE: &str = "pocket-tts-english-novc-f16.gguf";
const MODEL_URL: &str =
    "https://huggingface.co/cstr/pocket-tts-GGUF/resolve/main/pocket-tts-english-novc-f16.gguf";
const VOICE_REVISION: &str = "e81d79e8194ad4c7ce879c87a4258ef20cbf2487";
const VOICE_REPOSITORY: &str = "kyutai/pocket-tts-without-voice-cloning";
const TTS_THREADS: u32 = 4;
const FIXED_SEED: u32 = 42;
const SAMPLE_RATE: u32 = 24_000;
const IPC_CHUNK_SAMPLES: usize = 1_920;
const DOWNLOAD_EVENT: &str = "tts-download-progress";
const SELECTED_VOICE_FILE: &str = "selected_voice.txt";

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

#[derive(Serialize, Deserialize, Clone, Type)]
pub struct TtsVoice {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Clone, Type)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TtsStreamEvent {
    Started {
        sample_rate: u32,
    },
    Chunk {
        samples: Vec<i16>,
    },
    Finished {
        duration_ms: u64,
        first_audio_ms: u64,
    },
}

#[derive(Serialize, Clone, Type)]
pub struct TtsSynthesisSummary {
    pub duration_ms: u64,
    pub first_audio_ms: u64,
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

struct TtsServer {
    child: Child,
    base_url: String,
}

pub struct TtsManager {
    engine_path: Option<PathBuf>,
    model_dir: PathBuf,
    model_path: PathBuf,
    client: reqwest::Client,
    downloading: Arc<AtomicBool>,
    server: Mutex<Option<TtsServer>>,
    start_lock: tokio::sync::Mutex<()>,
    synthesis_lock: tokio::sync::Mutex<()>,
}

impl TtsManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let model_dir = crate::portable::app_data_dir(app_handle)
            .map_err(|e| anyhow::anyhow!("Failed to get app data dir: {e}"))?
            .join("models")
            .join("pocket-tts");
        std::fs::create_dir_all(&model_dir)?;
        Ok(Self {
            engine_path: Self::resolve_engine_path(app_handle),
            model_path: model_dir.join(MODEL_FILE),
            model_dir,
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(120))
                .build()?,
            downloading: Arc::new(AtomicBool::new(false)),
            server: Mutex::new(None),
            start_lock: tokio::sync::Mutex::new(()),
            synthesis_lock: tokio::sync::Mutex::new(()),
        })
    }

    fn resolve_engine_path(app_handle: &AppHandle) -> Option<PathBuf> {
        let mut candidates = Vec::new();
        if let Ok(path) = app_handle.path().resolve(
            "resources/crispasr/crispasr",
            tauri::path::BaseDirectory::Resource,
        ) {
            candidates.push(path);
        }
        if let Ok(path) = std::env::var("CRISPASR_BIN") {
            if !path.trim().is_empty() {
                candidates.push(PathBuf::from(path));
            }
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.push(dir.join("crispasr"));
                candidates.push(dir.join("resources/crispasr/crispasr"));
                let mut anchor = dir.to_path_buf();
                for _ in 0..7 {
                    candidates.push(anchor.join("../CrispASR/build/bin/crispasr"));
                    let Some(parent) = anchor.parent() else { break };
                    anchor = parent.to_path_buf();
                }
            }
        }
        if let Ok(cwd) = std::env::current_dir() {
            candidates.push(cwd.join("resources/crispasr/crispasr"));
            candidates.push(cwd.join("src-tauri/resources/crispasr/crispasr"));
            candidates.push(cwd.join("../CrispASR/build/bin/crispasr"));
        }

        let engine = candidates.into_iter().find_map(|path| {
            let normalized = path.canonicalize().unwrap_or(path);
            normalized.is_file().then_some(normalized)
        });
        match &engine {
            Some(path) => info!("pocket-tts: using crispasr at {}", path.display()),
            None => warn!("pocket-tts: crispasr engine not found"),
        }
        engine
    }

    fn voice_path(&self, voice: &str) -> PathBuf {
        self.model_dir.join(format!("{voice}.safetensors"))
    }

    fn voice_url(voice: &str) -> String {
        format!("https://huggingface.co/{VOICE_REPOSITORY}/resolve/{VOICE_REVISION}/languages/english/embeddings/{voice}.safetensors")
    }

    fn assets_ready(&self) -> bool {
        file_is_ready(&self.model_path)
            && POCKET_VOICES
                .iter()
                .all(|(voice, _)| file_is_ready(&self.voice_path(voice)))
    }

    fn asset_size(&self) -> u64 {
        std::iter::once(self.model_path.clone())
            .chain(
                POCKET_VOICES
                    .iter()
                    .map(|(voice, _)| self.voice_path(voice)),
            )
            .filter_map(|path| std::fs::metadata(path).ok().map(|metadata| metadata.len()))
            .sum()
    }

    pub fn voices() -> Vec<TtsVoice> {
        POCKET_VOICES
            .iter()
            .map(|(id, name)| TtsVoice {
                id: (*id).into(),
                name: (*name).into(),
            })
            .collect()
    }

    pub fn selected_voice(&self) -> String {
        std::fs::read_to_string(self.model_dir.join(SELECTED_VOICE_FILE))
            .ok()
            .map(|voice| voice.trim().to_lowercase())
            .filter(|voice| POCKET_VOICES.iter().any(|(id, _)| *id == voice))
            .unwrap_or_else(|| DEFAULT_VOICE.to_string())
    }

    pub fn set_voice(&self, voice: String) -> Result<()> {
        let voice = voice.trim().to_lowercase();
        if !POCKET_VOICES.iter().any(|(id, _)| *id == voice) {
            anyhow::bail!("unknown Pocket-TTS voice: {voice}");
        }
        std::fs::write(
            self.model_dir.join(SELECTED_VOICE_FILE),
            format!("{voice}\n"),
        )?;
        info!("pocket-tts: selected voice -> {voice}");
        Ok(())
    }

    pub fn status(&self) -> TtsStatus {
        TtsStatus {
            engine_available: self.engine_path.is_some(),
            model_downloaded: self.assets_ready(),
            model_size_bytes: self.asset_size(),
            downloading: self.downloading.load(Ordering::Relaxed),
        }
    }

    pub async fn download_model(&self, app_handle: AppHandle) -> Result<()> {
        if self.assets_ready() {
            return Ok(());
        }
        if self
            .downloading
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(());
        }
        let result = self.download_missing_assets(&app_handle).await;
        self.downloading.store(false, Ordering::SeqCst);
        if result.is_ok() {
            let _ = self.warm().await;
        }
        result
    }

    async fn download_missing_assets(&self, app_handle: &AppHandle) -> Result<()> {
        let mut assets = vec![(self.model_path.clone(), MODEL_URL.to_string())];
        assets.extend(
            POCKET_VOICES
                .iter()
                .map(|(voice, _)| (self.voice_path(voice), Self::voice_url(voice))),
        );
        let missing: Vec<_> = assets
            .into_iter()
            .filter(|(path, _)| !file_is_ready(path))
            .collect();

        let mut sizes = Vec::with_capacity(missing.len());
        for (_, url) in &missing {
            sizes.push(
                self.client
                    .head(url)
                    .send()
                    .await?
                    .error_for_status()?
                    .content_length()
                    .unwrap_or(0),
            );
        }
        let existing = self.asset_size();
        let total = existing + sizes.iter().sum::<u64>();
        let mut downloaded = existing;

        for ((path, url), expected_size) in missing.into_iter().zip(sizes) {
            info!(
                "pocket-tts: downloading {}",
                path.file_name().unwrap_or_default().to_string_lossy()
            );
            let response = self.client.get(&url).send().await?.error_for_status()?;
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("download");
            let part_path = path.with_extension(format!("{extension}.part"));
            let _ = tokio::fs::remove_file(&part_path).await;
            let mut file = tokio::fs::File::create(&part_path).await?;
            let mut stream = response.bytes_stream();
            let mut asset_downloaded = 0_u64;
            use tokio::io::AsyncWriteExt;
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                file.write_all(&chunk).await?;
                asset_downloaded += chunk.len() as u64;
                emit_download_progress(app_handle, downloaded + asset_downloaded, total);
            }
            file.flush().await?;
            drop(file);
            if expected_size > 0 && asset_downloaded != expected_size {
                let _ = tokio::fs::remove_file(&part_path).await;
                anyhow::bail!("incomplete TTS asset download: expected {expected_size} bytes, received {asset_downloaded}");
            }
            tokio::fs::rename(&part_path, &path).await?;
            downloaded += asset_downloaded;
        }
        emit_download_progress(app_handle, total, total);
        Ok(())
    }

    pub async fn warm(&self) -> Result<()> {
        if !self.assets_ready() || self.engine_path.is_none() {
            return Ok(());
        }
        let endpoint = self.ensure_server().await?;
        self.speech_request(&endpoint, "Ready.", &self.selected_voice())
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        info!("pocket-tts: warmup complete");
        Ok(())
    }

    pub async fn synthesize_stream(
        &self,
        text: String,
        on_event: Channel<TtsStreamEvent>,
    ) -> Result<TtsSynthesisSummary> {
        let text = text.trim().to_string();
        if text.is_empty() {
            anyhow::bail!("nothing to say");
        }
        if text.chars().count() > 2000 {
            anyhow::bail!("text too long (max 2000 chars)");
        }
        if !self.assets_ready() {
            anyhow::bail!("Pocket-TTS model and voices are not downloaded yet");
        }

        let _synthesis_guard = self.synthesis_lock.lock().await;
        let started_at = Instant::now();
        let endpoint = self.ensure_server().await?;
        let response = self
            .speech_request(&endpoint, &text, &self.selected_voice())
            .await
            .context("Pocket-TTS server request failed")?
            .error_for_status()
            .context("Pocket-TTS server rejected synthesis")?;

        let mut stream = response.bytes_stream();
        let mut pending_byte = None;
        let mut sample_buffer = Vec::with_capacity(IPC_CHUNK_SAMPLES * 2);
        let mut total_samples = 0_u64;
        let mut first_audio_ms = None;

        while let Some(chunk) = stream.next().await {
            decode_pcm_chunk(&chunk?, &mut pending_byte, &mut sample_buffer);
            while sample_buffer.len() >= IPC_CHUNK_SAMPLES {
                let samples: Vec<i16> = sample_buffer.drain(..IPC_CHUNK_SAMPLES).collect();
                total_samples += samples.len() as u64;
                if first_audio_ms.is_none() {
                    first_audio_ms = Some(started_at.elapsed().as_millis() as u64);
                    on_event.send(TtsStreamEvent::Started {
                        sample_rate: SAMPLE_RATE,
                    })?;
                }
                on_event.send(TtsStreamEvent::Chunk { samples })?;
            }
        }
        if pending_byte.is_some() {
            anyhow::bail!("Pocket-TTS returned malformed PCM audio");
        }
        if !sample_buffer.is_empty() {
            total_samples += sample_buffer.len() as u64;
            if first_audio_ms.is_none() {
                first_audio_ms = Some(started_at.elapsed().as_millis() as u64);
                on_event.send(TtsStreamEvent::Started {
                    sample_rate: SAMPLE_RATE,
                })?;
            }
            on_event.send(TtsStreamEvent::Chunk {
                samples: sample_buffer,
            })?;
        }
        if total_samples == 0 {
            anyhow::bail!("Pocket-TTS returned no audio");
        }

        let summary = TtsSynthesisSummary {
            duration_ms: total_samples.saturating_mul(1000) / u64::from(SAMPLE_RATE),
            first_audio_ms: first_audio_ms.unwrap_or_default(),
        };
        on_event.send(TtsStreamEvent::Finished {
            duration_ms: summary.duration_ms,
            first_audio_ms: summary.first_audio_ms,
        })?;
        info!(
            "pocket-tts: streamed {:.2}s of audio; first audio {}ms",
            summary.duration_ms as f64 / 1000.0,
            summary.first_audio_ms
        );
        Ok(summary)
    }

    async fn speech_request(
        &self,
        endpoint: &str,
        text: &str,
        voice: &str,
    ) -> Result<reqwest::Response> {
        Ok(self.client.post(format!("{endpoint}/v1/audio/speech"))
            .json(&serde_json::json!({
                "model": "pocket-tts",
                "input": text,
                "voice": voice,
                "stream": true,
                "response_format": "pcm",
                "seed": FIXED_SEED,
                "spoken_disclaimer": false,
                "consent_attestation": "Kyutai publishes this prepared preset voice for licensed TTS use",
                "marking_attestation": "SuperFlow identifies this preview as AI-generated audio",
                "speaker_identity": "unknown"
            }))
            .send().await?)
    }

    async fn ensure_server(&self) -> Result<String> {
        let _start_guard = self.start_lock.lock().await;
        let existing_url = {
            let mut server = self
                .server
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(process) = server.as_mut() {
                if process.child.try_wait()?.is_none() {
                    Some(process.base_url.clone())
                } else {
                    *server = None;
                    None
                }
            } else {
                None
            }
        };
        if let Some(url) = existing_url {
            if self.health_ready(&url).await {
                return Ok(url);
            }
        }

        self.stop_server();
        let engine = self
            .engine_path
            .as_ref()
            .context("crispasr engine is not available")?;
        let port = available_local_port()?;
        let base_url = format!("http://127.0.0.1:{port}");
        let mut child = Command::new(engine)
            .arg("--server")
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .arg("--backend")
            .arg("pocket-tts")
            .arg("-m")
            .arg(&self.model_path)
            .arg("-t")
            .arg(TTS_THREADS.to_string())
            .arg("--voice-dir")
            .arg(&self.model_dir)
            .arg("--tts-max-input-chars")
            .arg("2000")
            .arg("--no-punctuation")
            .arg("--no-spoken-disclaimer")
            .arg("--accept-marking-responsibility")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to start the Pocket-TTS server")?;
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    debug!("crispasr-tts: {line}");
                }
            });
        }
        {
            let mut server = self
                .server
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *server = Some(TtsServer {
                child,
                base_url: base_url.clone(),
            });
        }

        let deadline = Instant::now() + Duration::from_secs(45);
        while Instant::now() < deadline {
            if self.health_ready(&base_url).await {
                info!("pocket-tts: persistent server ready at {base_url}");
                return Ok(base_url);
            }
            let exited = {
                let mut server = self
                    .server
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                server
                    .as_mut()
                    .and_then(|process| process.child.try_wait().ok().flatten())
            };
            if let Some(status) = exited {
                self.stop_server();
                anyhow::bail!("Pocket-TTS server exited during startup: {status}");
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        self.stop_server();
        anyhow::bail!("Pocket-TTS server did not become ready within 45 seconds")
    }

    async fn health_ready(&self, base_url: &str) -> bool {
        self.client
            .get(format!("{base_url}/health"))
            .send()
            .await
            .is_ok_and(|response| response.status().is_success())
    }

    fn stop_server(&self) {
        let mut server = self
            .server
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(mut process) = server.take() {
            let _ = process.child.kill();
            let _ = process.child.wait();
        }
    }
}

impl Drop for TtsManager {
    fn drop(&mut self) {
        self.stop_server();
    }
}

fn file_is_ready(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
}

fn available_local_port() -> Result<u16> {
    Ok(TcpListener::bind(("127.0.0.1", 0))?.local_addr()?.port())
}

fn emit_download_progress(app_handle: &AppHandle, downloaded: u64, total: u64) {
    let percent = if total == 0 {
        0
    } else {
        ((downloaded.saturating_mul(100) / total).min(100)) as u8
    };
    let _ = app_handle.emit(
        DOWNLOAD_EVENT,
        TtsDownloadProgress {
            downloaded_bytes: downloaded,
            total_bytes: total,
            percent,
        },
    );
}

fn decode_pcm_chunk(bytes: &[u8], pending_byte: &mut Option<u8>, samples: &mut Vec<i16>) {
    let mut index = 0;
    if let Some(low) = pending_byte.take() {
        if let Some(high) = bytes.first() {
            samples.push(i16::from_le_bytes([low, *high]));
            index = 1;
        } else {
            *pending_byte = Some(low);
            return;
        }
    }
    while index + 1 < bytes.len() {
        samples.push(i16::from_le_bytes([bytes[index], bytes[index + 1]]));
        index += 2;
    }
    if index < bytes.len() {
        *pending_byte = Some(bytes[index]);
    }
}

pub fn progress_event_name() -> &'static str {
    DOWNLOAD_EVENT
}

#[cfg(test)]
mod tests {
    use super::decode_pcm_chunk;

    #[test]
    fn decodes_pcm_across_network_chunk_boundaries() {
        let mut pending = None;
        let mut samples = Vec::new();
        decode_pcm_chunk(&[1, 0, 2], &mut pending, &mut samples);
        decode_pcm_chunk(&[0, 255, 255], &mut pending, &mut samples);
        assert_eq!(samples, vec![1, 2, -1]);
        assert_eq!(pending, None);
    }
}
