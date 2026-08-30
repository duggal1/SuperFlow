use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use portable_pty::{Child, CommandBuilder, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::tmux::{PaneState, Tmux};

#[derive(Debug, Clone, Copy)]
pub enum SubmitMode {
    LineFeed,
    CarriageReturn,
}

#[derive(Debug, Clone)]
pub struct ProcessLaunchSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
    pub prompt_delay: Duration,
    pub startup_input: Option<(Duration, String)>,
    pub startup_rules: Vec<StartupAutomationRule>,
    pub surface_label: String,
    pub submit_mode: SubmitMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupAutomationRule {
    pub name: String,
    pub match_text: String,
    pub response: String,
    pub fire_once: bool,
}

impl StartupAutomationRule {
    pub fn new(
        name: impl Into<String>,
        match_text: impl Into<String>,
        response: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            match_text: match_text.into(),
            response: response.into(),
            fire_once: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeEvent {
    Output {
        session_id: Uuid,
        chunk: String,
    },
    Automation {
        session_id: Uuid,
        rule_name: String,
    },
    Exited {
        session_id: Uuid,
        exit_code: Option<i32>,
    },
}

pub struct SessionRuntime {
    tx: mpsc::UnboundedSender<RuntimeEvent>,
    rx: mpsc::UnboundedReceiver<RuntimeEvent>,
    backend: RuntimeBackend,
}

impl SessionRuntime {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            tx,
            rx,
            backend: RuntimeBackend::Pty,
        }
    }

    pub fn with_tmux(session_name: impl Into<String>, transcript_dir: PathBuf) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            tx,
            rx,
            backend: RuntimeBackend::Tmux(Arc::new(TmuxBackend {
                tmux: Tmux::new(None),
                session_name: session_name.into(),
                transcript_dir,
                spawned_panes: Mutex::new(0),
            })),
        }
    }

    pub fn spawn(&self, session_id: Uuid, spec: ProcessLaunchSpec) -> Result<RunningSession> {
        match &self.backend {
            RuntimeBackend::Pty => self.spawn_pty(session_id, spec),
            RuntimeBackend::Tmux(backend) => backend.spawn(self.tx.clone(), session_id, spec),
        }
    }

    fn spawn_pty(&self, session_id: Uuid, spec: ProcessLaunchSpec) -> Result<RunningSession> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 30,
                cols: 140,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("failed to allocate PTY")?;

        let mut command = CommandBuilder::new(spec.program);
        command.cwd(spec.cwd);
        for arg in &spec.args {
            command.arg(arg);
        }
        for (key, value) in &spec.env {
            command.env(key, value);
        }

        let child = pair
            .slave
            .spawn_command(command)
            .context("failed to spawn agent process")?;

        let writer = Arc::new(Mutex::new(pair.master.take_writer()?));
        let mut reader = pair.master.try_clone_reader()?;
        let reader_writer = Arc::clone(&writer);
        let tx = self.tx.clone();
        let automation_rules = spec.startup_rules.clone();
        let startup_input = spec.startup_input.clone();
        let submit_mode = spec.submit_mode;
        let child = Arc::new(Mutex::new(child));

        if let Some((delay, text)) = startup_input {
            let startup_writer = Arc::clone(&writer);
            thread::spawn(move || {
                thread::sleep(delay);
                let mut guard = startup_writer.lock();
                let _ = write_terminal_submission(&mut *guard, submit_mode, &text);
            });
        }

        thread::spawn(move || {
            let mut buffer = [0_u8; 4096];
            let mut carry = BufferManager::new(24_000, 12_000);
            let mut rules = automation_rules;

            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => {
                        let _ = tx.send(RuntimeEvent::Exited {
                            session_id,
                            exit_code: None,
                        });
                        break;
                    }
                    Ok(read_bytes) => {
                        let chunk = String::from_utf8_lossy(&buffer[..read_bytes]).into_owned();
                        carry.append(&chunk);

                        let _ = tx.send(RuntimeEvent::Output {
                            session_id,
                            chunk: chunk.clone(),
                        });

                        for rule in &mut rules {
                            if !rule.match_text.is_empty() && carry.contains(&rule.match_text) {
                                let mut guard = reader_writer.lock();
                                let _ = write_terminal_submission(
                                    &mut *guard,
                                    submit_mode,
                                    &rule.response,
                                );
                                let _ = tx.send(RuntimeEvent::Automation {
                                    session_id,
                                    rule_name: rule.name.clone(),
                                });
                                if rule.fire_once {
                                    rule.match_text.clear();
                                }
                            }
                        }
                    }
                    Err(_) => {
                        let _ = tx.send(RuntimeEvent::Exited {
                            session_id,
                            exit_code: None,
                        });
                        break;
                    }
                }
            }
        });

        Ok(RunningSession {
            handle: Arc::new(PtySessionHandle {
                session_id,
                writer,
                child,
                prompt_delay: spec.prompt_delay,
                display_name: spec.surface_label,
                submit_mode: spec.submit_mode,
            }),
        })
    }

    pub async fn next_event(&mut self, timeout: Duration) -> Option<RuntimeEvent> {
        match tokio::time::timeout(timeout, self.rx.recv()).await {
            Ok(Some(event)) => Some(event),
            Ok(None) | Err(_) => None,
        }
    }
}

