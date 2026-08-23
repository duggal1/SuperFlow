//! Mandatory local transcript cleanup powered by S1-mini (superwhisper).
//!
//! Pipeline position: STT → deterministic passes (tech lexicon, fillers,
//! formatting) → **this stage** → optional cloud cleanup → paste. There is no
//! user-facing toggle by design; the only skips are correctness guards
//! (non-English transcripts, model not ready yet) which fail open to the raw
//! text rather than blocking dictation.

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use log::{info, warn};
use serde::Serialize;
use tauri::Emitter;

const MODEL_FILENAME: &str = "s1-mini-q4_k_m.gguf";
const MODEL_DISPLAY_NAME: &str = "S1 Mini Q4_K_M";
const MODEL_URL: &str =
    "https://huggingface.co/superwhisper/s1-mini-GGUF/resolve/main/s1-mini-q4_k_m.gguf";
const MODEL_SIZE_BYTES: u64 = 484_219_808;
const MODEL_SHA256: &str = "3b41ebe2502cbd03e811d5d16b022f5ab551eda58d62597d152f89535003c634";

const SYSTEM_PROMPT: &str = "You are a text normalizer for speech-to-text transcripts. The input begins with a control line specifying the styling, structure, and context settings; clean the transcript to match those settings and output only the cleaned text.";
const CONTROL_LINE: &str = "[Styling: semi-formal] [Structure: prose] [Context: general]";

/// The model card recommends keeping a single pass under ~1,000 tokens.
/// ~700 words sits safely inside that with the prompt overhead included.
const MAX_CHUNK_WORDS: usize = 700;

/// Longest we will hold one paste pipeline hostage waiting on generation.
const GENERATION_TIMEOUT_SECS: u64 = 60;

static JOB_TX: OnceLock<Mutex<Option<tokio::sync::mpsc::Sender<Job>>>> = OnceLock::new();
static READY: AtomicBool = AtomicBool::new(false);
static INSTALLING: AtomicBool = AtomicBool::new(false);

/// Download progress event payload for the cleanup model, mirrored by the
/// frontend hook driving the install card.
#[derive(Serialize, Clone, specta::Type)]
pub struct CleanupModelProgress {
    pub downloaded: u64,
    pub total: u64,
    pub percentage: f64,
}

/// Install failure detail; the UI surfaces it and offers retry.
#[derive(Serialize, Clone, specta::Type)]
pub struct CleanupModelError {
    pub error: String,
}

/// Full install state for one UI render pass.
#[derive(Serialize, Clone, specta::Type)]
pub struct CleanupModelStatus {
    pub model_name: String,
    pub installed: bool,
    pub installing: bool,
    pub ready: bool,
    pub active: bool,
    pub last_error: Option<String>,
}

static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

enum Job {
    Normalize {
        text: String,
        reply: tokio::sync::oneshot::Sender<Option<String>>,
    },
}

pub(crate) fn build_prompt(transcript: &str) -> String {
    format!(
        "<|im_start|>system\n{SYSTEM_PROMPT}<|im_end|>\n\
         <|im_start|>user\n{CONTROL_LINE}\n{transcript}<|im_end|>\n\
         <|im_start|>assistant\n<think>\n\n</think>\n\n"
    )
}

