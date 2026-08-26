use crate::audio_feedback::{
    play_feedback_sound, play_feedback_sound_blocking, AiCleanupSound, SoundType,
};
use crate::audio_toolkit::{is_microphone_access_denied, is_no_input_device_error, VadPolicy};
use crate::managers::audio::AudioRecordingManager;
use crate::managers::history::HistoryManager;
use crate::managers::model::ModelManager;
use crate::managers::transcription::StreamWorkKind;
use crate::managers::transcription::TranscriptionManager;
use crate::settings::{get_settings, AppSettings, OverlayStyle};
use crate::shortcut;
use crate::tray::{change_tray_icon, TrayIconState};
use crate::utils::{
    self, show_processing_overlay, show_recording_overlay, show_transcribing_overlay,
};
use crate::TranscriptionCoordinator;
use ferrous_opencc::{config::BuiltinConfig, OpenCC};
use log::{debug, error, warn};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::Manager;
use tauri::{AppHandle, Emitter};

const CANCELLATION_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Clone, serde::Serialize)]
struct RecordingErrorEvent {
    error_type: String,
    detail: Option<String>,
}

/// Drop guard that notifies the [`TranscriptionCoordinator`] when the
/// transcription pipeline finishes — whether it completes normally or panics.
struct FinishGuard(AppHandle);
impl Drop for FinishGuard {
    fn drop(&mut self) {
        if let Some(c) = self.0.try_state::<TranscriptionCoordinator>() {
            c.notify_processing_finished();
        }
        // The session is truly over on every exit path (success, cancel,
        // error, panic) — disarm the raw-Escape watcher only here, since
        // unregistering the cancel shortcut at trigger-key release happens
        // while this pipeline is still running.
        crate::escape_cancel::set_session_active(false);
        crate::escape_cancel::set_hands_free_active(false);
        // The pipeline just freed its large transient buffers (captured PCM,
        // WAV copy, engine scratch); hand the cached pages back to the OS so
        // they don't sit in malloc arenas until they get swapped out (#1792).
        crate::memory::trim_freed_memory();
    }
}

// Shortcut Action Trait
pub trait ShortcutAction: Send + Sync {
    fn start(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str);
    fn stop(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str);
}

// Transcribe Action
struct TranscribeAction {
    post_process: bool,
}

/// Field name for structured output JSON schema
const TRANSCRIPTION_FIELD: &str = "transcription";

/// Strip invisible Unicode characters that some LLMs may insert
fn strip_invisible_chars(s: &str) -> String {
    s.replace(['\u{200B}', '\u{200C}', '\u{200D}', '\u{FEFF}'], "")
}

/// Strip a leading `<think>...</think>` block. Some endpoints can't disable
/// reasoning, and some local servers put the reasoning text into `content`
/// instead of a separate field — without this the user would get the model's
/// chain of thought pasted along with the cleaned transcription.
fn strip_think_block(s: &str) -> &str {
    if let Some(rest) = s.trim_start().strip_prefix("<think>") {
        if let Some(end) = rest.find("</think>") {
            return rest[end + "</think>".len()..].trim_start();
        }
    }
    s
}

/// Build a system prompt from the user's prompt template.
/// Removes `${output}` placeholder since the transcription is sent as the user message.
fn build_system_prompt(prompt_template: &str) -> String {
    prompt_template.replace("${output}", "").trim().to_string()
}

/// Returns `true` when a transcription has no meaningful content to
/// post-process (empty or whitespace-only). Used to skip the post-processing
/// LLM call when nothing was actually transcribed, which would otherwise make
/// the model reply with an error message such as "you need to provide the
/// transcription".
fn is_blank_transcription(transcription: &str) -> bool {
    transcription.trim().is_empty()
}

async fn complete_unless_cancelled<F, C>(operation: F, is_cancelled: C) -> Option<F::Output>
where
    F: Future,
    C: Fn() -> bool,
{
    tokio::pin!(operation);

    loop {
        if is_cancelled() {
            return None;
        }

        if let Ok(result) =
            tokio::time::timeout(CANCELLATION_POLL_INTERVAL, operation.as_mut()).await
        {
            return Some(result);
        }
    }
}

fn should_use_streaming_overlay(style: OverlayStyle, is_streaming: bool) -> bool {
    style == OverlayStyle::Live && is_streaming
}

fn should_enter_edit_mode(binding_id: &str, post_process: bool, has_selection: bool) -> bool {
    binding_id == "transcribe" && !post_process && has_selection
}

fn selection_is_unchanged(
    original: &crate::context::capture::SelectionSnapshot,
    current: Option<&crate::context::capture::SelectionSnapshot>,
) -> bool {
    current.is_some_and(|current| current.pid == original.pid && current.text == original.text)
}

