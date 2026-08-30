//! Non-destructive nudge delivery via filesystem queue.
//! Instead of injecting text directly into the agent's PTY (which cancels
//! in-flight tool calls), nudges are written as JSON files and picked up
//! at the next natural turn boundary.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use super::types::QueuedNudge;
use crate::orchestrator::ActiveSession;

const MAX_QUEUE_DEPTH: usize = 50;
const STALE_CLAIM_SECS: u64 = 5 * 60; // 5 min

fn queue_dir(state_dir: &Path, session_id: &str) -> PathBuf {
    let safe = session_id.replace(['/', '\\'], "_");
    state_dir.join("nudge_queue").join(safe)
}

/// Write a nudge to the filesystem queue. Returns error if queue is full.
pub fn nudge_enqueue(state_dir: &Path, session_id: &str, nudge: QueuedNudge) -> Result<()> {
    let dir = queue_dir(state_dir, session_id);
    fs::create_dir_all(&dir)?;
    let pending = nudge_pending_count(state_dir, session_id);
    if pending >= MAX_QUEUE_DEPTH {
        anyhow::bail!("nudge queue full ({}/{})", pending, MAX_QUEUE_DEPTH);
    }
    let safe_sender = nudge.sender.replace(['/', '\\'], "_");
    let filename = format!(
        "{}-{}.json",
        nudge.timestamp.timestamp_nanos_opt().unwrap_or(0),
        safe_sender
    );
    fs::write(dir.join(&filename), serde_json::to_string_pretty(&nudge)?)?;
    Ok(())
}

/// Drain all queued nudges in FIFO order. Atomic rename prevents double-delivery.
/// Expired nudges are discarded. Orphaned .claimed files are requeued.
pub fn nudge_drain(state_dir: &Path, session_id: &str) -> Result<Vec<QueuedNudge>> {
    let dir = queue_dir(state_dir, session_id);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let now = chrono::Utc::now();

    // Sweep orphaned .claimed files
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.contains(".claimed") {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(modified_ts) = meta.modified() {
                        if modified_ts.elapsed().unwrap_or_default().as_secs() > STALE_CLAIM_SECS {
                            if let Some(base) = name.split(".claimed").next() {
                                let _ =
                                    fs::rename(entry.path(), dir.join(format!("{}.json", base)));
                            }
                        } else {
                            let _ = fs::remove_file(entry.path());
                        }
                    }
                }
            }
        }
    }

    let mut nudges = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        let mut files: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
            .collect();
        files.sort_by_key(|e| e.file_name());
        for entry in files {
            let path = entry.path();
            let claim = PathBuf::from(format!("{}.claimed", path.display()));
            if fs::rename(&path, &claim).is_err() {
                continue;
            }
            let data = match fs::read_to_string(&claim) {
                Ok(d) => d,
                Err(_) => {
                    let _ = fs::rename(&claim, &path);
                    continue;
                }
            };
            let nudge: QueuedNudge = match serde_json::from_str(&data) {
                Ok(n) => n,
                Err(_) => {
                    let _ = fs::remove_file(&claim);
                    continue;
                }
            };
            if now > nudge.expires_at {
                let _ = fs::remove_file(&claim);
                continue;
            }
            nudges.push(nudge);
            let _ = fs::remove_file(&claim);
        }
    }
    Ok(nudges)
}

/// Count pending nudges without draining.
pub fn nudge_pending_count(state_dir: &Path, session_id: &str) -> usize {
    let dir = queue_dir(state_dir, session_id);
    if !dir.exists() {
        return 0;
    }
    fs::read_dir(&dir)
        .map(|e| {
            e.filter_map(|x| x.ok())
                .filter(|x| x.file_name().to_string_lossy().ends_with(".json"))
                .count()
        })
        .unwrap_or(0)
}

/// Format nudges as <system-reminder> block for PTY injection.
pub fn nudge_format_for_injection(nudges: &[QueuedNudge]) -> String {
    if nudges.is_empty() {
        return String::new();
    }
    let (urgent, normal): (Vec<_>, Vec<_>) = nudges.iter().partition(|n| {
        n.priority.eq_ignore_ascii_case("urgent") || n.priority.eq_ignore_ascii_case("critical")
    });
    let mut lines = vec!["<system-reminder>".to_owned()];
    if !urgent.is_empty() {
        lines.push(format!("QUEUED NUDGE ({} urgent):\n", urgent.len()));
        for n in &urgent {
            lines.push(format!("  [URGENT from {}] {}", n.sender, n.message));
        }
        if !normal.is_empty() {
            lines.push(format!("\nPlus {} non-urgent nudge(s):", normal.len()));
            for n in &normal {
                lines.push(format!("  [from {}] {}", n.sender, n.message));
            }
        }
        lines.push("\nHandle urgent nudges before continuing current work.".to_owned());
    } else {
        lines.push(format!("QUEUED NUDGE ({} message(s)):\n", normal.len()));
        for n in &normal {
            lines.push(format!("  [from {}] {}", n.sender, n.message));
        }
        lines.push(
            "\nBackground notification. Continue work unless nudge is higher priority.".to_owned(),
        );
    }
    lines.push("</system-reminder>".to_owned());
    lines.join("\n")
}

/// Probe the nudge queue for each active session and drain deliverable nudges.
/// Returns nudges to inject per session.
pub fn drain_nudge_queues(
    state_dir: &Path,
    active_sessions: &HashMap<uuid::Uuid, ActiveSession>,
) -> HashMap<uuid::Uuid, String> {
    let mut injections = HashMap::new();
    for (session_id, session) in active_sessions {
        if session.state.is_terminal() {
            continue;
        }
        let nudges = nudge_drain(state_dir, &session.record.id.to_string()).unwrap_or_default();
        if nudges.is_empty() {
            continue;
        }
        let formatted = nudge_format_for_injection(&nudges);
        injections.insert(*session_id, formatted);
    }
    injections
}