/// Split long transcripts at sentence boundaries so each S1-mini pass stays
/// within the trained input window.
fn chunk_transcript(text: &str) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() <= MAX_CHUNK_WORDS {
        let trimmed = text.trim();
        return if trimmed.is_empty() {
            Vec::new()
        } else {
            vec![trimmed.to_string()]
        };
    }

    let mut sentences: Vec<Vec<&str>> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for word in words {
        current.push(word);
        if word.ends_with('.') || word.ends_with('!') || word.ends_with('?') {
            sentences.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        sentences.push(current);
    }

    let mut chunks: Vec<String> = Vec::new();
    let mut chunk: Vec<String> = Vec::new();
    let mut chunk_len = 0usize;
    for sentence in sentences {
        if chunk_len > 0 && chunk_len + sentence.len() > MAX_CHUNK_WORDS {
            chunks.push(chunk.join(" "));
            chunk.clear();
            chunk_len = 0;
        }
        // A single unbroken run longer than the cap is hard-split by words.
        if sentence.len() > MAX_CHUNK_WORDS {
            for piece in sentence.chunks(MAX_CHUNK_WORDS) {
                chunks.push(
                    piece
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                        .join(" "),
                );
            }
            continue;
        }
        chunk.extend(sentence.iter().map(|s| s.to_string()));
        chunk_len += sentence.len();
    }
    if !chunk.is_empty() {
        chunks.push(chunk.join(" "));
    }
    chunks
}

/// The model is English-only; running it over other languages corrupts them.
/// Explicit `en` always runs; `auto` runs when the text itself reads English.
fn should_run(effective_language: &str, text: &str) -> bool {
    match effective_language {
        "en" => true,
        "auto" | "" => whatlang::detect(text)
            .map(|info| info.lang() == whatlang::Lang::Eng)
            .unwrap_or(false),
        _ => false,
    }
}

/// Clean a transcript. Returns `None` to fail open (model not ready yet or
/// engine error) and `Some(text)` to replace it — `Some("")` is meaningful:
/// filler-only speech normalizes to nothing.
pub(crate) async fn normalize(effective_language: &str, text: String) -> Option<String> {
    if !should_run(effective_language, &text) {
        return None;
    }
    if !READY.load(Ordering::Acquire) {
        warn!("S1-mini not loaded yet; passing transcript through uncleaned");
        return None;
    }
    let tx = JOB_TX
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap()
        .clone()?;
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    tx.send_timeout(
        Job::Normalize {
            text,
            reply: reply_tx,
        },
        std::time::Duration::from_secs(5),
    )
    .await
    .ok()?;
    match tokio::time::timeout(
        std::time::Duration::from_secs(GENERATION_TIMEOUT_SECS),
        reply_rx,
    )
    .await
    {
        Ok(Ok(cleaned)) => cleaned,
        _ => {
            warn!("S1-mini cleanup timed out; passing transcript through uncleaned");
            None
        }
    }
}

/// Resolve where the GGUF lives, matching ModelManager's layout
/// (`app_data_dir/models/<filename>`).
fn model_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    use tauri::Manager;
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir: PathBuf| dir.join("models").join(MODEL_FILENAME))
}

/// True when a fully-downloaded GGUF sits on disk (size-checked; sha256 was
/// verified at install time).
pub fn is_model_installed(app: &tauri::AppHandle) -> bool {
    model_path(app)
        .and_then(|path| std::fs::metadata(&path).ok())
        .is_some_and(|meta| meta.len() == MODEL_SIZE_BYTES)
}

/// One-shot status snapshot for the settings card and onboarding gate.
pub fn status(app: &tauri::AppHandle) -> CleanupModelStatus {
    status_from_state(
        is_model_installed(app),
        INSTALLING.load(Ordering::Acquire),
        READY.load(Ordering::Acquire),
        LAST_ERROR.lock().unwrap().clone(),
    )
}

fn status_from_state(
    installed: bool,
    installing: bool,
    ready: bool,
    last_error: Option<String>,
) -> CleanupModelStatus {
    CleanupModelStatus {
        model_name: MODEL_DISPLAY_NAME.to_string(),
        installed,
        installing,
        ready,
        active: installed && ready,
        last_error,
    }
}

/// Load at startup off the hot path. Installed models load immediately;
/// missing ones auto-download in the background (update case for existing
/// users, first-run race for new users). Progress streams via events so any
/// UI can render live state. Explicit install calls are single-flight-safe.
pub fn preload(app: tauri::AppHandle) {
    if is_model_installed(&app) {
        let Some(path) = model_path(&app) else {
            return;
        };
        start_engine_thread(app, path);
        return;
    }
    info!("S1-mini missing; starting background auto-install");
    install(app);
}