async fn post_process_transcription(settings: &AppSettings, transcription: &str) -> Option<String> {
    if is_blank_transcription(transcription) {
        debug!("Post-processing skipped because the transcription is empty");
        return None;
    }

    let provider = match settings.active_post_process_provider().cloned() {
        Some(provider) => provider,
        None => {
            debug!("Post-processing enabled but no provider is selected");
            return None;
        }
    };

    let model = settings
        .post_process_models
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    if model.trim().is_empty() {
        debug!(
            "Post-processing skipped because provider '{}' has no model configured",
            provider.id
        );
        return None;
    }

    let selected_prompt_id = match &settings.post_process_selected_prompt_id {
        Some(id) => id.clone(),
        None => {
            debug!("Post-processing skipped because no prompt is selected");
            return None;
        }
    };

    let prompt = match settings
        .post_process_prompts
        .iter()
        .find(|prompt| prompt.id == selected_prompt_id)
    {
        Some(prompt) => prompt.prompt.clone(),
        None => {
            debug!(
                "Post-processing skipped because prompt '{}' was not found",
                selected_prompt_id
            );
            return None;
        }
    };

    if prompt.trim().is_empty() {
        debug!("Post-processing skipped because the selected prompt is empty");
        return None;
    }

    debug!(
        "Starting LLM post-processing with provider '{}' (model: {})",
        provider.id, model
    );

    let api_key = settings
        .post_process_api_keys
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    // Ask these providers to skip reasoning/thinking — post-processing rarely
    // benefits from it and it adds seconds of latency. llm_client picks the
    // field the endpoint understands and retries without it if rejected.
    let disable_reasoning = matches!(provider.id.as_str(), "custom" | "openrouter");

    if provider.supports_structured_output {
        debug!("Using structured outputs for provider '{}'", provider.id);

        let system_prompt = build_system_prompt(&prompt);
        let user_content = transcription.to_string();

        // Define JSON schema for transcription output
        let json_schema = serde_json::json!({
            "type": "object",
            "properties": {
                (TRANSCRIPTION_FIELD): {
                    "type": "string",
                    "description": "The cleaned and processed transcription text"
                }
            },
            "required": [TRANSCRIPTION_FIELD],
            "additionalProperties": false
        });

        match crate::llm_client::send_chat_completion_with_schema(
            &provider,
            api_key.clone(),
            &model,
            user_content,
            Some(system_prompt),
            Some(json_schema),
            disable_reasoning,
        )
        .await
        {
            Ok(Some(content)) => {
                // Parse the JSON response to extract the transcription field
                let content = strip_think_block(&content);
                match serde_json::from_str::<serde_json::Value>(content) {
                    Ok(json) => {
                        if let Some(transcription_value) =
                            json.get(TRANSCRIPTION_FIELD).and_then(|t| t.as_str())
                        {
                            let result = strip_invisible_chars(transcription_value);
                            debug!(
                                "Structured output post-processing succeeded for provider '{}'. Output length: {} chars",
                                provider.id,
                                result.len()
                            );
                            return Some(result);
                        } else {
                            error!("Structured output response missing 'transcription' field");
                            return Some(strip_invisible_chars(content));
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to parse structured output JSON: {}. Returning raw content.",
                            e
                        );
                        return Some(strip_invisible_chars(content));
                    }
                }
            }
            Ok(None) => {
                error!("LLM API response has no content");
                return None;
            }
            Err(e) => {
                warn!(
                    "Structured output failed for provider '{}': {}. Falling back to legacy mode.",
                    provider.id, e
                );
                // Fall through to legacy mode below
            }
        }
    }

    // Legacy mode: Replace ${output} variable in the prompt with the actual text
    let processed_prompt = prompt.replace("${output}", transcription);
    debug!("Processed prompt length: {} chars", processed_prompt.len());

    match crate::llm_client::send_chat_completion(
        &provider,
        api_key,
        &model,
        processed_prompt,
        disable_reasoning,
    )
    .await
    {
        Ok(Some(content)) => {
            let content = strip_invisible_chars(strip_think_block(&content));
            debug!(
                "LLM post-processing succeeded for provider '{}'. Output length: {} chars",
                provider.id,
                content.len()
            );
            Some(content)
        }
        Ok(None) => {
            error!("LLM API response has no content");
            None
        }
        Err(e) => {
            error!(
                "LLM post-processing failed for provider '{}': {}. Falling back to original transcription.",
                provider.id,
                e
            );
            None
        }
    }
}

async fn maybe_convert_chinese_variant(
    effective_language: &str,
    transcription: &str,
) -> Option<String> {
    // Gate on the language the model actually transcribed in (the effective
    // language), not the persisted intent. A leftover zh-Hans/zh-Hant intent
    // from a previously selected model must not run OpenCC S2T/T2S over output a
    // non-Chinese model produced — that would silently rewrite any shared CJK
    // characters (e.g. Japanese kanji) in the result.
    let is_simplified = effective_language == "zh-Hans";
    let is_traditional = effective_language == "zh-Hant";

    if !is_simplified && !is_traditional {
        debug!("effective language is not Simplified or Traditional Chinese; skipping conversion");
        return None;
    }

    debug!(
        "Starting Chinese variant conversion using OpenCC for language: {}",
        effective_language
    );

    // Use OpenCC to convert based on selected language
    let config = if is_simplified {
        // Convert Traditional Chinese to Simplified Chinese
        BuiltinConfig::Tw2sp
    } else {
        // Convert Simplified Chinese to Traditional Chinese
        BuiltinConfig::S2tw
    };

    match OpenCC::from_config(config) {
        Ok(converter) => {
            let converted = converter.convert(transcription);
            debug!(
                "OpenCC translation completed. Input length: {}, Output length: {}",
                transcription.len(),
                converted.len()
            );
            Some(converted)
        }
        Err(e) => {
            error!("Failed to initialize OpenCC converter: {}. Falling back to original transcription.", e);
            None
        }
    }
}

pub(crate) struct ProcessedTranscription {
    pub final_text: String,
    pub post_processed_text: Option<String>,
    pub post_process_prompt: Option<String>,
}

/// Resolve the persisted language *intent* into the language the currently-loaded
/// model will actually use — the same capability-aware coercion the transcription
/// paths apply (see [`crate::managers::model::effective_language`]). Post-processing
/// resolves it independently so it agrees with the language the transcription ran
/// in, without threading a value through the pipeline.
fn resolve_effective_language(app: &AppHandle, settings: &AppSettings) -> String {
    let tm = app.state::<Arc<TranscriptionManager>>();
    let model_manager = app.state::<Arc<ModelManager>>();
    let active_model = tm
        .get_current_model()
        .unwrap_or_else(|| settings.selected_model.clone());
    match model_manager.get_model_info(&active_model) {
        Some(info) => crate::managers::model::effective_language(
            &settings.selected_language,
            &info.supported_languages,
            info.supports_language_detection,
        ),
        None => settings.selected_language.clone(),
    }
}

pub(crate) async fn process_transcription_output(
    app: &AppHandle,
    transcription: &str,
    post_process: bool,
) -> ProcessedTranscription {
    process_transcription_output_with_context(app, transcription, post_process, None).await
}

async fn apply_local_transcript_cleanup(
    app: &AppHandle,
    transcription: &str,
    settings: &AppSettings,
) -> String {
    let effective_language = resolve_effective_language(app, settings);
    let mut text = transcription.to_string();
    if let Some(converted_text) = maybe_convert_chinese_variant(&effective_language, &text).await {
        text = converted_text;
    }

    if settings.cleanup_model_enabled && !text.trim().is_empty() {
        let outcome = match crate::local_cleanup::finalize_session(app, &effective_language, &text)
            .await
        {
            Some(outcome) => outcome,
            None => crate::local_cleanup::normalize(app, &effective_language, text.clone()).await,
        };
        debug!(
            "cleanup run {}: {:?} via {:?} ({} -> {} chars)",
            outcome.summary.run_id,
            outcome.summary.lifecycle,
            outcome.summary.final_source,
            text.len(),
            outcome.final_text.len()
        );
        text = outcome.final_text;
    }

    text
}

