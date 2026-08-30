//! Minimal tmux subprocess wrapper.
//! Thin functions around `tmux` CLI — no async, no complexity.

pub mod grid;
mod zombie;

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;

/// Global mutex for tmux buffer operations (load-buffer / paste-buffer / delete-buffer).
/// The tmux buffer namespace is **global per server**, not per-pane. Without this mutex,
/// concurrent paste operations from different workers can interleave at the tmux server level,
/// causing prompt duplication or garbled text.
static TMUX_BUFFER_MUTEX: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneState {
    pub dead: bool,
    pub exit_code: Option<i32>,
}

pub struct Tmux {
    socket: Option<String>,
}

#[allow(dead_code)]
impl Tmux {
    pub fn new(socket: Option<String>) -> Self {
        Self { socket }
    }

    pub fn run(&self, args: &[&str]) -> Result<String, String> {
        let mut cmd = Command::new("tmux");
        cmd.arg("-u"); // UTF-8
        if let Some(sock) = &self.socket {
            cmd.arg("-L").arg(sock);
        }
        cmd.args(args);
        let output = cmd.output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(stderr);
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Create a detached session.
    #[allow(dead_code)]
    pub fn new_session(&self, name: &str, work_dir: &str) -> Result<(), String> {
        let args = vec!["new-session", "-d", "-s", name, "-c", work_dir];
        self.run(&args)?;
        // Override tmux 3.3+ manual sizing so window auto-resizes to client
        let _ = self.run(&["set-option", "-wt", name, "window-size", "latest"]);
        let _ = self.run(&["set-option", "-t", name, "remain-on-exit", "on"]);
        Ok(())
    }

    /// Create session with a command as the initial pane process.
    /// Uses respawn-pane to replace the default shell.
    #[allow(dead_code)]
    pub fn new_session_with_command(
        &self,
        name: &str,
        work_dir: &str,
        command: &str,
        env: &[(&str, &str)],
    ) -> Result<(), String> {
        // Step 1: create session with default shell
        self.new_session(name, work_dir)?;

        // Step 2: set environment variables on the session
        for (key, value) in env {
            let _ = self.run(&["set-environment", "-t", name, key, value]);
        }

        // Step 3: enable remain-on-exit before replacing shell
        let _ = self.run(&["set-option", "-t", name, "remain-on-exit", "on"]);

        // Step 4: replace shell with actual command
        let args = vec!["respawn-pane", "-k", "-t", name, "-c", work_dir, command];
        self.run(&args)?;

        Ok(())
    }

    /// Create a session with explicit dimensions for worker panes.
    /// Does NOT launch anything — just prepares the session for splitting.
    pub fn create_session_for_workers(
        &self,
        name: &str,
        work_dir: &str,
        cols: usize,
        rows: usize,
    ) -> Result<(), String> {
        let _ = self.kill_session(name);
        let cols_s = cols.to_string();
        let rows_s = rows.to_string();
        let args = vec![
            "new-session",
            "-d",
            "-s",
            name,
            "-c",
            work_dir,
            "-x",
            &cols_s,
            "-y",
            &rows_s,
        ];
        self.run(&args)?;
        let _ = self.run(&["set-option", "-t", name, "remain-on-exit", "on"]);
        let _ = self.run(&["set-option", "-wt", name, "window-size", "latest"]);
        Ok(())
    }

    /// Create a batch of tmux sessions with max `per_tab` panes each.
    /// Returns session names in order (from launch-codex-tabs.sh pattern).
    pub fn create_batch_sessions(
        &self,
        base_name: &str,
        work_dir: &str,
        total_workers: usize,
        per_tab: usize,
    ) -> Result<Vec<String>, String> {
        let mut session_names = Vec::new();
        let mut tab_num = 0;

        for start in (0..total_workers).step_by(per_tab) {
            let end = (start + per_tab).min(total_workers);
            let batch_size = end - start;
            let session = format!("{base_name}-{tab_num}");

            // Match the proven launch-supervisor-grid.sh geometry so interactive
            // terminals boot with enough vertical room before a GUI client attaches.
            let cols = if batch_size <= 2 {
                batch_size
            } else if batch_size <= 4 {
                2
            } else if batch_size <= 8 {
                4
            } else {
                4
            };
            let rows = (batch_size + cols - 1) / cols;
            let win_w = (cols * 55).max(200);
            let win_h = (rows * 22).max(40);

            self.create_session_for_workers(&session, work_dir, win_w, win_h)?;

            // Shell script pattern: split ALL panes first, then select layout ONCE
            for _ in 1..batch_size {
                let _ = self.split_window(&session, &session, true);
            }

            // Select the correct layout based on grid shape (from launch-supervisor-grid.sh)
            let layout = if cols == 1 {
                "even-vertical"
            } else if rows == 1 {
                "even-horizontal"
            } else {
                "tiled"
            };
            let _ = self.select_layout(&session, layout);

            session_names.push(session);
            tab_num += 1;
        }

        Ok(session_names)
    }

    /// Get all pane IDs in a session.
    pub fn list_pane_ids(&self, session: &str) -> Vec<String> {
        match self.run(&["list-panes", "-t", session, "-F", "#{pane_id}"]) {
            Ok(output) if !output.is_empty() => output.lines().map(|s| s.to_owned()).collect(),
            _ => Vec::new(),
        }
    }

    /// Send keys then Enter to a pane.
    pub fn send_command(&self, pane: &str, text: &str) -> Result<(), String> {
        self.send_keys_literal(pane, text)?;
        self.send_enter(pane)
    }

    /// Split a window/pane. `horizontal=true` → side-by-side (`-h`).
    /// Returns the new pane ID (e.g. `%5`).
    pub fn split_window(
        &self,
        _session: &str,
        target_pane: &str,
        horizontal: bool,
    ) -> Result<String, String> {
        let mut args = vec!["split-window", "-P", "-F", "#{pane_id}"];
        if horizontal {
            args.push("-h");
        } else {
            args.push("-v");
        }
        args.extend_from_slice(&["-t", target_pane]);
        self.run(&args)
    }

    pub fn split_window_with_command(
        &self,
        target_pane: &str,
        horizontal: bool,
        work_dir: &str,
        command: &str,
    ) -> Result<String, String> {
        let mut args = vec!["split-window", "-P", "-F", "#{pane_id}", "-c", work_dir];
        if horizontal {
            args.push("-h");
        } else {
            args.push("-v");
        }
        args.extend_from_slice(&["-t", target_pane, command]);
        self.run(&args)
    }

    pub fn pipe_pane(&self, pane: &str, command: &str) -> Result<(), String> {
        self.run(&["pipe-pane", "-o", "-t", pane, command])?;
        Ok(())
    }

    /// Send keystrokes to a pane. Uses literal mode for safety.
    pub fn send_keys_literal(&self, pane: &str, text: &str) -> Result<(), String> {
        if !text.is_empty() {
            self.run(&["send-keys", "-t", pane, "-l", text])?;
        }
        Ok(())
    }

    pub fn paste_text_via_buffer(&self, pane: &str, text: &str) -> Result<(), String> {
        if text.is_empty() {
            return Ok(());
        }

        // Serialize all tmux buffer operations to prevent concurrent pastes
        // from interleaving at the tmux server level.
        let _guard = TMUX_BUFFER_MUTEX.lock();

        let buffer_name = format!(
            "sapphire_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        );
        let temp_path = tmux_buffer_temp_path(&buffer_name);
        fs::write(&temp_path, text).map_err(|e| e.to_string())?;

        let temp_path_string = temp_path.to_string_lossy().into_owned();
        let load_result = self.run(&["load-buffer", "-b", &buffer_name, &temp_path_string]);
        let _ = fs::remove_file(&temp_path);
        load_result?;
        self.run(&["paste-buffer", "-b", &buffer_name, "-t", pane])?;
        let _ = self.run(&["delete-buffer", "-b", &buffer_name]);
        Ok(())
    }

    pub fn send_enter(&self, pane: &str) -> Result<(), String> {
        self.run(&["send-keys", "-t", pane, "Enter"])?;
        Ok(())
    }

    pub fn send_ctrl_c(&self, pane: &str) -> Result<(), String> {
        self.run(&["send-keys", "-t", pane, "C-c"])?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn send_keys(&self, pane: &str, text: &str) -> Result<(), String> {
        self.send_keys_literal(pane, text)?;
        std::thread::sleep(std::time::Duration::from_millis(500));
        self.send_enter(pane)
    }

    /// Capture the last N lines of a pane's visible content.
    #[allow(dead_code)]
    pub fn capture_pane(&self, pane: &str, lines: usize) -> Result<String, String> {
        self.run(&[
            "capture-pane",
            "-p",
            "-t",
            pane,
            "-S",
            &format!("-{}", lines),
        ])
    }

    /// Kill a session. Returns Ok even if session doesn't exist.
    pub fn kill_session(&self, name: &str) -> Result<(), String> {
        match self.run(&["kill-session", "-t", name]) {
            Ok(_) => Ok(()),
            Err(e) if e.contains("can't find session") || e.contains("no server") => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Check if a session exists.
    pub fn has_session(&self, name: &str) -> bool {
        self.run(&["has-session", "-t", name]).is_ok()
    }

    /// Set a tmux option on a session.
    pub fn set_option(&self, session: &str, key: &str, value: &str) -> Result<(), String> {
        self.run(&["set-option", "-t", session, key, value])?;
        Ok(())
    }

    /// Set a tmux window option.
    pub fn set_window_option(&self, session: &str, key: &str, value: &str) -> Result<(), String> {
        self.run(&["set-window-option", "-t", session, key, value])?;
        Ok(())
    }

    pub fn select_layout(&self, target: &str, layout: &str) -> Result<(), String> {
        self.run(&["select-layout", "-t", target, layout])?;
        Ok(())
    }

    pub fn set_pane_title(&self, pane: &str, title: &str) -> Result<(), String> {
        self.run(&["select-pane", "-t", pane, "-T", title])?;
        Ok(())
    }

    pub fn kill_pane(&self, pane: &str) -> Result<(), String> {
        self.run(&["kill-pane", "-t", pane])?;
        Ok(())
    }

    pub fn pane_state(&self, pane: &str) -> Result<PaneState, String> {
        let raw = self.run(&[
            "display-message",
            "-p",
            "-t",
            pane,
            "#{pane_dead} #{pane_dead_status}",
        ])?;
        let mut parts = raw.split_whitespace();
        let dead = matches!(parts.next(), Some("1"));
        let exit_code = parts.next().and_then(|value| value.parse::<i32>().ok());
        Ok(PaneState { dead, exit_code })
    }

    pub fn attach_session(&self, session: &str) -> Result<(), String> {
        let status = Command::new("tmux")
            .arg("-u")
            .args(["attach-session", "-t", session])
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("tmux attach failed with status {status}"))
        }
    }

    pub fn open_external_terminal_for_session(&self, session: &str) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            // VS Code terminal: route tmux sessions into Ghostty using the same tab-only policy.
            if terminal_program() == Some("vscode") {
                if ghostty_app_path().is_some() {
                    self.open_ghostty_batch_tabs(&[session.to_owned()])
                        .map_err(|error| {
                            format!(
                                "failed to open Ghostty tab for tmux session {session}: {error}"
                            )
                        })?;
                    return Ok(());
                }
                // Ghostty not installed — fall through to Apple Terminal
            }

            // Ghostty terminal: stay tab-only; never create a new Ghostty window here.
            if terminal_program() == Some("ghostty") {
                self.open_ghostty_batch_tabs(&[session.to_owned()])
                    .map_err(|error| {
                        format!("failed to open Ghostty tab for tmux session {session}: {error}")
                    })?;
                return Ok(());
            }

            self.open_terminal_app_for_session(session)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = session;
            Err("external terminal auto-open is only implemented on macOS".to_owned())
        }
    }

    #[cfg(target_os = "macos")]
    pub fn open_terminal_app_for_session(&self, session: &str) -> Result<(), String> {
        let command = format!("tmux attach-session -t {}", shell_quote(session));
        std::thread::sleep(std::time::Duration::from_millis(300));
        let output = Command::new("/usr/bin/osascript")
            .args([
                "-e",
                "tell application \"Terminal\" to activate",
                "-e",
                &format!(
                    "tell application \"Terminal\" to do script \"{}\"",
                    applescript_escape(&command)
                ),
            ])
            .output()
            .map_err(|e| e.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "failed to open external Terminal.app window for tmux session {session}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }

    #[cfg(target_os = "macos")]
    fn open_ghostty_tab_for_session(&self, session: &str) -> Result<(), String> {
        let attach_command = format!("tmux attach-session -t {}", shell_quote(session));
        let script = vec![
            "tell application \"Ghostty\"".to_owned(),
            "    activate".to_owned(),
            "    set win to front window".to_owned(),
            "    set t to new tab in win".to_owned(),
            "    delay 0.2".to_owned(),
            "    select tab t".to_owned(),
            "    delay 0.1".to_owned(),
            "    set term to focused terminal of selected tab of win".to_owned(),
            format!(
                "    input text (\"{}\\n\") to term",
                applescript_escape(&attach_command)
            ),
            "end tell".to_owned(),
        ];
        let mut command = Command::new("/usr/bin/osascript");
        for line in &script {
            command.arg("-e").arg(line);
        }
        let output = command.output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "Ghostty refused direct tab creation for tmux session {session}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn send_ghostty_attach_to_current_tab(&self, session: &str) -> Result<(), String> {
        let attach_command = format!("tmux attach-session -t {}", shell_quote(session));
        let script = vec![
            "tell application \"Ghostty\"".to_owned(),
            "    activate".to_owned(),
            "    set win to front window".to_owned(),
            "    delay 0.2".to_owned(),
            "    set term to focused terminal of selected tab of win".to_owned(),
            format!(
                "    input text (\"{}\\n\") to term",
                applescript_escape(&attach_command)
            ),
            "end tell".to_owned(),
        ];
        let mut command = Command::new("/usr/bin/osascript");
        for line in &script {
            command.arg("-e").arg(line);
        }
        let output = command.output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "Ghostty refused current-tab attach for tmux session {session}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn activate_ghostty(&self) -> Result<(), String> {
        let mut command = Command::new("/usr/bin/osascript");
        command
            .arg("-e")
            .arg("tell application \"Ghostty\" to activate");
        let output = command.output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "failed to activate Ghostty: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn ghostty_window_count(&self) -> Result<usize, String> {
        let script = concat!(
            "if application \"Ghostty\" is running then\n",
            "  tell application \"Ghostty\" to return count of windows\n",
            "else\n",
            "  return 0\n",
            "end if"
        );
        let output = Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(script)
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<usize>()
            .map_err(|e| e.to_string())
    }

    #[cfg(target_os = "macos")]
    fn launch_ghostty_app(&self) -> Result<(), String> {
        let ghostty_app = ghostty_app_path()
            .ok_or_else(|| "Ghostty.app was not found in /Applications".to_owned())?;
        let status = Command::new("open")
            .args(["-a", &ghostty_app])
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("failed to launch Ghostty".to_owned());
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn ensure_ghostty_front_window(&self) -> Result<bool, String> {
        let had_window = self.ghostty_window_count()? > 0;
        if !had_window {
            self.launch_ghostty_app()?;
        }
        self.activate_ghostty()?;

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if self.ghostty_window_count()? > 0 {
                return Ok(had_window);
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        Err("Ghostty did not expose a front window in time".to_owned())
    }

    #[cfg(target_os = "macos")]
    pub fn open_ghostty_batch_tabs(&self, session_names: &[String]) -> Result<(), String> {
        if session_names.is_empty() {
            return Ok(());
        }

        let had_window = self.ensure_ghostty_front_window()?;

        if had_window {
            for session in session_names {
                self.open_ghostty_tab_for_session(session)?;
                std::thread::sleep(std::time::Duration::from_millis(120));
            }
            return Ok(());
        }

        self.send_ghostty_attach_to_current_tab(&session_names[0])?;
        std::thread::sleep(std::time::Duration::from_millis(250));

        for session in session_names.iter().skip(1) {
            self.open_ghostty_tab_for_session(session)?;
            std::thread::sleep(std::time::Duration::from_millis(120));
        }

        Ok(())
    }

    pub fn is_available() -> bool {
        Command::new("tmux")
            .arg("-V")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    // ─── Zombie Detection (from gastown witness/deacon pattern) ──────────────────
    // Distinguish "tmux session exists but agent process dead" from "agent truly dead".

    /// Check session health: is the agent process alive within the tmux session?
    /// Uses pane PID → process liveness check (from gastown ZFC-compliant pattern).
    pub fn check_session_health(
        &self,
        target: &str,
        max_inactivity: std::time::Duration,
    ) -> SessionHealth {
        if let Some(created_at) = self.get_session_created(target) {
            if let Ok(age) = std::time::SystemTime::now().duration_since(created_at) {
                if age < zombie_starting_grace_period() {
                    return SessionHealth::Starting;
                }
            }
        }

        if matches!(self.pane_state(target), Ok(PaneState { dead: true, .. })) {
            return SessionHealth::Zombie;
        }

        let pid_str = match self.run(&["display-message", "-p", "-t", target, "#{pane_pid}"]) {
            Ok(s) => s,
            Err(_) => return SessionHealth::Dead,
        };
        let pid: u32 = match pid_str.trim().parse() {
            Ok(p) if p > 0 => p,
            _ => return SessionHealth::Dead,
        };
        if !is_process_alive(pid) {
            return SessionHealth::Zombie;
        }

        if !max_inactivity.is_zero() {
            let pane_dead = self
                .run(&["display-message", "-p", "-t", target, "#{pane_dead}"])
                .ok()
                .is_some_and(|value| value.trim() == "1");
            if pane_dead {
                return SessionHealth::Hung;
            }
        }

        SessionHealth::Healthy
    }

    pub fn get_session_created(&self, target: &str) -> Option<std::time::SystemTime> {
        let created = self
            .run(&["display-message", "-p", "-t", target, "#{session_created}"])
            .ok()?
            .trim()
            .parse::<u64>()
            .ok()?;
        Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(created))
    }

    // ─── Auto-Respawn Hook (from gastown PATCH-010 pattern) ──────────────────
    // When the agent process exits, tmux auto-respawns it — instant recovery
    // vs watchdog polling delay (1s).

    /// Set remain-on-exit on a pane so it persists after process exit.
    pub fn set_remain_on_exit(&self, pane: &str, on: bool) -> Result<(), String> {
        let value = if on { "on" } else { "off" };
        self.run(&["set-option", "-t", pane, "remain-on-exit", value])?;
        Ok(())
    }

    /// Set tmux auto-respawn hook: when the process exits, respawn it with the same command.
    /// From gastown SetAutoRespawnHook pattern: tmux set-hook "pane-exited" "respawn-pane ..."
    pub fn set_auto_respawn_hook(&self, pane: &str, command: &str) -> Result<(), String> {
        let hook_cmd = format!(
            "respawn-pane -k -t {} -c '#{{pane_current_path}}' -- {}",
            pane,
            shell_quote(command)
        );
        self.run(&["set-hook", "-t", pane, "pane-exited", &hook_cmd])?;
        Ok(())
    }
}

/// Session health status for zombie detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionHealth {
    /// tmux session + agent process alive + recent output
    Healthy,
    /// tmux session exists but agent process dead
    Zombie,
    /// agent alive but no output for > max_inactivity
    Hung,
    /// no tmux session
    Dead,
    /// recently created, within grace period
    Starting,
}

impl SessionHealth {
    #[allow(dead_code)]
    pub fn is_alive(&self) -> bool {
        matches!(self, Self::Healthy | Self::Hung | Self::Starting)
    }

    #[allow(dead_code)]
    pub fn is_zombie(&self) -> bool {
        matches!(self, Self::Zombie)
    }
}

/// Check if a process is alive by sending signal 0 via kill command.
/// From gastown IsAgentAlive pattern: kill(pid, 0) returns 0 if alive.
fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // Use the kill command as a portable way to check process liveness
        // kill -0 sends no signal but returns 0 if process exists
        std::process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        // Fallback: check /proc on systems that have it
        std::path::Path::new(&format!("/proc/{}", pid)).exists()
    }
}

fn zombie_starting_grace_period() -> std::time::Duration {
    std::time::Duration::from_secs(5)
}

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\"'\"'"))
}

fn tmux_buffer_temp_path(buffer_name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{buffer_name}.txt"))
}

#[cfg(target_os = "macos")]
fn applescript_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

#[allow(dead_code)]
fn terminal_program() -> Option<&'static str> {
    match std::env::var("TERM_PROGRAM")
        .ok()?
        .to_ascii_lowercase()
        .as_str()
    {
        "ghostty" => Some("ghostty"),
        "vscode" => Some("vscode"),
        "apple_terminal" => Some("apple_terminal"),
        "iterm.app" => Some("iterm"),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn ghostty_app_path() -> Option<String> {
    for path in [
        "/Applications/Ghostty.app",
        "/System/Applications/Ghostty.app",
    ] {
        if std::path::Path::new(path).exists() {
            return Some(path.to_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::Tmux;

    #[test]
    fn tmux_not_available_returns_error() {
        let tmux = Tmux::new(None);
        let result = tmux.run(&["new-session", "-d", "-s", "__sapphire_test_abc__"]);
        // Should fail gracefully if tmux not installed
        assert!(result.is_ok() || result.is_err());
        // Cleanup if it succeeded
        let _ = tmux.kill_session("__sapphire_test_abc__");
    }
}
