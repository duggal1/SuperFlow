//! Provider routing for awareness composition through the configured provider.
//! Gmail/Slack compose via LLM; Ghostty/Terminal and Editor use deterministic
//! file-reference resolution only and skip LLM entirely.

use crate::llm_client;
use crate::settings::AppSettings;

use super::prompt::build_context_prompts;
use super::validation::accept_model_output;
use crate::context::developer::DeveloperContext;
use crate::context::types::ContextSnapshot;

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
    developer_context: Option<&DeveloperContext>,
    transcript: &str,
) -> AwarenessOutcome {
    // Developer surfaces (Ghostty/Terminal, VS Code/Editor) are intentionally
    // excluded from LLM awareness. They use only deterministic local
    // file-reference resolution (smart_file_references) which maps
    // "hero dot tsx" -> "components/.../hero.tsx" without hallucinating.
    // Gmail/Slack remain the only LLM-aware surfaces.
    if matches!(
        snapshot.surface,
        crate::context::types::Surface::Terminal | crate::context::types::Surface::Editor
    ) {
        return AwarenessOutcome::Skipped("developer_surface_uses_file_refs_only");
    }

    let Some((system, user)) = build_context_prompts(snapshot, transcript, developer_context)
    else {
        return AwarenessOutcome::Skipped("not_an_aware_context");
    };

    cloud_fallback(settings, &system, &user, transcript).await
}

async fn cloud_fallback(
    settings: &AppSettings,
    system: &str,
    user: &str,
    transcript: &str,
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
        Ok(Some(text)) if !text.trim().is_empty() => accept_model_output(transcript, &text)
            .map(AwarenessOutcome::Composed)
            .unwrap_or_else(|| {
                AwarenessOutcome::Failed("provider output failed validation".into())
            }),
        Ok(_) => AwarenessOutcome::Failed("cloud provider returned no content".to_string()),
        Err(e) => AwarenessOutcome::Failed(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::types::{ContextSnapshot, Surface};

    #[tokio::test]
    async fn other_surface_skips_without_touching_providers() {
        let settings = AppSettings::default();
        let snap = ContextSnapshot::other("Finder");
        match compose_aware_reply(&settings, &snap, None, "hello").await {
            AwarenessOutcome::Skipped(reason) => assert_eq!(reason, "not_an_aware_context"),
            other => panic!("expected skip, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn developer_surfaces_skip_llm_and_use_file_refs_only() {
        let settings = AppSettings::default();
        for surface in [Surface::Terminal, Surface::Editor] {
            let snap = ContextSnapshot {
                surface,
                ..ContextSnapshot::other("Ghostty")
            };
            match compose_aware_reply(&settings, &snap, None, "fix hero dot tsx").await {
                AwarenessOutcome::Skipped(reason) => {
                    assert_eq!(reason, "developer_surface_uses_file_refs_only")
                }
                other => panic!("expected developer skip, got {other:?}"),
            }
        }
    }
}
