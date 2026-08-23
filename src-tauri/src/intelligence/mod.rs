//! Intelligence Awareness (Lane A): context-conditioned composition.
//!
//! When the user dictates inside an aware surface (Gmail/Slack) with the
//! toggle enabled, the transcript is treated as an INSTRUCTION and the
//! configured intelligence turns it into finished text using the captured
//! page context. Apple Intelligence runs first; the configured post-process
//! provider (e.g. Gemini) is the fallback.

pub(crate) mod prompt;
mod router;
mod validation;

pub use router::{compose_aware_reply, AwarenessOutcome};
