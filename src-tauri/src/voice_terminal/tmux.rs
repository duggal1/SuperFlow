//! Minimal tmux control surface — a surgical port of the proven parts of
//! `Terminal-kit/src/tmux/mod.rs` (session creation, grid splitting via
//! `select-layout`, and buffer-based prompt pasting). Thin functions around
//! the `tmux` CLI; no async, no complexity.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// tmux buffer operations are global per server. Serialize them so
/// concurrent prompt pastes can never interleave (same invariant as sp).
static BUFFER_LOCK: Mutex<()> = Mutex::new(());

pub struct Tmux {
    bin: String,
}

impl Tmux {
    /// Locate tmux from a GUI-app context where PATH is minimal: check the
    /// common Homebrew prefixes first, then ask the user's login shell.
    pub fn discover() -> Option<Tmux> {
        for candidate in [
            "/opt/homebrew/bin/tmux",
            "/usr/local/bin/tmux",
            "/usr/bin/tmux",
        ] {
            if Command::new(candidate)
                .arg("-V")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                return Some(Tmux {
                    bin: candidate.to_string(),
                });
            }
        }
        let output = Command::new("/bin/zsh")
            .args(["-lc", "command -v tmux"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let bin = String::from_utf8_lossy(&output.stdout).trim().to_string();
        (!bin.is_empty()).then_some(Tmux { bin })
    }

    pub fn binary(&self) -> &str {
        &self.bin
    }

    fn run(&self, args: &[&str]) -> Result<String, String> {
        let mut cmd = Command::new(&self.bin);
        cmd.arg("-u"); // UTF-8
        cmd.args(args);
        let output = cmd.output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Create a detached session whose first pane is a plain login shell.
    /// The agent CLI is typed into panes afterwards so each pane resolves its
    /// own PATH through the user's login environment.
    pub fn create_session(&self, name: &str, work_dir: &str) -> Result<(), String> {
        // Kill any stale session with the same name first.
        let _ = self.kill_session(name);
        self.run(&[
            "new-session",
            "-d",
            "-s",
            name,
            "-c",
            work_dir,
            "-x",
            "220",
            "-y",
            "44",
        ])?;
        let _ = self.run(&["set-option", "-t", name, "remain-on-exit", "on"]);
        let _ = self.run(&["set-option", "-wt", name, "window-size", "latest"]);
        Ok(())
    }

    pub fn pane_ids(&self, session: &str) -> Vec<String> {
        match self.run(&["list-panes", "-t", session, "-F", "#{pane_id}"]) {
            Ok(out) if !out.is_empty() => out.lines().map(str::to_owned).collect(),
            _ => Vec::new(),
        }
    }

    /// Paste a full prompt into a pane through a named tmux buffer (atomic,
    /// no per-keystroke typing), then press Enter to submit.
    pub fn paste_prompt(&self, pane: &str, text: &str) -> Result<(), String> {
        if text.trim().is_empty() {
            return Ok(());
        }
        let _guard = BUFFER_LOCK.lock();

        let buffer_name = format!(
            "superflow_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        );
        let temp_path: PathBuf = std::env::temp_dir().join(format!("{buffer_name}.txt"));
        std::fs::write(&temp_path, text).map_err(|e| e.to_string())?;

        let load = self.run(&[
            "load-buffer",
            "-b",
            &buffer_name,
            &temp_path.to_string_lossy(),
        ]);
        let _ = std::fs::remove_file(&temp_path);
        load?;

        let result = self.run(&["paste-buffer", "-b", &buffer_name, "-t", pane]);
        let _ = self.run(&["delete-buffer", "-b", &buffer_name]);
        result?;

        self.send_enter(pane)
    }

    /// Send literal keystrokes (no Enter) — used for typing launch lines.
    pub fn send_keys_literal(&self, pane: &str, text: &str) -> Result<(), String> {
        if !text.is_empty() {
            self.run(&["send-keys", "-t", pane, "-l", text])?;
        }
        Ok(())
    }

    /// Wake an interactive TUI readline before a prompt arrives (sp's
    /// startup-nudge pattern).
    pub fn send_enter(&self, pane: &str) -> Result<(), String> {
        self.run(&["send-keys", "-t", pane, "Enter"])?;
        Ok(())
    }

    pub fn kill_session(&self, name: &str) -> Result<(), String> {
        match self.run(&["kill-session", "-t", name]) {
            Ok(_) => Ok(()),
            Err(e) if e.contains("can't find session") || e.contains("no server") => Ok(()),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_finds_tmux_or_none() {
        // Must not panic in either case; real availability depends on machine.
        let _ = Tmux::discover();
    }
}