enum RuntimeBackend {
    Pty,
    Tmux(Arc<TmuxBackend>),
}

struct TmuxBackend {
    tmux: Tmux,
    session_name: String,
    transcript_dir: PathBuf,
    spawned_panes: Mutex<usize>,
}

impl TmuxBackend {
    fn spawn(
        &self,
        tx: mpsc::UnboundedSender<RuntimeEvent>,
        session_id: Uuid,
        spec: ProcessLaunchSpec,
    ) -> Result<RunningSession> {
        let transcript_path = self
            .transcript_dir
            .join(format!("{}.log", sanitize_file_stem(&spec.surface_label)));
        if let Some(parent) = transcript_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let _ = std::fs::remove_file(&transcript_path);
        let _ = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&transcript_path)?;
        let command = render_tmux_command(&spec)?;

        // Reuse an existing pane if available, otherwise split a new one
        let (pane_id, created_new_pane) = {
            let mut spawned = self.spawned_panes.lock();
            let pane_id = {
                let pane_ids = self.tmux.list_pane_ids(&self.session_name);
                if *spawned < pane_ids.len() {
                    (pane_ids[*spawned].clone(), false)
                } else {
                    (
                        self.tmux
                            .split_window(&self.session_name, &self.session_name, *spawned % 2 == 0)
                            .map_err(anyhow::Error::msg)?,
                        true,
                    )
                }
            };
            *spawned += 1;
            pane_id
        };

        let _ = self.tmux.set_pane_title(&pane_id, &spec.surface_label);
        let _ = self.tmux.pipe_pane(
            &pane_id,
            &format!("cat >> {}", shell_quote(&transcript_path.to_string_lossy())),
        );
        let _ = self.tmux.send_command(&pane_id, &command);
        if created_new_pane {
            let _ = self.tmux.select_layout(&self.session_name, "tiled");
        }

        // Auto-respawn hook (from gastown PATCH-010 pattern):
        // When the agent process exits, tmux auto-respawns it — instant recovery
        // vs watchdog polling delay (1s). Non-fatal: session works without this.
        let _ = self.tmux.set_remain_on_exit(&pane_id, true);
        let _ = self.tmux.set_auto_respawn_hook(&pane_id, &command);

        let submit_mode = spec.submit_mode;
        let automation_rules = spec.startup_rules.clone();
        let startup_input = spec.startup_input.clone();
        let handle = Arc::new(TmuxSessionHandle {
            tmux: Tmux::new(None),
            pane_id: pane_id.clone(),
            prompt_delay: spec.prompt_delay,
            display_name: spec.surface_label.clone(),
            submit_mode: spec.submit_mode,
        });

        if let Some((delay, text)) = startup_input {
            let startup_handle = Arc::clone(&handle);
            let label = handle.display_name.to_string();
            let pane = handle.pane_id.clone();
            thread::spawn(move || {
                thread::sleep(delay);
                tracing::debug!(
                    worker = %label,
                    pane = %pane,
                    delay_ms = delay.as_millis(),
                    "startup_input firing: {preview}",
                    preview = text_preview(&text)
                );
                let _ = startup_handle.send_prompt(&text);
            });
        }