async fn process_transcription_output_with_context(
    app: &AppHandle,
    transcription: &str,
    post_process: bool,
    context: Option<&crate::context::RecordingContext>,
) -> ProcessedTranscription {
    let settings = get_settings(app);
    let mut final_text = apply_local_transcript_cleanup(app, transcription, &settings).await;
    let mut post_processed_text: Option<String> = None;
    let mut post_process_prompt: Option<String> = None;

    // Smart file references: resolve spoken file names against the active dev
    // project when dictating into a terminal or editor. Local-only, best-effort.
    if settings.smart_file_references_enabled {
        let project = context
            .as_ref()
            .and_then(|context| context.project_root.clone());
        if let Some(resolved) = context
            .as_ref()
            .and_then(|context| context.project_root.as_deref())
            .and_then(|root| crate::file_refs::resolve_references(root, &final_text))
        {
            debug!(
                "Smart file reference resolved in {} characters",
                final_text.len()
            );
            final_text = resolved;
        }
        // Deterministic code-context enhancement (inline only): spoken
        // symbol names -> exact identifiers from resolved files; error
        // wording + captured terminal diagnostic -> one evidence line.
        if let Some(root) = project {
            let focused_buffer = context
                .as_ref()
                .and_then(|c| c.snapshot.focused_text.as_deref());
            if let Some(enhanced) =
                crate::code_context::maybe_enhance(&root, &final_text, focused_buffer)
            {
                debug!("code_context enhanced transcript inline");
                final_text = enhanced;
            }
        }
    }

    // User-defined shortcuts: expand spoken references ("my design prompt",
    // "work email") into their stored content. Deterministic, local, and
    // skipped entirely when the user has no shortcuts configured.
    if !settings.shortcuts.is_empty() {
        if let Some(expanded) = crate::shortcuts::expand_shortcuts(&settings.shortcuts, &final_text)
        {
            debug!("Shortcuts expanded in {} characters", final_text.len());
            final_text = expanded;
        }
    }

    // Terminal/Editor context awareness is intentionally minimal: only
    // deterministic file-reference resolution (smart_file_references) is
    // applied. LLM-based awareness is reserved for Gmail/Slack where it
    // composes finished prose. This prevents Ghostty from pasting
    // hallucinated engineering prompts when the user simply said
    // "fix hero dot tsx".
    if settings.intelligence_awareness_enabled {
        if let Some(context) = context.filter(|context| {
            matches!(
                context.snapshot.surface,
                crate::context::types::Surface::Gmail | crate::context::types::Surface::Slack
            )
        }) {
            match crate::intelligence::compose_aware_reply(
                &settings,
                &context.snapshot,
                context.developer.as_ref(),
                &final_text,
            )
            .await
            {
                crate::intelligence::AwarenessOutcome::Composed(text) => {
                    debug!(
                        "Awareness composed {} characters for {}",
                        text.len(),
                        context.snapshot.surface.as_str()
                    );
                    final_text = text;
                }
                crate::intelligence::AwarenessOutcome::Skipped(reason) => {
                    debug!("Awareness skipped: {reason}");
                }
                crate::intelligence::AwarenessOutcome::Failed(error) => {
                    warn!("Awareness composition failed; using deterministic output: {error}");
                }
            }
        }
    }

    if settings.auto_ai_cleanup_enabled && !final_text.trim().is_empty() {
        match crate::ai_cleanup::clean(&final_text, &settings).await {
            Ok(cleaned) => {
                let input = final_text.clone();
                final_text = cleaned.clone();
                post_processed_text = Some(cleaned);
                let history = app.state::<Arc<HistoryManager>>();
                if let Err(error) = history.save_ai_cleanup(
                    "transcription",
                    &input,
                    &final_text,
                    &settings.ai_cleanup_model,
                    crate::ai_cleanup::thinking_level_name(settings.ai_cleanup_thinking_level),
                ) {
                    warn!("Failed to save automatic AI cleanup history: {error}");
                }
            }
            Err(error) => {
                warn!("Automatic AI cleanup failed; using original transcript: {error}");
            }
        }
    }

    if post_process {
        if let Some(processed_text) = post_process_transcription(&settings, &final_text).await {
            post_processed_text = Some(processed_text.clone());
            final_text = processed_text;

            if let Some(prompt_id) = &settings.post_process_selected_prompt_id {
                if let Some(prompt) = settings
                    .post_process_prompts
                    .iter()
                    .find(|prompt| &prompt.id == prompt_id)
                {
                    post_process_prompt = Some(prompt.prompt.clone());
                }
            }
        }
    }

    if post_processed_text.is_none() && final_text != transcription {
        post_processed_text = Some(final_text.clone());
    }

    ProcessedTranscription {
        final_text,
        post_processed_text,
        post_process_prompt,
    }
}

/// A streamed transcript far below the audio's plausible word rate means the
/// model under-decoded even without a native truncation flag. The caller
/// re-derives the text from the full recording instead of pasting a stub.
/// ~15 words/minute sits far below even slow dictation, so genuine short
/// utterances and silence-heavy recordings never trip it.
fn is_implausibly_short_transcript(text: &str, sample_count: usize) -> bool {
    let minutes = sample_count as f64 / 16_000.0 / 60.0;
    if minutes < 2.0 {
        return false;
    }
    let words = text.split_whitespace().count() as f64;
    words < minutes * 15.0
}

fn save_edit_recording(
    history: &HistoryManager,
    wav_saved: bool,
    file_name: &str,
    instruction: &str,
    output: Option<String>,
    sample_count: usize,
) {
    if !wav_saved {
        return;
    }
    if let Err(error) = history.save_entry(
        file_name.to_string(),
        instruction.to_string(),
        true,
        output,
        None,
        sample_count as f64 / 16_000.0,
    ) {
        error!("Failed to save edit-mode history entry: {error}");
    }
}

