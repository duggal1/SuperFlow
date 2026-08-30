//! Mail subsystem for the Sapphire orchestrator.
//!
//! Handles SAPPHIRE_MAIL routing, ack processing, validation, auto-archival,
//! and engineering-semantic rendering. Extracted from mod.rs for modularity.

mod contract;
mod handlers;
mod nudge_queue;
mod render;
mod scavenge;
#[cfg(test)]
mod tests;
mod timeouts;
mod types;

// ─── Re-exports (same paths as the original single-file module) ──────────────
// These are consumed by orchestrator/mod.rs via `mail::` paths.
// The compiler sees them as "unused" within this module, but they ARE the public API.

#[allow(unused_imports)]
pub use types::{
    MailHandlingResult, MailStats, QueuedNudge, derive_delivery_mode, normalize_message_type,
    nudge_from_mail, parse_mail_id, requires_ack, resolve_alias, validate_mail,
};

#[allow(unused_imports)]
pub use contract::validate_team_mail;

#[allow(unused_imports)]
pub use render::{render_cc_notice, render_mail_for_delivery};

#[allow(unused_imports)]
pub use nudge_queue::{
    drain_nudge_queues, nudge_drain, nudge_enqueue, nudge_format_for_injection, nudge_pending_count,
};

#[allow(unused_imports)]
pub use scavenge::{ClaimResult, attempt_scavenge_claim, release_scavenge};

#[allow(unused_imports)]
pub use timeouts::{
    cc_timeout_prompt, mail_timeout_interval, mail_timeout_stage_due, probe_pending_mail,
    recipient_timeout_prompt, sender_timeout_prompt,
};

// Re-export the orchestrator's PendingMail to avoid duplication
pub use super::PendingMail;

#[allow(unused_imports)]
pub use handlers::{
    auto_archive_resolved_mail, handle_ack_directive, handle_lease_directive, handle_mail_directive,
};