        spawn_tmux_monitor(
            tx,
            session_id,
            pane_id,
            transcript_path,
            automation_rules,
            submit_mode,
        );

        Ok(RunningSession { handle })
    }
}

/// Trait for interacting with a running terminal session.
///
/// Implemented by both `PtySessionHandle` (real PTY sessions) and
/// `TmuxSessionHandle` (tmux pane sessions), plus `TestSessionHandle` for unit tests.
///
/// The trait abstracts over three operations:
/// - Sending raw text to the session's input stream
/// - Submitting a prompt (text + terminal-specific line termination)
/// - Terminating the session
trait SessionHandle: Send + Sync {
    /// How long to wait before injecting the initial prompt.
    fn prompt_delay(&self) -> Duration;

    /// Human-readable label for this session (e.g. "Engineer-1").
    fn display_name(&self) -> &str;

    /// Terminal target for health checks when the session is backed by tmux.
    fn terminal_target(&self) -> Option<&str>;

    /// Send raw text to the session without triggering submission.
    fn send_text(&self, text: &str) -> Result<()>;

    /// Send text and trigger terminal submission with the correct line termination.
    /// Strips trailing `\r`/`\n` before sending, then appends the mode-specific terminator.
    fn send_prompt(&self, text: &str) -> Result<()>;

    /// Send Ctrl+C to interrupt the running process.
    fn terminate(&self) -> Result<()>;
}

struct TmuxSessionHandle {
    tmux: Tmux,
    pane_id: String,
    prompt_delay: Duration,
    display_name: String,
    submit_mode: SubmitMode,
}

impl SessionHandle for TmuxSessionHandle {
    fn prompt_delay(&self) -> Duration {
        self.prompt_delay
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn terminal_target(&self) -> Option<&str> {
        Some(&self.pane_id)
    }

    fn send_text(&self, text: &str) -> Result<()> {
        tracing::debug!(
            pane = %self.pane_id,
            worker = %self.display_name,
            bytes = text.len(),
            "send_text (tmux): {preview}",
            preview = text_preview(text)
        );
        self.tmux
            .paste_text_via_buffer(&self.pane_id, text)
            .map_err(anyhow::Error::msg)
    }

    fn send_prompt(&self, text: &str) -> Result<()> {
        let body = text.trim_end_matches(['\r', '\n']);
        tracing::debug!(
            pane = %self.pane_id,
            worker = %self.display_name,
            bytes = body.len(),
            submit_mode = ?self.submit_mode,
            "send_prompt (tmux): {preview}",
            preview = text_preview(body)
        );
        if !body.is_empty() {
            self.tmux
                .paste_text_via_buffer(&self.pane_id, body)
                .map_err(anyhow::Error::msg)?;
            if matches!(self.submit_mode, SubmitMode::CarriageReturn) {
                thread::sleep(Duration::from_millis(250));
            }
        }
        self.tmux
            .send_enter(&self.pane_id)
            .map_err(anyhow::Error::msg)
    }

    fn terminate(&self) -> Result<()> {
        self.tmux
            .send_ctrl_c(&self.pane_id)
            .map_err(anyhow::Error::msg)
    }
}

struct PtySessionHandle {
    session_id: Uuid,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
    prompt_delay: Duration,
    display_name: String,
    submit_mode: SubmitMode,
}

impl SessionHandle for PtySessionHandle {
    fn prompt_delay(&self) -> Duration {
        self.prompt_delay
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn terminal_target(&self) -> Option<&str> {
        None
    }

    fn send_text(&self, text: &str) -> Result<()> {
        tracing::debug!(
            session = %self.session_id,
            worker = %self.display_name,
            bytes = text.len(),
            "send_text (pty): {preview}",
            preview = text_preview(text)
        );
        let mut writer = self.writer.lock();
        writer
            .write_all(text.as_bytes())
            .with_context(|| format!("failed to write prompt to session {}", self.session_id))?;
        writer.flush()?;
        Ok(())
    }

    fn send_prompt(&self, text: &str) -> Result<()> {
        tracing::debug!(
            session = %self.session_id,
            worker = %self.display_name,
            bytes = text.len(),
            submit_mode = ?self.submit_mode,
            "send_prompt (pty): {preview}",
            preview = text_preview(text)
        );
        let mut writer = self.writer.lock();
        write_terminal_submission(&mut *writer, self.submit_mode, text)
            .with_context(|| format!("failed to submit prompt to session {}", self.session_id))
    }

    fn terminate(&self) -> Result<()> {
        self.child
            .lock()
            .kill()
            .context("failed to terminate child")
    }
}

impl Drop for PtySessionHandle {
    fn drop(&mut self) {
        let _ = self.child.lock().kill();
    }
}

pub struct RunningSession {
    handle: Arc<dyn SessionHandle>,
}

impl RunningSession {
    pub fn prompt_delay(&self) -> Duration {
        self.handle.prompt_delay()
    }