async fn run_edit_mode(
    app: &AppHandle,
    recording_manager: &Arc<AudioRecordingManager>,
    history: &Arc<HistoryManager>,
    selection: crate::context::capture::SelectionSnapshot,
    raw_instruction: String,
    file_name: String,
    wav_saved: bool,
    sample_count: usize,
    cancel_generation: u64,
) {
    let settings = get_settings(app);
    let Some(instruction) = complete_unless_cancelled(
        apply_local_transcript_cleanup(app, &raw_instruction, &settings),
        || recording_manager.was_cancelled_since(cancel_generation),
    )
    .await
    else {
        crate::overlay::hide_recording_overlay(app);
        change_tray_icon(app, TrayIconState::Idle);
        return;
    };

    if instruction.trim().is_empty() {
        save_edit_recording(
            history,
            wav_saved,
            &file_name,
            &raw_instruction,
            None,
            sample_count,
        );
        crate::audio_feedback::play_ai_cleanup_sound(app, AiCleanupSound::Error);
        crate::overlay::show_ai_cleanup_notice(
            app,
            "Please say how you want the selected text edited.".to_string(),
            "Edit instruction".to_string(),
            "warning",
        );
        change_tray_icon(app, TrayIconState::Idle);
        return;
    }

    if AI_CLEANUP_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        save_edit_recording(
            history,
            wav_saved,
            &file_name,
            &raw_instruction,
            None,
            sample_count,
        );
        crate::audio_feedback::play_ai_cleanup_sound(app, AiCleanupSound::Error);
        crate::overlay::show_ai_cleanup_notice(
            app,
            "Another Gemini edit is already running.".to_string(),
            "Busy".to_string(),
            "warning",
        );
        change_tray_icon(app, TrayIconState::Idle);
        return;
    }
    let flight_guard = AiCleanupFlightGuard;

    crate::audio_feedback::play_ai_cleanup_sound(app, AiCleanupSound::Trigger);
    crate::overlay::show_editing_overlay(app);
    let output_result = complete_unless_cancelled(
        crate::ai_cleanup::edit(&selection.text, &instruction, &settings),
        || recording_manager.was_cancelled_since(cancel_generation),
    )
    .await;
    drop(flight_guard);

    let Some(output_result) = output_result else {
        crate::overlay::hide_recording_overlay(app);
        change_tray_icon(app, TrayIconState::Idle);
        return;
    };

    let output = match output_result {
        Ok(output) => output,
        Err(error) => {
            save_edit_recording(
                history,
                wav_saved,
                &file_name,
                &raw_instruction,
                None,
                sample_count,
            );
            warn!("Edit mode Gemini request failed: {error}");
            crate::audio_feedback::play_ai_cleanup_sound(app, AiCleanupSound::Error);
            let badge = if crate::ai_cleanup::is_missing_api_key_error(&error) {
                "API key"
            } else {
                "Unavailable"
            };
            crate::overlay::show_ai_cleanup_notice(app, error, badge.to_string(), "error");
            change_tray_icon(app, TrayIconState::Idle);
            return;
        }
    };

    save_edit_recording(
        history,
        wav_saved,
        &file_name,
        &raw_instruction,
        Some(output.clone()),
        sample_count,
    );
    if let Err(error) = history.save_ai_cleanup(
        "edit",
        &selection.text,
        &output,
        &settings.ai_cleanup_model,
        crate::ai_cleanup::thinking_level_name(settings.ai_cleanup_thinking_level),
    ) {
        warn!("Failed to save edit-mode Gemini history: {error}");
    }

    if recording_manager.was_cancelled_since(cancel_generation) {
        crate::overlay::hide_recording_overlay(app);
        change_tray_icon(app, TrayIconState::Idle);
        return;
    }
    if crate::secure_input::is_enabled_now() {
        crate::overlay::hide_recording_overlay(app);
        change_tray_icon(app, TrayIconState::Idle);
        return;
    }

    let current =
        tauri::async_runtime::spawn_blocking(|| crate::context::capture::capture_selected_text())
            .await
            .ok()
            .flatten();
    if !selection_is_unchanged(&selection, current.as_ref()) {
        crate::audio_feedback::play_ai_cleanup_sound(app, AiCleanupSound::Complete);
        crate::overlay::show_result_overlay(app, output);
        change_tray_icon(app, TrayIconState::Idle);
        return;
    }

    let (paste_done, paste_finished) = tokio::sync::oneshot::channel();
    let app_for_paste = app.clone();
    let fallback = output.clone();
    let schedule_fallback = output.clone();
    let recording_manager = Arc::clone(recording_manager);
    if let Err(error) = app.run_on_main_thread(move || {
        if recording_manager.was_cancelled_since(cancel_generation)
            || crate::secure_input::is_enabled_now()
        {
            crate::overlay::hide_recording_overlay(&app_for_paste);
            change_tray_icon(&app_for_paste, TrayIconState::Idle);
            let _ = paste_done.send(());
            return;
        }

        match crate::clipboard::paste_exact(output, app_for_paste.clone()) {
            Ok(()) => {
                crate::audio_feedback::play_ai_cleanup_sound(
                    &app_for_paste,
                    AiCleanupSound::Complete,
                );
                crate::overlay::hide_recording_overlay(&app_for_paste);
            }
            Err(error) => {
                warn!("Edit mode paste failed: {error}");
                crate::audio_feedback::play_ai_cleanup_sound(&app_for_paste, AiCleanupSound::Error);
                crate::overlay::show_result_overlay(&app_for_paste, fallback);
            }
        }
        change_tray_icon(&app_for_paste, TrayIconState::Idle);
        let _ = paste_done.send(());
    }) {
        error!("Failed to schedule edit-mode paste: {error}");
        crate::overlay::show_result_overlay(app, schedule_fallback);
        change_tray_icon(app, TrayIconState::Idle);
        return;
    }
    let _ = paste_finished.await;
}

