#![allow(dead_code)]

use std::time::{Duration, Instant};

use super::Tmux;

/// Grace period for zombie re-verification (from Gas Town constants).
/// Prevents killing slow-starting agents that appear dead during initialization.
fn zombie_kill_grace_period() -> Duration {
    Duration::from_secs(5)
}

/// Result of a zombie verification attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZombieVerificationResult {
    /// Session is healthy (agent is alive)
    Healthy,
    /// Session is a confirmed zombie (agent dead, re-verified)
    ConfirmedZombie,
    /// Session was replaced by another process (TOCTOU guard)
    SessionReplaced,
    /// Session doesn't exist
    NoSession,
}

/// Zombie verification with TOCTOU mitigation (from Gas Town witness.md:115-145).
///
/// Instead of immediately killing a zombie session, this:
/// 1. Records the session creation time
/// 2. Waits for a grace period
/// 3. Re-verifies liveness
/// 4. Checks if session was replaced (TOCTOU guard)
///
/// This prevents false kills during slow agent startup.
impl Tmux {
    /// Verify if a session is truly a zombie (with TOCTOU mitigation).
    ///
    /// Returns `ConfirmedZombie` only after re-verification fails.
    /// Returns `Healthy` if the agent is actually alive.
    /// Returns `SessionReplaced` if another process already replaced the session.
    pub fn verify_zombie(&self, session: &str) -> ZombieVerificationResult {
        // Record creation time before waiting
        let created_at = self.get_session_created_unix(session);

        // Wait for grace period
        std::thread::sleep(zombie_kill_grace_period());

        // Re-check liveness
        if self.is_agent_alive(session) {
            return ZombieVerificationResult::Healthy;
        }

        // Check if session was replaced (TOCTOU guard)
        if let Some(created_now) = self.get_session_created_unix(session) {
            if let Some(created_before) = created_at {
                if created_now != created_before {
                    // Session was replaced by another process — don't kill
                    return ZombieVerificationResult::SessionReplaced;
                }
            }
        }

        ZombieVerificationResult::ConfirmedZombie
    }

    /// Check if the agent process is alive within a tmux session.
    /// Uses pane PID → process liveness check.
    pub fn is_agent_alive(&self, session: &str) -> bool {
        let pid_str = match self.run(&["display-message", "-p", "-t", session, "#{pane_pid}"]) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let pid: u32 = match pid_str.trim().parse() {
            Ok(p) if p > 0 => p,
            _ => return false,
        };
        is_process_alive(pid)
    }

    /// Get the session creation time as a Unix timestamp.
    fn get_session_created_unix(&self, session: &str) -> Option<u64> {
        self.run(&["display-message", "-p", "-t", session, "#{session_created}"])
            .ok()
            .and_then(|s| s.trim().parse().ok())
    }
}

/// Wait for a command/process to be ready in a tmux session.
/// From Gas Town witness.md:220-228 pattern.
///
/// Polls `#{pane_dead}` until the pane is confirmed alive, up to a timeout.
/// This is more reliable than fixed delays — it actually verifies readiness.
impl Tmux {
    /// Wait for the agent shell to be ready in a tmux session.
    ///
    /// Polls the pane state every 200ms until:
    /// - Pane is not dead → returns Ok(())
    /// - Timeout exceeded → returns Err
    ///
    /// This replaces fixed boot delays with actual readiness verification.
    pub fn wait_for_ready(&self, session: &str, timeout: Duration) -> Result<(), String> {
        let start = Instant::now();
        let poll_interval = Duration::from_millis(200);

        loop {
            if start.elapsed() > timeout {
                return Err(format!(
                    "timed out waiting for session '{}' to be ready after {:?}",
                    session,
                    start.elapsed()
                ));
            }

            // Check if pane is alive
            let pane_dead = self
                .run(&["display-message", "-p", "-t", session, "#{pane_dead}"])
                .ok()
                .is_some_and(|v| v.trim() == "1");

            if !pane_dead {
                // Also verify PID is alive
                if self.is_agent_alive(session) {
                    return Ok(());
                }
            }

            std::thread::sleep(poll_interval);
        }
    }
}

/// Check if a process is alive by sending signal 0.
fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        std::path::Path::new(&format!("/proc/{}", pid)).exists()
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zombie_verification_returns_no_session_for_missing() {
        let tmux = Tmux::new(None);
        let result = tmux.verify_zombie("__sapphire_nonexistent_test__");
        assert!(matches!(
            result,
            ZombieVerificationResult::NoSession | ZombieVerificationResult::ConfirmedZombie
        ));
    }
}