    pub fn display_name(&self) -> &str {
        self.handle.display_name()
    }

    pub fn terminal_target(&self) -> Option<&str> {
        self.handle.terminal_target()
    }

    pub fn send_text(&self, text: &str) -> Result<()> {
        self.handle.send_text(text)
    }

    pub fn send_prompt(&self, text: &str) -> Result<()> {
        self.handle.send_prompt(text)
    }

    #[allow(dead_code)]
    pub fn send_ctrl_c(&self) -> Result<()> {
        self.send_text("\u{3}")
    }

    #[allow(dead_code)]
    pub fn terminate(&self) -> Result<()> {
        self.handle.terminate()
    }
}

#[cfg(test)]
#[derive(Clone)]
pub struct TestSessionProbe {
    sent_texts: Arc<Mutex<Vec<String>>>,
}

#[cfg(test)]
impl TestSessionProbe {
    pub fn sent_texts(&self) -> Vec<String> {
        self.sent_texts.lock().clone()
    }
}

#[cfg(test)]
struct TestSessionHandle {
    display_name: String,
    prompt_delay: Duration,
    sent_texts: Arc<Mutex<Vec<String>>>,
    terminated: Arc<Mutex<bool>>,
}

#[cfg(test)]
impl SessionHandle for TestSessionHandle {
    fn prompt_delay(&self) -> Duration {
        self.prompt_delay
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn terminal_target(&self) -> Option<&str> {
        None
    }

    fn send_text(&self, text: &str) -> Result<()> {
        self.sent_texts.lock().push(text.to_owned());
        Ok(())
    }

    fn send_prompt(&self, text: &str) -> Result<()> {
        self.send_text(&format_terminal_submission(SubmitMode::LineFeed, text))
    }

    fn terminate(&self) -> Result<()> {
        *self.terminated.lock() = true;
        Ok(())
    }
}

#[cfg(test)]
impl RunningSession {
    pub fn test(
        display_name: impl Into<String>,
        prompt_delay: Duration,
    ) -> (Self, TestSessionProbe) {
        let sent_texts = Arc::new(Mutex::new(Vec::new()));
        let terminated = Arc::new(Mutex::new(false));
        let handle = TestSessionHandle {
            display_name: display_name.into(),
            prompt_delay,
            sent_texts: Arc::clone(&sent_texts),
            terminated,
        };
        (
            Self {
                handle: Arc::new(handle),
            },
            TestSessionProbe { sent_texts },
        )
    }
}

#[allow(dead_code)]
fn format_terminal_submission(mode: SubmitMode, text: &str) -> String {
    let mut rendered = text.trim_end_matches(['\r', '\n']).to_owned();
    match mode {
        SubmitMode::LineFeed => rendered.push('\n'),
        SubmitMode::CarriageReturn => rendered.push_str("\r\n"),
    }
    rendered
}

fn write_terminal_submission(
    writer: &mut (dyn Write + Send),
    mode: SubmitMode,
    text: &str,
) -> std::io::Result<()> {
    let body = text.trim_end_matches(['\r', '\n']);
    if !body.is_empty() {
        writer.write_all(body.as_bytes())?;
        writer.flush()?;
        if matches!(mode, SubmitMode::CarriageReturn) {
            thread::sleep(Duration::from_millis(250));
        }
    }
    let submit = match mode {
        SubmitMode::LineFeed => b"\n".as_slice(),
        SubmitMode::CarriageReturn => b"\r\n".as_slice(),
    };
    writer.write_all(submit)?;
    writer.flush()
}

/// Manages a bounded carry buffer that preserves UTF-8 character boundaries.
///
/// This is shared between the PTY reader thread and the tmux monitor thread.
/// Both accumulate output chunks into a carry buffer, trim when it exceeds
/// a maximum size, and drain from the front while keeping the tail.
pub struct BufferManager {
    carry: String,
    max_bytes: usize,
    keep_bytes: usize,
}

impl BufferManager {
    pub fn new(max_bytes: usize, keep_bytes: usize) -> Self {
        Self {
            carry: String::new(),
            max_bytes,
            keep_bytes,
        }
    }