impl ShortcutAction for TranscribeAction {
    fn start(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        let start_time = Instant::now();
        debug!("TranscribeAction::start called for binding: {}", binding_id);
        let is_hands_free = binding_id == crate::transcription_coordinator::HANDS_FREE_BINDING_ID;

        // The optional S1-mini clean-up stage only constrains dictation when
        // the user explicitly enabled it: then the engine must be live before
        // any speech is captured, and while it downloads/loads recording is
        // refused with a distinct error type the UI maps to a toast. With the
        // model disabled (the default) dictation never waits on it.
        if get_settings(app).cleanup_model_enabled && !crate::local_cleanup::is_ready() {
            debug!("Dictation refused: clean-up model not ready yet");
            let _ = app.emit(
                "recording-error",
                serde_json::json!({
                    "error_type": "cleanup_model_not_ready",
                    "detail": "text cleanup model is still installing"
                }),
            );
            return;
        }

        // Load model in the background
        let tm = app.state::<Arc<TranscriptionManager>>();
        let rm = app.state::<Arc<AudioRecordingManager>>();

        // Edit Mode must snapshot the selection before model, overlay, and
        // microphone setup. This matches the already-working Command+1 path
        // and prevents browser focus/Secure Input changes from erasing it.
        if binding_id == "transcribe" {
            rm.begin_edit_selection_capture();
        } else {
            rm.clear_edit_selection_capture();
        }

        // Load ASR model and VAD model in parallel
        let kickoff_started = Instant::now();
        tm.initiate_model_load();
        let rm_clone = Arc::clone(&rm);
        std::thread::spawn(move || {
            if let Err(e) = rm_clone.preload_vad() {
                debug!("VAD pre-load failed: {}", e);
            }
        });
        let kickoff_elapsed = kickoff_started.elapsed();

        let binding_id = binding_id.to_string();
        let tray_started = Instant::now();
        change_tray_icon(app, TrayIconState::Recording);
        let tray_elapsed = tray_started.elapsed();

        // Get the microphone mode to determine audio feedback timing
        let plan_started = Instant::now();
        let settings = get_settings(app);
        let cleanup_language = resolve_effective_language(app, &settings);
        if settings.cleanup_model_enabled {
            crate::local_cleanup::start_session(app, &cleanup_language);
        }
        let is_always_on = settings.always_on_microphone;

        let selected_model_info = app
            .state::<Arc<ModelManager>>()
            .get_model_info(&settings.selected_model);

        // Use the app-facing model capability as the single pre-recording source
        // for live streaming decisions. Unknown support is represented as false
        // until the model registry is updated by discovery or runtime load.
        let model_supports_streaming = selected_model_info
            .as_ref()
            .map(|m| m.supports_streaming)
            .unwrap_or(false);
        let vad_policy = if !settings.vad_enabled {
            VadPolicy::Disabled
        } else if model_supports_streaming {
            VadPolicy::Streaming
        } else {
            VadPolicy::Offline
        };
        if model_supports_streaming {
            tm.start_stream();
        }
        let plan_elapsed = plan_started.elapsed();

        // Sizing the overlay follows the same advertised capability. A model that
        // doesn't stream (or whose capability is not known yet) gets the compact
        // pill instead of an oversized transparent live window.
        let overlay_started = Instant::now();
        match settings.overlay_style {
            OverlayStyle::Live | OverlayStyle::Minimal if is_hands_free => {
                crate::overlay::show_hands_free_overlay(app)
            }
            OverlayStyle::Live if model_supports_streaming => utils::show_streaming_overlay(app),
            OverlayStyle::Live | OverlayStyle::Minimal => show_recording_overlay(app),
            OverlayStyle::None => {}
        }
        // Everything above runs before capture can begin, so each span here is
        // added keypress->capture latency.
        debug!(
            "start-path pre-recording steps: model_kickoff={:?} tray={:?} settings+stream_plan={:?} overlay={:?}",
            kickoff_elapsed,
            tray_elapsed,
            plan_elapsed,
            overlay_started.elapsed()
        );
        debug!("Microphone mode - always_on: {}", is_always_on);

        let mut recording_error: Option<String> = None;
        let recording_start_time = Instant::now();

        // Crash-durability journal: decide the recording's identity now so
        // the sidecar, the final WAV and the history row all share one name.
        // If the app dies mid-dictation the sidecar survives on disk and
        // startup recovery converts it into a real entry.
        let journal_stem = format!("superflow-{}", chrono::Utc::now().timestamp());
        let journal_path = app
            .state::<Arc<HistoryManager>>()
            .recordings_dir()
            .join(format!("{journal_stem}.f32part"));

        match rm.try_start_recording(&binding_id, vad_policy, Some(journal_path)) {
            Ok(readiness) => {
                rm.begin_context_capture(
                    settings.smart_file_references_enabled
                        || settings.intelligence_awareness_enabled,
                    settings.intelligence_awareness_enabled,
                );
                debug!(
                    "Recording request accepted in {:?}; waiting for first microphone samples",
                    recording_start_time.elapsed()
                );
                let generation = readiness.generation();
                let app_clone = app.clone();
                let rm_clone = Arc::clone(&rm);
                std::thread::spawn(move || {
                    if !readiness.wait() {
                        debug!("Microphone readiness wait ended without receiving samples");
                        return;
                    }

                    // Development-only preview hook for evaluating the brief
                    // arming animation on hardware that normally starts too fast
                    // to make it visible.
                    #[cfg(debug_assertions)]
                    if let Ok(delay_ms) = std::env::var("SUPERFLOW_DEBUG_MIC_READY_DELAY_MS")
                        .unwrap_or_default()
                        .parse::<u64>()
                    {
                        let delay_ms = delay_ms.min(10_000);
                        if delay_ms > 0 {
                            debug!("Delaying microphone-ready cue by {delay_ms}ms for UI preview");
                            std::thread::sleep(Duration::from_millis(delay_ms));
                        }
                    }

                    if !rm_clone.is_recording_readiness_current(generation) {
                        debug!("Microphone became ready for an inactive recording");
                        return;
                    }

                    debug!("Microphone is receiving samples; recording is ready");
                    utils::emit_recording_ready(&app_clone);

                    // The start chime is a readiness cue, so it must follow the
                    // first real input callback rather than Stream::play() or a
                    // fixed delay. The helper returns immediately when feedback
                    // is disabled; mute still follows the same readiness point.
                    if rm_clone.is_recording_readiness_current(generation) {
                        play_feedback_sound_blocking(&app_clone, SoundType::Start);
                    }
                    if rm_clone.is_recording_readiness_current(generation) {
                        rm_clone.apply_mute();
                    }
                });
            }
            Err(e) => {
                debug!("Failed to start recording: {}", e);
                rm.clear_edit_selection_capture();
                recording_error = Some(e);
            }
        }

        if recording_error.is_none() {
            // Dynamically register the cancel shortcut in a separate task to avoid deadlock
            shortcut::register_cancel_shortcut(app);
        } else {
            // Starting failed (for example due to blocked microphone permissions).
            // Revert UI state so we don't stay stuck in the recording overlay.
            tm.cancel_stream();
            utils::hide_recording_overlay(app);
            change_tray_icon(app, TrayIconState::Idle);
            if let Some(err) = recording_error {
                let error_type = if is_microphone_access_denied(&err) {
                    "microphone_permission_denied"
                } else if is_no_input_device_error(&err) {
                    "no_input_device"
                } else {
                    "unknown"
                };
                let _ = app.emit(
                    "recording-error",
                    RecordingErrorEvent {
                        error_type: error_type.to_string(),
                        detail: Some(err),
                    },
                );
            }
        }

        debug!(
            "TranscribeAction::start completed in {:?}",
            start_time.elapsed()
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str) {
        // Prevent a slow microphone from emitting a ready event or start chime
        // after the user has already requested stop.
        app.state::<Arc<AudioRecordingManager>>()
            .invalidate_recording_readiness();

        // Unregister the cancel shortcut when transcription stops
        shortcut::unregister_cancel_shortcut(app);

        let stop_time = Instant::now();
        debug!("TranscribeAction::stop called for binding: {}", binding_id);

        let ah = app.clone();
        let rm = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());
        let tm = Arc::clone(&app.state::<Arc<TranscriptionManager>>());
        let hm = Arc::clone(&app.state::<Arc<HistoryManager>>());

        change_tray_icon(app, TrayIconState::Transcribing);
        // Stop should give immediate visual feedback. Live streaming can keep
        // the larger panel, but it still switches from listening to a working
        // spinner while the stream finalizes. Non-streaming paths use the
        // compact transcribing pill (None no-ops in show_*).
        let style = get_settings(app).overlay_style;
        // Capture this before finalizing the stream so every later working state
        // targets the same overlay that was shown for this transcription.
        let is_hands_free = binding_id == crate::transcription_coordinator::HANDS_FREE_BINDING_ID
            || shortcut_str == crate::transcription_coordinator::HANDS_FREE_BINDING_ID;
        let use_streaming_overlay =
            !is_hands_free && should_use_streaming_overlay(style, tm.is_streaming());
        if use_streaming_overlay {
            tm.emit_stream_working(StreamWorkKind::Transcribing);
        } else {
            show_transcribing_overlay(app);
        }

        // Unmute before playing audio feedback so the stop sound is audible
        rm.remove_mute();

        // Play audio feedback for recording stop
        play_feedback_sound(app, SoundType::Stop);

        let binding_id = binding_id.to_string(); // Clone binding_id for the async task
        let post_process = self.post_process;
        let cancel_generation = rm.cancel_generation();

        tauri::async_runtime::spawn(async move {
            let _guard = FinishGuard(ah.clone());
            debug!(
                "Starting async transcription task for binding: {}",
                binding_id
            );

            let stop_recording_time = Instant::now();
            if let Some(samples) = rm.stop_recording(&binding_id, cancel_generation) {
                let recording_context = rm.take_recording_context();
                debug!(
                    "Recording stopped and samples retrieved in {:?}, sample count: {}",
                    stop_recording_time.elapsed(),
                    samples.len()
                );

                if rm.was_cancelled_since(cancel_generation) {
                    debug!("Transcription operation cancelled after recording stop");
                    rm.clear_edit_selection_capture();
                    tm.cancel_stream();
                    utils::hide_recording_overlay(&ah);
                    change_tray_icon(&ah, TrayIconState::Idle);
                    return;
                }

                if samples.is_empty() {
                    debug!("Recording produced no audio samples; skipping persistence");
                    rm.clear_edit_selection_capture();
                    // Tear down any streaming worker so its channel doesn't leak
                    // and block the next start_stream.
                    tm.cancel_stream();
                    utils::hide_recording_overlay(&ah);
                    change_tray_icon(&ah, TrayIconState::Idle);
                } else {
                    // Save WAV concurrently with transcription. The name comes
                    // from the session's journal stem so the history entry
                    // matches what startup recovery would have produced for a
                    // crash; without a stem (journaling unavailable), fall
                    // back to a fresh timestamp.
                    let sample_count = samples.len();
                    let file_name = match rm.take_journal_stem() {
                        Some(stem) => format!("{stem}.wav"),
                        None => format!("superflow-{}.wav", chrono::Utc::now().timestamp()),
                    };
                    let wav_path = hm.recordings_dir().join(&file_name);
                    let wav_path_for_verify = wav_path.clone();
                    let samples_for_wav = samples.clone();
                    let wav_handle = tauri::async_runtime::spawn_blocking(move || {
                        crate::audio_toolkit::save_wav_file(&wav_path, &samples_for_wav)
                    });

                    // Transcribe concurrently with WAV save. If a live stream was
                    // running, finalize it and use its text (all audio was already
                    // fed to the stream); otherwise batch-transcribe the samples.
                    let transcription_time = Instant::now();
                    let transcription_result = match tm.finalize_stream() {
                        // A finalized stream with usable text wins. An empty result
                        // (no active stream, produced nothing, or a finalize error
                        // after the engine was returned) falls back to a full batch
                        // transcription of the same audio. A finalize timeout is
                        // surfaced instead — the worker may still hold the engine,
                        // so a batch fallback would contend with it. Text that is
                        // implausibly short for the recording's length is treated
                        // the same as empty: the model under-decoded, so the full
                        // audio is re-transcribed rather than a stub pasted.
                        Ok(Some(text))
                            if !text.trim().is_empty()
                                && !is_implausibly_short_transcript(&text, sample_count) =>
                        {
                            Ok(text)
                        }
                        Ok(_) => tm.transcribe(samples),
                        Err(err) => Err(err),
                    };

                    // Await WAV save and verify
                    let wav_saved = match wav_handle.await {
                        Ok(Ok(())) => {
                            match crate::audio_toolkit::verify_wav_file(
                                &wav_path_for_verify,
                                sample_count,
                            ) {
                                Ok(()) => true,
                                Err(e) => {
                                    error!("WAV verification failed: {}", e);
                                    false
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            error!("Failed to save WAV file: {}", e);
                            false
                        }
                        Err(e) => {
                            error!("WAV save task panicked: {}", e);
                            false
                        }
                    };

                    if rm.was_cancelled_since(cancel_generation) {
                        debug!("Transcription operation cancelled before output handling");
                        rm.clear_edit_selection_capture();
                        // A finished dictation survives cancellation — stash it
                        // for the cancel toast's Undo instead of dropping it.
                        if let Ok(transcription) = &transcription_result {
                            utils::stash_canceled_transcript(transcription.clone());
                            utils::show_cancel_toast(&ah, true);
                        } else {
                            utils::hide_recording_overlay(&ah);
                        }
                        change_tray_icon(&ah, TrayIconState::Idle);
                        return;
                    }

                    match transcription_result {
                        Ok(transcription) => {
                            debug!(
                                "Transcription completed in {:?} ({} characters)",
                                transcription_time.elapsed(),
                                transcription.len()
                            );

                            if crate::secure_input::is_enabled_now() {
                                rm.clear_edit_selection_capture();
                                if wav_saved {
                                    if let Err(error) = std::fs::remove_file(&wav_path_for_verify) {
                                        warn!("Failed to remove secure-input recording: {error}");
                                    }
                                }
                                debug!(
                                    "Secure Input active; discarded transcription and recording"
                                );
                                utils::hide_recording_overlay(&ah);
                                change_tray_icon(&ah, TrayIconState::Idle);
                                return;
                            }

                            let edit_selection = rm.take_edit_selection();
                            if should_enter_edit_mode(
                                &binding_id,
                                post_process,
                                edit_selection.is_some(),
                            ) {
                                run_edit_mode(
                                    &ah,
                                    &rm,
                                    &hm,
                                    edit_selection.expect("edit selection checked above"),
                                    transcription,
                                    file_name,
                                    wav_saved,
                                    sample_count,
                                    cancel_generation,
                                )
                                .await;
                                return;
                            }

                            // A spoken terminal command ("please open four
                            // claude code ...") consumes the utterance: it
                            // launches a local agent team instead of pasting.
                            if crate::voice_terminal::try_handle_voice_command(&transcription) {
                                debug!("Transcript handled as voice terminal command");
                                utils::hide_recording_overlay(&ah);
                                change_tray_icon(&ah, TrayIconState::Idle);
                                return;
                            }

                            let auto_ai_cleanup = get_settings(&ah).auto_ai_cleanup_enabled;
                            if use_streaming_overlay {
                                tm.emit_stream_working(StreamWorkKind::Finalizing);
                            }
                            if auto_ai_cleanup {
                                crate::audio_feedback::play_ai_cleanup_sound(
                                    &ah,
                                    AiCleanupSound::Trigger,
                                );
                                crate::overlay::show_ai_prompting_overlay(&ah);
                            } else if post_process {
                                if use_streaming_overlay {
                                    tm.emit_stream_working(StreamWorkKind::Polishing);
                                } else {
                                    show_processing_overlay(&ah);
                                }
                            }
                            let Some(processed) = complete_unless_cancelled(
                                process_transcription_output_with_context(
                                    &ah,
                                    &transcription,
                                    post_process,
                                    recording_context.as_ref(),
                                ),
                                || rm.was_cancelled_since(cancel_generation),
                            )
                            .await
                            else {
                                debug!("Transcription operation cancelled during output handling");
                                // Raw transcript exists even though polishing
                                // was abandoned — Undo pastes it.
                                utils::stash_canceled_transcript(transcription.clone());
                                utils::show_cancel_toast(&ah, true);
                                change_tray_icon(&ah, TrayIconState::Idle);
                                return;
                            };

                            if rm.was_cancelled_since(cancel_generation) {
                                debug!("Transcription operation cancelled before paste");
                                utils::stash_canceled_transcript(processed.final_text.clone());
                                utils::show_cancel_toast(&ah, true);
                                change_tray_icon(&ah, TrayIconState::Idle);
                                return;
                            }

                            if auto_ai_cleanup && processed.post_processed_text.is_some() {
                                crate::audio_feedback::play_ai_cleanup_sound(
                                    &ah,
                                    AiCleanupSound::Complete,
                                );
                            }

                            // Save to history if WAV was saved. The recorded
                            // sample count is the real audio length (16 kHz
                            // mono), persisted for WPM / time-saved stats.
                            if wav_saved {
                                if let Err(err) = hm.save_entry(
                                    file_name,
                                    transcription,
                                    post_process || auto_ai_cleanup,
                                    processed.post_processed_text.clone(),
                                    processed.post_process_prompt.clone(),
                                    sample_count as f64 / 16_000.0,
                                ) {
                                    error!("Failed to save history entry: {}", err);
                                }
                            }

                            if processed.final_text.is_empty() {
                                utils::hide_recording_overlay(&ah);
                                change_tray_icon(&ah, TrayIconState::Idle);
                            } else {
                                let ah_clone = ah.clone();
                                let paste_time = Instant::now();
                                let final_text = processed.final_text;
                                let rm_for_paste = Arc::clone(&rm);
                                ah.run_on_main_thread(move || {
                                    if rm_for_paste.was_cancelled_since(cancel_generation) {
                                        debug!("Transcription operation cancelled before paste");
                                        utils::stash_canceled_transcript(final_text);
                                        utils::show_cancel_toast(&ah_clone, true);
                                        change_tray_icon(&ah_clone, TrayIconState::Idle);
                                        return;
                                    }

                                    // Password field focused: never paste and never
                                    // surface the transcript — it may be a secret.
                                    if crate::secure_input::is_enabled_now() {
                                        debug!(
                                            "Secure Input active — discarding transcript quietly"
                                        );
                                        utils::hide_recording_overlay(&ah_clone);
                                        change_tray_icon(&ah_clone, TrayIconState::Idle);
                                        return;
                                    }

                                    match utils::paste(final_text.clone(), ah_clone.clone()) {
                                        Ok(()) => {
                                            // Paste landed: the transcript is where the user
                                            // wanted it. Clean up quietly — no result card.
                                            debug!(
                                                "Text pasted successfully in {:?}",
                                                paste_time.elapsed()
                                            );
                                            utils::hide_recording_overlay(&ah_clone);
                                            change_tray_icon(&ah_clone, TrayIconState::Idle);
                                        }
                                        Err(e) => {
                                            // The paste had nowhere to land. Surface the
                                            // transcript as a copyable result card so a
                                            // finished dictation is never lost.
                                            error!("Failed to paste transcription: {}", e);
                                            let _ = ah_clone.emit("paste-error", ());
                                            utils::show_result_overlay(&ah_clone, final_text);
                                            change_tray_icon(&ah_clone, TrayIconState::Idle);
                                        }
                                    }
                                })
                                .unwrap_or_else(|e| {
                                    error!("Failed to run paste on main thread: {:?}", e);
                                    utils::hide_recording_overlay(&ah);
                                    change_tray_icon(&ah, TrayIconState::Idle);
                                });
                            }
                        }
                        Err(err) => {
                            rm.clear_edit_selection_capture();
                            if rm.was_cancelled_since(cancel_generation) {
                                debug!(
                                    "Transcription operation cancelled after transcription error"
                                );
                                utils::hide_recording_overlay(&ah);
                                change_tray_icon(&ah, TrayIconState::Idle);
                                return;
                            }

                            error!("Transcription failed: {}", err);
                            // Surface the failure to the UI (toast). The full
                            // message is also in superflow.log via the line above.
                            let _ = ah.emit("transcription-error", err.to_string());
                            // Save entry with empty text so it shows up as
                            // failed in history; the user can Retry manually
                            // from there. No automatic re-transcription runs.
                            if wav_saved {
                                if let Err(save_err) = hm.save_entry(
                                    file_name,
                                    String::new(),
                                    post_process,
                                    None,
                                    None,
                                    sample_count as f64 / 16_000.0,
                                ) {
                                    error!("Failed to save failed history entry: {}", save_err);
                                }
                            }
                            utils::hide_recording_overlay(&ah);
                            change_tray_icon(&ah, TrayIconState::Idle);
                        }
                    }
                }
            } else {
                rm.clear_context_capture();
                rm.clear_edit_selection_capture();
                debug!("No samples retrieved from recording stop");
                // Tear down any streaming worker so its channel doesn't leak.
                tm.cancel_stream();
                utils::hide_recording_overlay(&ah);
                change_tray_icon(&ah, TrayIconState::Idle);
            }
        });

        debug!(
            "TranscribeAction::stop completed in {:?}",
            stop_time.elapsed()
        );
    }
}

// Cancel Action
struct CancelAction;

impl ShortcutAction for CancelAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        utils::cancel_current_operation(app);
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        // Nothing to do on stop for cancel
    }
}

