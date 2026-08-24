//! Intelligence-awareness context engine (Lane A).
//!
//! Captures a [`ContextSnapshot`] describing where the user is working the
//! moment a recording starts: frontmost application, browser tab URL and
//! title, classified into a product [`Surface`]. Detection is best-effort and
//! never blocks or fails dictation — every failure degrades to `Surface::Other`.

use std::path::PathBuf;

use developer::DeveloperContext;
use types::ContextSnapshot;

mod browser;
#[cfg(target_os = "macos")]
mod detector;
pub(crate) mod developer;
#[cfg(target_os = "macos")]
mod focused_text;

pub mod types;

pub(crate) mod capture;
pub mod classify;

#[derive(Clone, Debug)]
pub(crate) struct RecordingContext {
    pub snapshot: ContextSnapshot,
    pub project_root: Option<PathBuf>,
    pub developer: Option<DeveloperContext>,
}