    /// Append a chunk to the buffer and trim if it exceeds the maximum size.
    pub fn append(&mut self, chunk: &str) {
        self.carry.push_str(chunk);
        self.trim_if_needed();
    }

    /// Returns the full carry buffer contents.
    #[cfg(test)]
    pub fn as_str(&self) -> &str {
        &self.carry
    }

    /// Check if the carry buffer contains the given text.
    pub fn contains(&self, text: &str) -> bool {
        self.carry.contains(text)
    }

    /// Clear the carry buffer.
    #[cfg(test)]
    pub fn clear(&mut self) {
        self.carry.clear();
    }

    fn trim_if_needed(&mut self) {
        if self.carry.len() <= self.max_bytes {
            return;
        }
        let target = self.carry.len().saturating_sub(self.keep_bytes);
        let keep_from = previous_char_boundary(&self.carry, target);
        self.carry.drain(..keep_from);
    }
}

fn previous_char_boundary(text: &str, index: usize) -> usize {
    let mut boundary = index.min(text.len());
    while boundary > 0 && !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    boundary
}

fn render_tmux_command(spec: &ProcessLaunchSpec) -> Result<String> {
    let env_prefix = spec
        .env
        .iter()
        .map(|(key, value)| format!("{key}={}", shell_quote(value)))
        .collect::<Vec<_>>();
    let command = std::iter::once(shell_quote(&spec.program))
        .chain(spec.args.iter().map(|arg| shell_quote(arg)))
        .collect::<Vec<_>>()
        .join(" ");
    let full = if env_prefix.is_empty() {
        command
    } else {
        format!("env {} {}", env_prefix.join(" "), command)
    };
    let wrapped = format!(
        "cd {} && {}",
        shell_quote(&spec.cwd.to_string_lossy()),
        full
    );
    Ok(format!("/bin/zsh -lc {}", shell_quote(&wrapped)))
}

fn spawn_tmux_monitor(
    tx: mpsc::UnboundedSender<RuntimeEvent>,
    session_id: Uuid,
    pane_id: String,
    transcript_path: PathBuf,
    automation_rules: Vec<StartupAutomationRule>,
    submit_mode: SubmitMode,
) {
    thread::spawn(move || {
        let tmux = Tmux::new(None);
        let mut rules = automation_rules;
        let mut carry = BufferManager::new(24_000, 12_000);
        let mut offset = 0_u64;
        let mut idle_ticks = 0_u32;

        loop {
            match read_appended_text(&transcript_path, &mut offset) {
                Ok(chunk) if !chunk.is_empty() => {
                    idle_ticks = 0;
                    carry.append(&chunk);
                    let _ = tx.send(RuntimeEvent::Output {
                        session_id,
                        chunk: chunk.clone(),
                    });

                    for rule in &mut rules {
                        if !rule.match_text.is_empty() && carry.contains(&rule.match_text) {
                            let body = rule.response.trim_end_matches(['\r', '\n']);
                            if !body.is_empty() {
                                let _ = tmux.send_keys_literal(&pane_id, body);
                                if matches!(submit_mode, SubmitMode::CarriageReturn) {
                                    thread::sleep(Duration::from_millis(250));
                                }
                            }
                            let _ = tmux.send_enter(&pane_id);
                            let _ = tx.send(RuntimeEvent::Automation {
                                session_id,
                                rule_name: rule.name.clone(),
                            });
                            if rule.fire_once {
                                rule.match_text.clear();
                            }
                        }
                    }
                }
                Ok(_) => {
                    idle_ticks = idle_ticks.saturating_add(1);
                }
                Err(_) => {
                    idle_ticks = idle_ticks.saturating_add(1);
                }
            }

            if idle_ticks >= 2
                && let Ok(PaneState {
                    dead: true,
                    exit_code,
                }) = tmux.pane_state(&pane_id)
            {
                let _ = tx.send(RuntimeEvent::Exited {
                    session_id,
                    exit_code,
                });
                let _ = std::fs::remove_file(&transcript_path);
                break;
            }

            thread::sleep(Duration::from_millis(220));
        }
    });
}

fn read_appended_text(path: &Path, offset: &mut u64) -> Result<String> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(*offset))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    *offset += bytes.len() as u64;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn sanitize_file_stem(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '-',
        })
        .collect()
}

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\"'\"'"))
}