/// True once the engine is loaded and serving normalization jobs. Dictation
/// is gated on this — no transcript is produced before it flips true.
pub fn is_ready() -> bool {
    READY.load(Ordering::Acquire)
}

/// Explicit user-driven install: download → sha256 verify → load engine.
/// Emits `cleanup-model-progress` / `cleanup-model-complete` /
/// `cleanup-model-failed`. Single-flight: concurrent calls are no-ops.
pub fn install(app: tauri::AppHandle) {
    if READY.load(Ordering::Acquire) {
        return;
    }
    if INSTALLING.swap(true, Ordering::AcqRel) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let result = run_install(&app).await;
        INSTALLING.store(false, Ordering::Release);
        match result {
            Ok(()) => {
                *LAST_ERROR.lock().unwrap() = None;
                let _ = app.emit("cleanup-model-complete", ());
            }
            Err(error) => {
                warn!("S1-mini install failed: {error}");
                *LAST_ERROR.lock().unwrap() = Some(error.clone());
                let _ = app.emit("cleanup-model-failed", CleanupModelError { error });
            }
        }
    });
}

async fn run_install(app: &tauri::AppHandle) -> Result<(), String> {
    let path = model_path(app).ok_or_else(|| "no app data dir".to_string())?;

    ensure_model_downloaded(&path, |downloaded| {
        let percentage = (downloaded as f64 / MODEL_SIZE_BYTES as f64) * 100.0;
        let _ = app.emit(
            "cleanup-model-progress",
            CleanupModelProgress {
                downloaded,
                total: MODEL_SIZE_BYTES,
                percentage,
            },
        );
    })
    .await?;

    start_engine_thread(app.clone(), path);
    Ok(())
}

async fn ensure_model_downloaded(
    path: &PathBuf,
    mut on_progress: impl FnMut(u64) + Send,
) -> Result<(), String> {
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() == MODEL_SIZE_BYTES {
            info!("S1-mini model already present ({})", path.display());
            on_progress(MODEL_SIZE_BYTES);
            return Ok(());
        }
        warn!(
            "S1-mini model has wrong size ({}), redownloading",
            meta.len()
        );
        let _ = std::fs::remove_file(path);
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    info!("Downloading S1-mini model from {MODEL_URL}");
    let response = reqwest::get(MODEL_URL)
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("request failed: {e}"))?;
    let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    while let Some(chunk) = futures_util::StreamExt::next(&mut stream).await {
        let chunk = chunk.map_err(|e| format!("stream failed: {e}"))?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;
        // Throttle UI updates to ~10/sec; the final chunk always reports.
        if downloaded % (512 * 1024) < chunk.len() as u64 || downloaded >= MODEL_SIZE_BYTES {
            on_progress(downloaded);
        }
    }
    file.flush().ok();
    drop(file);

    if downloaded != MODEL_SIZE_BYTES {
        let _ = std::fs::remove_file(path);
        return Err(format!(
            "downloaded {downloaded} bytes, expected {MODEL_SIZE_BYTES}"
        ));
    }

    let verify_path = path.clone();
    let actual = tokio::task::spawn_blocking(move || sha256_hex(&verify_path))
        .await
        .map_err(|e| e.to_string())??;
    if actual != MODEL_SHA256 {
        let _ = std::fs::remove_file(path);
        return Err(format!(
            "sha256 mismatch: expected {MODEL_SHA256}, got {actual}"
        ));
    }
    info!(
        "S1-mini model downloaded and verified ({} bytes)",
        downloaded
    );
    Ok(())
}