// Test Action
struct TestAction;

struct AiCleanupAction;

static AI_CLEANUP_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

struct AiCleanupFlightGuard;

impl Drop for AiCleanupFlightGuard {
    fn drop(&mut self) {
        AI_CLEANUP_IN_FLIGHT.store(false, Ordering::Release);
    }
}

impl ShortcutAction for AiCleanupAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        let settings = crate::settings::get_settings(app);
        if !settings.ai_cleanup_enabled || AI_CLEANUP_IN_FLIGHT.swap(true, Ordering::AcqRel) {
            return;
        }

        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let _guard = AiCleanupFlightGuard;
            crate::audio_feedback::play_ai_cleanup_sound(&app, AiCleanupSound::Trigger);
            let selection = tauri::async_runtime::spawn_blocking(|| {
                crate::context::capture::capture_selected_text()
            })
            .await
            .ok()
            .flatten();

            let Some(selection) = selection else {
                crate::audio_feedback::play_ai_cleanup_sound(&app, AiCleanupSound::Error);
                crate::overlay::show_ai_cleanup_notice(
                    &app,
                    "Please select the text first.".to_string(),
                    "Select text".to_string(),
                    "warning",
                );
                return;
            };

            crate::overlay::show_ai_prompting_overlay(&app);
            let settings = crate::settings::get_settings(&app);
            let output = match crate::ai_cleanup::clean(&selection.text, &settings).await {
                Ok(output) => output,
                Err(error) => {
                    log::warn!("AI clean up failed: {error}");
                    crate::audio_feedback::play_ai_cleanup_sound(&app, AiCleanupSound::Error);
                    let badge = if crate::ai_cleanup::is_missing_api_key_error(&error) {
                        "API key"
                    } else {
                        "Unavailable"
                    };
                    crate::overlay::show_ai_cleanup_notice(&app, error, badge.to_string(), "error");
                    return;
                }
            };

            crate::audio_feedback::play_ai_cleanup_sound(&app, AiCleanupSound::Complete);

            if crate::secure_input::is_enabled_now() {
                crate::overlay::hide_recording_overlay(&app);
                return;
            }

            let history = app.state::<Arc<HistoryManager>>();
            if let Err(error) = history.save_ai_cleanup(
                "selection",
                &selection.text,
                &output,
                &settings.ai_cleanup_model,
                crate::ai_cleanup::thinking_level_name(settings.ai_cleanup_thinking_level),
            ) {
                log::warn!("Failed to save AI cleanup history: {error}");
            }

            let current = tauri::async_runtime::spawn_blocking(|| {
                crate::context::capture::capture_selected_text()
            })
            .await
            .ok()
            .flatten();
            if current.as_ref().map_or(true, |current| {
                current.pid != selection.pid || current.text != selection.text
            }) {
                crate::overlay::show_result_overlay(&app, output);
                return;
            }

            let handle = app.clone();
            let fallback = output.clone();
            let _ = app.run_on_main_thread(move || {
                if crate::secure_input::is_enabled_now() {
                    crate::overlay::hide_recording_overlay(&handle);
                    return;
                }
                if let Err(error) = crate::clipboard::paste_exact(output, handle.clone()) {
                    log::warn!("AI cleanup paste failed: {error}");
                    crate::overlay::show_result_overlay(&handle, fallback);
                } else {
                    crate::overlay::hide_recording_overlay(&handle);
                }
            });
        });
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {}
}

