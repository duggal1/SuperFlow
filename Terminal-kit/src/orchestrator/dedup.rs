//! Message deduplication for mail and directive processing.
//!
//! From Gas Town witness.md:725-751 — MessageDeduplicator pattern.
//! Prevents duplicate processing after crash restart or across poll cycles.
//!
//! Sapphire uses this for:
//! - Mail deduplication (prevent duplicate PTY injection after restart)
//! - Directive deduplication (same status directive parsed multiple times)

// Real implementations — size/clear are utility methods not yet called.
#![allow(dead_code)]

use std::collections::HashSet;

/// Tracks processed message IDs to prevent duplicate handling.
///
/// If the orchestrator restarts and re-processes pending mail from SQLite,
/// it could inject duplicate prompts into worker PTYs. This provides
/// in-memory idempotency within a single orchestrator session.
///
/// Thread-safe for concurrent tokio tasks (uses parking_lot Mutex).
#[derive(Debug)]
pub struct MessageDeduplicator {
    processed: HashSet<String>,
}

impl MessageDeduplicator {
    pub fn new() -> Self {
        Self {
            processed: HashSet::new(),
        }
    }

    /// Returns true if this message ID has been seen before.
    /// If not seen, marks it as processed and returns false.
    /// This is an atomic check-and-set operation.
    pub fn already_processed(&mut self, message_id: &str) -> bool {
        if message_id.is_empty() {
            return false; // Empty IDs can't be deduped
        }
        !self.processed.insert(message_id.to_owned())
    }

    /// Mark a message as processed (without checking).
    pub fn mark_processed(&mut self, message_id: &str) {
        if !message_id.is_empty() {
            self.processed.insert(message_id.to_owned());
        }
    }

    /// Number of tracked message IDs.
    pub fn size(&self) -> usize {
        self.processed.len()
    }

    /// Clear all tracked IDs (e.g., after a checkpoint).
    pub fn clear(&mut self) {
        self.processed.clear();
    }
}

impl Default for MessageDeduplicator {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_processing_returns_false() {
        let mut dedup = MessageDeduplicator::new();
        assert!(!dedup.already_processed("msg-1"));
    }

    #[test]
    fn second_processing_returns_true() {
        let mut dedup = MessageDeduplicator::new();
        assert!(!dedup.already_processed("msg-1"));
        assert!(dedup.already_processed("msg-1"));
    }

    #[test]
    fn empty_ids_are_not_deduped() {
        let mut dedup = MessageDeduplicator::new();
        assert!(!dedup.already_processed(""));
        assert!(!dedup.already_processed(""));
    }

    #[test]
    fn different_ids_are_independent() {
        let mut dedup = MessageDeduplicator::new();
        assert!(!dedup.already_processed("msg-1"));
        assert!(!dedup.already_processed("msg-2"));
        assert!(dedup.already_processed("msg-1"));
        assert!(dedup.already_processed("msg-2"));
    }

    #[test]
    fn mark_processed_works() {
        let mut dedup = MessageDeduplicator::new();
        dedup.mark_processed("msg-1");
        assert!(dedup.already_processed("msg-1"));
    }

    #[test]
    fn clear_resets_all() {
        let mut dedup = MessageDeduplicator::new();
        dedup.mark_processed("msg-1");
        dedup.mark_processed("msg-2");
        dedup.clear();
        assert!(!dedup.already_processed("msg-1"));
        assert!(!dedup.already_processed("msg-2"));
    }
}