fn sha256_hex(path: &PathBuf) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn start_engine_thread(app: tauri::AppHandle, model_path: PathBuf) {
    let (tx, rx) = tokio::sync::mpsc::channel::<Job>(16);
    *JOB_TX.get_or_init(|| Mutex::new(None)).lock().unwrap() = Some(tx);
    std::thread::Builder::new()
        .name("s1-cleanup".into())
        .spawn(move || {
            #[cfg(target_os = "macos")]
            engine_loop(app, model_path, rx);
            #[cfg(not(target_os = "macos"))]
            {
                let _ = model_path;
                report_engine_failure(
                    &app,
                    "S1 Mini cleanup is not supported on this platform".to_string(),
                );
                fail_open_loop(rx);
            }
        })
        .expect("failed to spawn s1-cleanup thread");
}

#[cfg(not(target_os = "macos"))]
fn fail_open_loop(mut rx: tokio::sync::mpsc::Receiver<Job>) {
    // No engine on this platform: fail open forever (READY is never set).
    while let Some(job) = rx.blocking_recv() {
        let Job::Normalize { reply, .. } = job;
        let _ = reply.send(None);
    }
}

#[cfg(target_os = "macos")]
fn engine_loop(
    app: tauri::AppHandle,
    model_path: PathBuf,
    mut rx: tokio::sync::mpsc::Receiver<Job>,
) {
    use llama_cpp_2::llama_backend::LlamaBackend;
    use llama_cpp_2::model::params::LlamaModelParams;
    use llama_cpp_2::model::LlamaModel;

    let backend = match LlamaBackend::init() {
        Ok(backend) => backend,
        Err(error) => {
            report_engine_failure(
                &app,
                format!("S1 Mini llama.cpp backend initialization failed: {error}"),
            );
            return;
        }
    };
    let params = LlamaModelParams::default().with_n_gpu_layers(99);
    let model = match LlamaModel::load_from_file(&backend, &model_path, &params) {
        Ok(model) => model,
        Err(error) => {
            report_engine_failure(
                &app,
                format!("S1 Mini failed to load {}: {error}", model_path.display()),
            );
            return;
        }
    };
    info!("S1-mini model loaded from {}", model_path.display());
    READY.store(true, Ordering::Release);
    *LAST_ERROR.lock().unwrap() = None;
    let _ = app.emit("cleanup-model-ready", ());

    while let Some(job) = rx.blocking_recv() {
        let Job::Normalize { text, reply } = job;
        let outcome = match chunk_transcript(&text) {
            chunks if chunks.is_empty() => Some(String::new()),
            chunks => {
                let mut results: Vec<String> = Vec::with_capacity(chunks.len());
                let mut failed = false;
                for chunk in &chunks {
                    match generate(&backend, &model, &build_prompt(chunk)) {
                        Ok(cleaned) => results.push(cleaned),
                        Err(error) => {
                            warn!("S1-mini generation failed; failing open: {error}");
                            failed = true;
                            break;
                        }
                    }
                }
                if failed {
                    None
                } else {
                    Some(results.join(" ").trim().to_string())
                }
            }
        };
        let _ = reply.send(outcome);
    }
}

fn report_engine_failure(app: &tauri::AppHandle, error: String) {
    READY.store(false, Ordering::Release);
    if let Some(sender) = JOB_TX.get() {
        *sender.lock().unwrap() = None;
    }
    warn!("{error}");
    *LAST_ERROR.lock().unwrap() = Some(error.clone());
    let _ = app.emit("cleanup-model-failed", CleanupModelError { error });
}

