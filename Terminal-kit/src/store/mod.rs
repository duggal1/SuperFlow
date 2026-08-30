// All code should migrate to `crate::storage::Store` directly.
// This module exists to avoid breaking 100+ call sites during the transition.

pub use crate::storage::Store;
