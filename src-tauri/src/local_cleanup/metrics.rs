//! Privacy-safe cleanup observability (plan T1).
//!
//! Internal [`CleanupResult`] carries final transcript text for paste and is
//! never serialized. Everything serializable here carries only state and
//! metrics — no transcript content ever reaches events, logs, or the frontend.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// Identity of one end-to-end cleanup run, assigned when output handling starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CleanupRunId(pub u64);

static NEXT_RUN_ID: AtomicU64 = AtomicU64::new(1);

impl CleanupRunId {
    pub fn next() -> Self {
        Self(NEXT_RUN_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// Terminal lifecycle of a cleanup run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum CleanupLifecycle {
    /// Full S1 output accepted for every chunk.
    Applied,
    /// At least one chunk fell back to its source span; neighbors kept.
    PartiallyApplied,
    /// No cleanup attempted (non-English or empty input).
    Skipped,
    /// Model output rejected by validation; source text substituted.
    Rejected,
    /// Engine, timeout, queue, or lifecycle failure; source text substituted.
    Failed,
    /// Superseded or cancelled before completion.
    Cancelled,
}

/// Where the pasted text actually came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum CleanupFinalSource {
    S1,
    MixedChunkFallback,
    RawFallback,
    NonEnglishSkip,
}

/// Stage at which a failed run stopped. Stable reason codes — never free text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum CleanupFailureStage {
    NotReady,
    QueueTimeout,
    GenerationTimeout,
    GenerationError,
    ValidationRejected,
    Cancelled,
}

/// Stable per-chunk validation rejection codes (full set enforced from T3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum CleanupValidationReason {
    ThinkTagLeakage,
    RepetitionLoop,
    InventedIdentifier,
    MissingNumericToken,
    MissingCurrencyOrPercentage,
    NegationChanged,
    ImplausibleTruncation,
    EmptyForMeaningfulSpeech,
}

/// Timing and token accounting for one chunk. No text fields, by design.
#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
pub struct CleanupChunkMetrics {
    pub chunk_index: u32,
    pub chunk_count: u32,
    pub queue_wait_ms: f64,
    pub prompt_eval_ms: f64,
    pub generation_ms: f64,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub generated_tokens_per_second: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
pub struct CleanupRunMetrics {
    pub total_ms: f64,
    pub backend: String,
    pub chunks: Vec<CleanupChunkMetrics>,
}

/// Serializable terminal summary for one run: safe for events, status, UI.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct CleanupOutcomeSummary {
    pub run_id: u64,
    pub lifecycle: CleanupLifecycle,
    pub final_source: CleanupFinalSource,
    pub failure_stage: Option<CleanupFailureStage>,
    pub validation_reason: Option<CleanupValidationReason>,
    pub metrics: CleanupRunMetrics,
}

/// Internal-only run result: final paste text plus its public summary.
/// `final_text` must never gain `Serialize`.
pub struct CleanupResult {
    pub final_text: String,
    pub summary: CleanupOutcomeSummary,
}

/// Typed terminal event emitted exactly once per finished run.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct CleanupRunStatusEvent {
    pub summary: CleanupOutcomeSummary,
}

static LAST_RUN: Mutex<Option<CleanupOutcomeSummary>> = Mutex::new(None);

/// Record a terminal run so the status command can report degraded states.
pub fn record_terminal_run(summary: &CleanupOutcomeSummary) {
    *LAST_RUN.lock().unwrap() = Some(summary.clone());
}

/// Latest terminal run summary, if any run has finished this session.
pub fn latest_run() -> Option<CleanupOutcomeSummary> {
    LAST_RUN.lock().unwrap().clone()
}

/// Wall-clock helper shared by stage timers.
#[derive(Debug)]
pub struct StageTimer {
    started: Instant,
}

impl StageTimer {
    pub fn start() -> Self {
        Self {
            started: Instant::now(),
        }
    }

    pub fn elapsed_ms(&self) -> f64 {
        self.started.elapsed().as_secs_f64() * 1000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_summary(lifecycle: CleanupLifecycle) -> CleanupOutcomeSummary {
        CleanupOutcomeSummary {
            run_id: 7,
            lifecycle,
            final_source: CleanupFinalSource::S1,
            failure_stage: None,
            validation_reason: None,
            metrics: CleanupRunMetrics {
                total_ms: 1234.5,
                backend: "metal".into(),
                chunks: vec![CleanupChunkMetrics {
                    chunk_index: 0,
                    chunk_count: 1,
                    queue_wait_ms: 1.0,
                    prompt_eval_ms: 40.0,
                    generation_ms: 900.0,
                    input_tokens: 120,
                    output_tokens: 140,
                    generated_tokens_per_second: 155.5,
                }],
            },
        }
    }

    #[test]
    fn summaries_serialize_without_text_and_with_stable_shapes() {
        for lifecycle in [
            CleanupLifecycle::Applied,
            CleanupLifecycle::PartiallyApplied,
            CleanupLifecycle::Skipped,
            CleanupLifecycle::Rejected,
            CleanupLifecycle::Failed,
            CleanupLifecycle::Cancelled,
        ] {
            let json = serde_json::to_string(&sample_summary(lifecycle)).unwrap();
            assert!(!json.contains("final_text"));
            assert!(json.contains("\"run_id\":7"));
        }
        let applied = serde_json::to_string(&sample_summary(CleanupLifecycle::Applied)).unwrap();
        assert!(applied.contains("\"lifecycle\":\"applied\""));
        assert!(applied.contains("\"final_source\":\"s1\""));
        assert!(applied.contains("\"backend\":\"metal\""));
    }

    #[test]
    fn latest_run_round_trips() {
        let summary = sample_summary(CleanupLifecycle::PartiallyApplied);
        record_terminal_run(&summary);
        assert_eq!(
            latest_run().unwrap().lifecycle,
            CleanupLifecycle::PartiallyApplied
        );
    }

    #[test]
    fn run_ids_are_unique() {
        let a = CleanupRunId::next();
        let b = CleanupRunId::next();
        assert_ne!(a, b);
    }
}