impl ShortcutAction for TestAction {
    fn start(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str) {
        log::info!(
            "Shortcut ID '{}': Started - {} (App: {})", // Changed "Pressed" to "Started" for consistency
            binding_id,
            shortcut_str,
            app.package_info().name
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str) {
        log::info!(
            "Shortcut ID '{}': Stopped - {} (App: {})", // Changed "Released" to "Stopped" for consistency
            binding_id,
            shortcut_str,
            app.package_info().name
        );
    }
}

// Static Action Map
pub static ACTION_MAP: Lazy<HashMap<String, Arc<dyn ShortcutAction>>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert(
        "transcribe".to_string(),
        Arc::new(TranscribeAction {
            post_process: false,
        }) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "transcribe_with_post_process".to_string(),
        Arc::new(TranscribeAction { post_process: true }) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        crate::transcription_coordinator::HANDS_FREE_BINDING_ID.to_string(),
        Arc::new(TranscribeAction {
            post_process: false,
        }) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "cancel".to_string(),
        Arc::new(CancelAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "test".to_string(),
        Arc::new(TestAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "ai_cleanup".to_string(),
        Arc::new(AiCleanupAction) as Arc<dyn ShortcutAction>,
    );
    map
});

#[cfg(test)]
mod tests {
    use super::{
        complete_unless_cancelled, is_blank_transcription, selection_is_unchanged,
        should_enter_edit_mode, should_use_streaming_overlay, strip_think_block,
    };
    use crate::context::capture::SelectionSnapshot;
    use crate::settings::OverlayStyle;
    use std::future;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn blank_transcription_is_detected() {
        assert!(is_blank_transcription(""));
        assert!(is_blank_transcription("   "));
        assert!(is_blank_transcription("\t\n  \r\n"));
    }

