//! Superflow Grammar — deterministic, offline, <50ms (measured, not claimed).
//!
//! Approved architecture (8.5/10):
//! ```text
//! Parakeet
//!   ↓
//! transcript_cleanup.rs (fillers, repeats, restarts — S1-mini off by default)
//!   ↓
//! protected spans (hard safety boundary — Path, Filename, Url, Email, CamelCase,
//!                  PascalCase, SnakeCase, KebabCase, Version, Package, Mention, InlineCode)
//!   ↓
//! Harper curated rules (2.8, ~287 rules — harper-core/src/linting)
//!   + custom dictionary (additional linguistic knowledge — Zustand, Tauri, Parakeet, …)
//!   ↓
//! Superflow custom grammar rules (ExprLinter families — see rules/mod.rs)
//!   ↓
//! restore protected spans (exact bytes)
//!   ↓
//! formatter.rs → Gmail / Slack surface formatting
//! ```
//! Harper `ExprLinter` is the intended extension mechanism and gets aggressive
//! caching inside `LintGroup` (writewithharper.com/docs/contributors/author-a-rule).
//!
//! Guarantees:
//! - No frontend toggle — always runs, backend, every Parakeet transcription.
//! - Never runs on Gemini output (ai_cleanup, post_process, gmail_voice) — only on Parakeet raw.
//! - Protected spans = hard safety boundary; custom dictionary = additional knowledge (not the reverse).
//! - Harper does NOT handle spacing / text track / email fulfillment — formatter does.
//! - Fail-open: panic or >50ms still returns original text.
//!
//! Hard gates (measured, not guessed):
//! - TECH PRESERVATION: 100% on protected-token suite
//! - FALSE POSITIVE RATE: effectively zero on sacred/technical content
//! - GRAMMAR RECALL: measured continuously, no invented target
//! - POST-STT LATENCY: hard <50ms release on reference hardware (45m synthetic)
//! - DETERMINISM: same input → identical output
//! - AMBIGUOUS CORRECTION: do nothing (e.g. `rolling out but afternoon` stays broken)

pub mod harper_engine;
pub mod invariant;
pub mod protected_spans;

#[allow(unused_imports)]
pub mod rules;

pub use harper_engine::correct;

/// Re-export for tests
pub use protected_spans::{find_protected_spans, ProtectedKind, ProtectedText};