#[cfg(target_os = "macos")]
fn generate(
    backend: &llama_cpp_2::llama_backend::LlamaBackend,
    model: &llama_cpp_2::model::LlamaModel,
    prompt: &str,
) -> Result<String, String> {
    use llama_cpp_2::context::params::LlamaContextParams;
    use llama_cpp_2::llama_batch::LlamaBatch;
    use llama_cpp_2::model::AddBos;
    use llama_cpp_2::sampling::LlamaSampler;
    use std::num::NonZeroU32;

    const N_CTX: u32 = 2048;

    let mut ctx = model
        .new_context(
            backend,
            LlamaContextParams::default().with_n_ctx(Some(NonZeroU32::new(N_CTX).unwrap())),
        )
        .map_err(|e| e.to_string())?;

    let tokens = model
        .str_to_token(prompt, AddBos::Never)
        .map_err(|e| e.to_string())?;
    if tokens.is_empty() {
        return Ok(String::new());
    }

    // Model-card guidance: output tracks input length; leave room in context.
    let max_new = ((tokens.len() as u32 * 13 / 10 + 32).min(N_CTX - tokens.len() as u32 - 1))
        .max(16) as usize;

    let mut prompt_batch = LlamaBatch::new(tokens.len(), 1);
    for (index, token) in tokens.iter().enumerate() {
        let is_last = index + 1 == tokens.len();
        prompt_batch
            .add(*token, index as i32, &[0], is_last)
            .map_err(|e| e.to_string())?;
    }
    ctx.decode(&mut prompt_batch).map_err(|e| e.to_string())?;

    let mut sampler = LlamaSampler::chain_simple([LlamaSampler::greedy()]);
    let mut generated: Vec<u8> = Vec::with_capacity(max_new * 4);
    let mut generated_tokens = 0usize;
    let mut next_batch = LlamaBatch::new(1, 1);
    let mut position = tokens.len();

    loop {
        let token = sampler.sample(&ctx, -1);
        sampler.accept(token);
        let piece = model
            .token_to_piece_bytes(token, 32, true, None)
            .unwrap_or_default();
        if model.is_eog_token(token) || piece == b"<|im_end|>" {
            break;
        }
        generated.extend_from_slice(&piece);
        generated_tokens += 1;
        if generated_tokens >= max_new {
            break;
        }
        next_batch.clear();
        next_batch
            .add(token, position as i32, &[0], true)
            .map_err(|e| e.to_string())?;
        ctx.decode(&mut next_batch).map_err(|e| e.to_string())?;
        position += 1;
    }

    Ok(String::from_utf8_lossy(&generated).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_matches_documented_format() {
        let prompt = build_prompt("hello world");
        assert!(prompt.starts_with("<|im_start|>system\nYou are a text normalizer"));
        assert!(prompt.contains(CONTROL_LINE));
        assert!(prompt.ends_with("<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"));
    }

    #[test]
    fn short_input_is_one_chunk() {
        assert_eq!(chunk_transcript("one two three"), vec!["one two three"]);
        assert!(chunk_transcript("   ").is_empty());
    }

    #[test]
    fn long_input_chunks_at_sentence_boundaries() {
        let sentence = "this is a moderately long sentence about testing. ";
        let text = sentence.repeat(400); // ~2800 words
        let chunks = chunk_transcript(&text);
        assert!(chunks.len() >= 4);
        for chunk in &chunks {
            assert!(chunk.split_whitespace().count() <= MAX_CHUNK_WORDS);
        }
        assert_eq!(
            chunks.join(" ").split_whitespace().count(),
            text.split_whitespace().count()
        );
    }

    #[test]
    fn language_guard_skips_non_english() {
        assert!(should_run("en", "anything"));
        assert!(should_run("auto", "please fix this transcript"));
        assert!(!should_run("auto", "bonjour le monde c'est magnifique"));
        assert!(!should_run("de", "irgendwas"));
    }

    #[test]
    fn cleanup_model_is_active_only_when_installed_and_ready() {
        let loading = status_from_state(true, false, false, None);
        assert_eq!(loading.model_name, MODEL_DISPLAY_NAME);
        assert!(!loading.active);

        let active = status_from_state(true, false, true, None);
        assert!(active.active);

        let impossible_ready_state = status_from_state(false, false, true, None);
        assert!(!impossible_ready_state.active);
    }
}
