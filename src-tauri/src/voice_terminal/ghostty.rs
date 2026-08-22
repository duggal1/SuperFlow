//! macOS terminal-surface opening — port of the Ghostty tab-first behavior
//! from `Terminal-kit/src/tmux/mod.rs`. Attaches tmux sessions to visible
//! terminals: a new Ghostty tab per session (falling back to Terminal.app
//! when Ghostty is not installed or refuses automation).

use std::process::Command;

/// Path of the Ghostty app bundle, when installed.
fn ghostty_app_path() -> Option<String> {
    [
        "/Applications/Ghostty.app",
        "/System/Applications/Ghostty.app",
    ]
    .into_iter()
    .find(|p| std::path::Path::new(p).exists())
    .map(str::to_owned)
}

fn osascript(script: &str) -> Result<(), String> {
    let output = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\"'\"'"))
}

fn applescript_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "macos")]
fn ghostty_window_count() -> Result<usize, String> {
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
fn activate_ghostty() -> Result<(), String> {
    osascript("tell application \"Ghostty\" to activate")
}

#[cfg(target_os = "macos")]
fn launch_ghostty_app() -> Result<(), String> {
    let app = ghostty_app_path().ok_or_else(|| "Ghostty.app not found".to_string())?;
    let status = Command::new("open")
        .args(["-a", &app])
        .status()
        .map_err(|e| e.to_string())?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| "failed to launch Ghostty".to_string())
}

/// Bring Ghostty to the front with at least one window. Returns whether a
/// window already existed (drives tab vs first-window handling).
#[cfg(target_os = "macos")]
fn ensure_ghostty_front_window() -> Result<bool, String> {
    let had_window = ghostty_window_count()? > 0;
    if !had_window {
        launch_ghostty_app()?;
    }
    activate_ghostty()?;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if ghostty_window_count()? > 0 {
            return Ok(had_window);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Err("Ghostty did not expose a front window in time".to_string())
}

/// Open one Ghostty tab that attaches the given tmux session.
#[cfg(target_os = "macos")]
fn open_ghostty_tab(attach_command: &str) -> Result<(), String> {
    let script = format!(
        "tell application \"Ghostty\"\n\
         \x20   activate\n\
         \x20   set win to front window\n\
         \x20   set t to new tab in win\n\
         \x20   delay 0.2\n\
         \x20   select tab t\n\
         \x20   delay 0.1\n\
         \x20   set term to focused terminal of selected tab of win\n\
         \x20   input text (\"{}\\n\") to term\n\
         end tell",
        applescript_escape(attach_command)
    );
    osascript(&script)
}

/// Fallback: open the attach command in Apple Terminal.
fn open_terminal_app_tab(attach_command: &str) -> Result<(), String> {
    std::thread::sleep(std::time::Duration::from_millis(300));
    osascript(&format!(
        "tell application \"Terminal\" to activate\ntell application \"Terminal\" to do script \"{}\"",
        applescript_escape(attach_command)
    ))
}

/// Attach every listed tmux session to a visible terminal, in order. Ghostty
/// is preferred (tab-first); Terminal.app is the fallback.
pub fn open_sessions(tmux_bin: &str, sessions: &[String]) -> Result<(), String> {
    if sessions.is_empty() {
        return Ok(());
    }

    let attach =
        |session: &str| format!("{} -u attach-session -t {}", tmux_bin, shell_quote(session));

    #[cfg(target_os = "macos")]
    if ghostty_app_path().is_some() {
        match ensure_ghostty_front_window() {
            Ok(had_window) => {
                let mut result = Ok(());
                for (i, session) in sessions.iter().enumerate() {
                    let opened = if had_window || i > 0 {
                        open_ghostty_tab(&attach(session))
                    } else {
                        // Fresh Ghostty launch: its first window IS our first
                        // surface — type the attach into it directly.
                        crate::voice_terminal::ghostty::type_into_front_terminal(&attach(session))
                    };
                    if let Err(e) = opened {
                        result = Err(e);
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(120));
                }
                if result.is_ok() {
                    return Ok(());
                }
                // Ghostty automation refused — fall through to Terminal.app.
            }
            // Window check failed — fall through to Terminal.app.
            _ => {}
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = tmux_bin;

    for session in sessions {
        open_terminal_app_tab(&attach(session))?;
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    Ok(())
}

/// Type an attach command into whatever Ghostty terminal currently has focus
/// (used only right after a cold Ghostty launch).
#[cfg(target_os = "macos")]
fn type_into_front_terminal(attach_command: &str) -> Result<(), String> {
    let script = format!(
        "tell application \"Ghostty\"\n\
         \x20   set win to front window\n\
         \x20   delay 0.2\n\
         \x20   set term to focused terminal of selected tab of win\n\
         \x20   input text (\"{}\\n\") to term\n\
         end tell",
        applescript_escape(attach_command)
    );
    osascript(&script)
}