    #[test]
    fn non_blank_transcription_is_kept() {
        assert!(!is_blank_transcription("hello"));
        assert!(!is_blank_transcription("  hello  "));
    }

    #[test]
    fn completed_operation_returns_its_output() {
        let result = tauri::async_runtime::block_on(complete_unless_cancelled(
            future::ready("done"),
            || false,
        ));

        assert_eq!(result, Some("done"));
    }

    #[test]
    fn pending_operation_stops_after_cancellation() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancelled_for_thread = Arc::clone(&cancelled);
        let cancel_thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(10));
            cancelled_for_thread.store(true, Ordering::Release);
        });

        let result = tauri::async_runtime::block_on(complete_unless_cancelled(
            future::pending::<()>(),
            || cancelled.load(Ordering::Acquire),
        ));

        cancel_thread.join().unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn leading_think_block_is_stripped() {
        assert_eq!(
            strip_think_block("<think>pondering...</think>Cleaned text."),
            "Cleaned text."
        );
        assert_eq!(
            strip_think_block("  \n<think>multi\nline</think>\n  Cleaned text."),
            "Cleaned text."
        );
    }

    #[test]
    fn content_without_think_block_is_unchanged() {
        assert_eq!(strip_think_block("Cleaned text."), "Cleaned text.");
        assert_eq!(
            strip_think_block("Mentions <think> mid-sentence."),
            "Mentions <think> mid-sentence."
        );
        // Unclosed block: leave untouched rather than guess
        assert_eq!(
            strip_think_block("<think>never closed"),
            "<think>never closed"
        );
    }

    #[test]
    fn live_overlay_uses_streaming_states_only_for_streaming_models() {
        assert!(should_use_streaming_overlay(OverlayStyle::Live, true));
        assert!(!should_use_streaming_overlay(OverlayStyle::Live, false));
        assert!(!should_use_streaming_overlay(OverlayStyle::Minimal, true));
        assert!(!should_use_streaming_overlay(OverlayStyle::None, true));
    }

    #[test]
    fn edit_mode_only_uses_the_standard_transcribe_binding_with_a_selection() {
        assert!(should_enter_edit_mode("transcribe", false, true));
        assert!(!should_enter_edit_mode("transcribe", false, false));
        assert!(!should_enter_edit_mode(
            "transcribe_with_post_process",
            true,
            true
        ));
        assert!(!should_enter_edit_mode(
            "hands_free_transcribe",
            false,
            true
        ));
    }

    #[test]
    fn edit_replacement_requires_the_same_process_and_selected_text() {
        let original = SelectionSnapshot {
            pid: 42,
            app_name: "Editor".to_string(),
            text: "selected words".to_string(),
        };
        assert!(selection_is_unchanged(&original, Some(&original)));

        let mut changed_text = original.clone();
        changed_text.text = "different words".to_string();
        assert!(!selection_is_unchanged(&original, Some(&changed_text)));

        let mut changed_process = original.clone();
        changed_process.pid = 43;
        assert!(!selection_is_unchanged(&original, Some(&changed_process)));
        assert!(!selection_is_unchanged(&original, None));
    }
}