/// Truncate text to a short preview for logging.
fn text_preview(text: &str) -> String {
    let bytes = text.as_bytes();
    let limit = std::cmp::min(200, bytes.len());
    // Ensure we don't split a UTF-8 character boundary
    let preview = std::str::from_utf8(&bytes[..limit]).unwrap_or("invalid utf8");
    let escaped = preview
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    if limit < bytes.len() {
        format!("{escaped}… (+{} more bytes)", bytes.len() - limit)
    } else {
        escaped
    }
}

#[cfg(test)]
mod tests {
    use super::{BufferManager, RunningSession, SubmitMode, format_terminal_submission};
    use std::time::Duration;

    #[test]
    fn buffer_manager_appends_without_trim() {
        let mut buf = BufferManager::new(24_000, 12_000);
        buf.append("hello");
        buf.append(" world");
        assert_eq!(buf.as_str(), "hello world");
    }

    #[test]
    fn buffer_manager_trims_at_boundary() {
        let mut buf = BufferManager::new(100, 50);
        let long = "a".repeat(120);
        buf.append(&long);
        assert!(buf.as_str().len() <= 60); // keep_bytes + some slack
        assert!(buf.as_str().ends_with('a'));
    }

    #[test]
    fn buffer_manager_preserves_utf8_boundaries() {
        let mut buf = BufferManager::new(100, 50);
        let emoji = "🦀".repeat(40); // 4-byte chars
        buf.append(&emoji);
        assert!(std::str::from_utf8(buf.as_str().as_bytes()).is_ok());
    }

    #[test]
    fn test_session_helper_records_sent_prompts() {
        let (session, probe) = RunningSession::test("Engineer-01", Duration::from_millis(5));
        session.send_text("status").unwrap();
        session.send_prompt("continue").unwrap();
        assert_eq!(
            probe.sent_texts(),
            vec!["status".to_owned(), "continue\n".to_owned()]
        );
    }

    #[test]
    fn terminal_submission_formats_by_mode() {
        assert_eq!(
            format_terminal_submission(SubmitMode::LineFeed, "go"),
            "go\n".to_owned()
        );
        assert_eq!(
            format_terminal_submission(SubmitMode::CarriageReturn, "go"),
            "go\r\n".to_owned()
        );
    }

    #[test]
    fn buffer_manager_contains_works() {
        let mut buf = BufferManager::new(24_000, 12_000);
        buf.append("SAPPHIRE_STATUS {\"state\":\"progressing\"}");
        assert!(buf.contains("SAPPHIRE_STATUS"));
        assert!(!buf.contains("SAPPHIRE_MAIL"));
    }

    #[test]
    fn buffer_manager_clear_works() {
        let mut buf = BufferManager::new(24_000, 12_000);
        buf.append("data");
        assert_eq!(buf.as_str(), "data");
        buf.clear();
        assert_eq!(buf.as_str(), "");
    }

    #[test]
    fn buffer_manager_handles_multi_byte_trim() {
        let mut buf = BufferManager::new(20, 10);
        // "é" is 2 bytes; fill beyond max
        buf.append(&"é".repeat(15));
        assert!(std::str::from_utf8(buf.as_str().as_bytes()).is_ok());
        // Should have trimmed but kept valid UTF-8
        assert!(buf.as_str().len() <= 20);
    }
}
