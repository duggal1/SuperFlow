//! Intelligence Awareness (Lane A): context-conditioned composition.
//!
//! When the user dictates inside an aware surface with the toggle enabled,
//! the transcript is treated as an instruction and the configured intelligence
//! turns it into finished text using bounded context. Developer surfaces emit
//! execution-ready coding-agent prompts; Gmail and Slack emit finished prose.

pub(crate) mod prompt;
mod router;
mod validation;

pub use router::{compose_aware_reply, AwarenessOutcome};
