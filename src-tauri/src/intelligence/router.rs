//! Provider routing for awareness composition: Apple Intelligence first,
//! configured cloud provider (Gemini/OpenAI-compatible) as fallback.

use crate::apple_intelligence;
use crate::llm_client;
use crate::settings::AppSettings;

use super::prompt::build_context_prompts;
use crate::context::types::ContextSnapshot;

const APPLE_MAX_TOKENS: i32 = 1024;

#[derive(Debug)]
pub enum AwarenessOutcome {
    /// Finished text ready to paste.
    Composed(String),
    /// Awareness deliberately not applied (wrong surface, no provider…).
    Skipped(&'static str),
    /// Awareness attempted but both providers failed.
    Failed(String),
}

/// Try to turn the dictated instruction into finished message text using the
/// page context. Never panics, never blocks on more than the model call.
pub async fn compose_aware_reply(
    settings: &AppSettings,
    snapshot: &ContextSnapshot,
    transcript: &str,
) -> AwarenessOutcome {
    let Some((system, user)) = build_context_prompts(snapshot, transcript) else {
        return AwarenessOutcome::Skipped("not_an_aware_context");
    };

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    if apple_intelligence::check_apple_intelligence_availability() {
        let apple_user = user.clone();
        let apple_system = system.clone();
        let attempt = tokio::task::spawn_blocking(move || {
            apple_intelligence::process_text_with_system_prompt(
                &apple_system,
                &apple_user,
                APPLE_MAX_TOKENS,
            )
        })
        .await;

        match attempt {
            Ok(Ok(text)) if !text.trim().is_empty() => {
                return AwarenessOutcome::Composed(text.trim().to_string());
            }
            Ok(Err(e)) => {
                log::warn!("Apple Intelligence compose failed, falling back: {e}");
            }
            Ok(Ok(_)) => {
                log::warn!("Apple Intelligence returned empty output, falling back");
            }
            Err(e) => {
                log::warn!("Apple Intelligence task join failed, falling back: {e}");
            }
        }
    }

    cloud_fallback(settings, &system, &user).await
}

async fn cloud_fallback(
    settings: &AppSettings,
    system: &str,
    user: &str,
) -> AwarenessOutcome {
    // Same credential/model plumbing as regular post-processing — a provider
    // configured for post-processing is reused for awareness.
    let Some(provider) = settings.active_post_process_provider().cloned() else {
        return AwarenessOutcome::Skipped("no_provider_configured");
    };
    let model = settings
        .post_process_models
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();
    if model.trim().is_empty() {
        return AwarenessOutcome::Skipped("no_model_configured");
    }
    let api_key = settings
        .post_process_api_keys
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    match llm_client::send_chat_completion_with_schema(
        &provider,
        api_key,
        &model,
        user.to_string(),
        Some(system.to_string()),
        None,
        false,
    )
    .await
    {
        Ok(Some(text)) if !text.trim().is_empty() => {
            AwarenessOutcome::Composed(text.trim().to_string())
        }
        Ok(_) => AwarenessOutcome::Failed("cloud provider returned no content".to_string()),
        Err(e) => AwarenessOutcome::Failed(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn other_surface_skips_without_touching_providers() {
        let settings = AppSettings::default();
        let snap = ContextSnapshot::other("Finder");
        match compose_aware_reply(&settings, &snap, "hello").await {
            AwarenessOutcome::Skipped(reason) => assert_eq!(reason, "not_an_aware_context"),
            other => panic!("expected skip, got {other:?}"),
        }
    }
}
