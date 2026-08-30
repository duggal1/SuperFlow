//! Parses .sp/control/status.txt — ALL fields, no partial reads.

use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct ControlStatusSnapshot {
    pub session_id: Option<String>,
    pub updated_at: Option<String>,
    pub supervisor: Option<LiveSessionStatus>,
    pub standby_supervisor: Option<LiveSessionStatus>,
    pub supervisors: HashMap<String, LiveSessionStatus>,
    pub workers: HashMap<String, LiveSessionStatus>,
    pub watchdog: WatchdogLine,
    pub blocked: Vec<String>,
    pub validation_queue: Vec<String>,
    pub contradictions: Vec<String>,
    pub mail_pressure: Vec<String>,
    pub problems: Vec<String>,
    pub ownership_gaps: Vec<String>,
    pub first_status_incidents: Vec<String>,
    pub systemic_incidents: Vec<String>,
    pub crash_loops: Vec<String>,
    pub pods: Vec<PodLine>,
}

#[derive(Debug, Clone, Default)]
pub struct WatchdogLine {
    pub worker_count: usize,
    pub directives: usize,
    pub mail_routed: usize,
    pub validation_challenges: usize,
    pub stall_interventions: usize,
    pub lease_conflicts: usize,
    pub protocol_reminders: usize,
    pub supervisor_health_events: usize,
    pub critical_failures: usize,
    pub crash_loops_detected: usize,
}

#[derive(Debug, Clone)]
pub struct LiveSessionStatus {
    pub name: String,
    pub state: String,
    pub summary: String,
    pub liveness: Option<String>,
    pub owner: Option<String>,
    pub branch: Option<String>,
    pub role: Option<String>,
    pub task: Option<String>,
    pub incident_scope: Option<String>,
    pub failure_kind: Option<String>,
    pub agent_count: usize,
    pub blocked_count: usize,
    pub validating_count: usize,
    pub consecutive_stall_failures: usize,
    pub total_interventions: usize,
    pub output_chunks: usize,
    pub mail_thread_count: usize,
    pub files_touched: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PodLine {
    pub name: String,
    pub members: Vec<String>,
    pub blocked: Vec<String>,
    pub open_threads: usize,
}

pub fn parse_control_status(text: &str) -> ControlStatusSnapshot {
    let mut snap = ControlStatusSnapshot::default();
    let mut section: &str = "header";

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        // Section headers
        if line.starts_with("Pods:") {
            snap.pods = parse_pods_line(&line["Pods: ".len()..]);
            section = "pods";
            continue;
        }
        if line == "Supervisors" {
            section = "supervisors";
            continue;
        }
        if line == "Workers" {
            section = "workers";
            continue;
        }
        if line.starts_with("Memory:") || line.starts_with("Meetings:") {
            section = line.split(':').next().unwrap_or("");
            continue;
        }

        // Header fields
        if section == "header" {
            if let Some(rest) = line.strip_prefix("Session: ") {
                snap.session_id = Some(rest.to_owned());
                continue;
            }
            if let Some(rest) = line.strip_prefix("Updated: ") {
                snap.updated_at = Some(rest.to_owned());
                continue;
            }
            if line.starts_with("Workers: ") {
                snap.watchdog = parse_watchdog_line(line);
                continue;
            }
            if let Some(rest) = line.strip_prefix("Supervisor: ") {
                snap.supervisor = parse_named_session(rest);
                continue;
            }
            if let Some(rest) = line.strip_prefix("Standby Supervisor: ") {
                snap.standby_supervisor = parse_named_session(rest);
                continue;
            }
            if let Some(rest) = line.strip_prefix("Blocked: ") {
                snap.blocked = parse_name_list(rest);
                continue;
            }
            if let Some(rest) = line.strip_prefix("Validation Queue: ") {
                snap.validation_queue = parse_name_list(rest);
                continue;
            }
            if let Some(rest) = line.strip_prefix("Contradictions: ") {
                snap.contradictions = parse_name_list(rest);
                continue;
            }
            if let Some(rest) = line.strip_prefix("Mail Pressure: ") {
                snap.mail_pressure = parse_name_list(rest);
                continue;
            }
            if let Some(rest) = line.strip_prefix("Problems: ") {
                snap.problems = parse_name_list(rest);
                continue;
            }
            if let Some(rest) = line.strip_prefix("Ownership Gaps: ") {
                snap.ownership_gaps = parse_name_list(rest);
                continue;
            }
            if let Some(rest) = line.strip_prefix("First-Status Incidents: ") {
                snap.first_status_incidents = parse_name_list(rest);
                continue;
            }
            if let Some(rest) = line.strip_prefix("Systemic Incidents: ") {
                snap.systemic_incidents = parse_name_list(rest);
                continue;
            }
            if let Some(rest) = line.strip_prefix("Crash Loops: ") {
                snap.crash_loops = parse_name_list(rest);
                continue;
            }
        }

        if line.starts_with("- ") {
            match section {
                "supervisors" => {
                    if let Some(supervisor) = parse_worker_line(line) {
                        snap.supervisors.insert(supervisor.name.clone(), supervisor);
                    }
                }
                "workers" | "header" => {
                    if let Some(worker) = parse_worker_line(line) {
                        snap.workers.insert(worker.name.clone(), worker);
                    }
                }
                _ => {}
            }
            continue;
        }
    }

    snap
}

fn parse_watchdog_line(line: &str) -> WatchdogLine {
    let mut wd = WatchdogLine::default();
    for part in line.split('|') {
        let part = part.trim();
        if let Some(v) = extract_usize_prefix(part, "Workers:") {
            wd.worker_count = v;
        } else if let Some(v) = extract_usize_prefix(part, "Directives:") {
            wd.directives = v;
        } else if let Some(v) = extract_usize_prefix(part, "Mail:") {
            wd.mail_routed = v;
        } else if let Some(v) = extract_usize_prefix(part, "Validation:") {
            wd.validation_challenges = v;
        } else if let Some(v) = extract_usize_prefix(part, "Stalls:") {
            wd.stall_interventions = v;
        } else if let Some(v) = extract_usize_prefix(part, "Lease Conflicts:") {
            wd.lease_conflicts = v;
        } else if let Some(v) = extract_usize_prefix(part, "Protocol Reminders:") {
            wd.protocol_reminders = v;
        } else if let Some(v) = extract_usize_prefix(part, "Supervisor Health:") {
            wd.supervisor_health_events = v;
        } else if let Some(v) = extract_usize_prefix(part, "Critical Failures:") {
            wd.critical_failures = v;
        } else if let Some(v) = extract_usize_prefix(part, "Crash Loops:") {
            wd.crash_loops_detected = v;
        }
    }
    wd
}

fn extract_usize_prefix(part: &str, prefix: &str) -> Option<usize> {
    part.strip_prefix(prefix)
        .map(str::trim)
        .and_then(|v| v.parse::<usize>().ok())
}

fn parse_named_session(line: &str) -> Option<LiveSessionStatus> {
    let line = line.trim();
    let open = line.find('[')?;
    let close = line[open..].find(']')? + open;
    let name = line[..open].trim();
    let state = line[open + 1..close].trim();
    let rest = line[close + 1..].trim();
    if name.is_empty() || state.is_empty() {
        return None;
    }

    let meta = parse_session_rest(rest);

    Some(LiveSessionStatus {
        name: name.to_owned(),
        state: state.to_owned(),
        summary: meta.summary,
        liveness: meta.liveness,
        owner: meta.owner,
        branch: meta.branch,
        role: meta.role,
        task: meta.task,
        incident_scope: meta.incident_scope,
        failure_kind: meta.failure_kind,
        agent_count: meta.agent_count,
        blocked_count: meta.blocked_count,
        validating_count: meta.validating_count,
        consecutive_stall_failures: meta.consecutive_stall_failures,
        total_interventions: meta.total_interventions,
        output_chunks: meta.output_chunks,
        mail_thread_count: meta.mail_thread_count,
        files_touched: meta.files_touched,
    })
}

#[derive(Debug, Default)]
struct ParsedSessionMeta {
    summary: String,
    liveness: Option<String>,
    owner: Option<String>,
    branch: Option<String>,
    role: Option<String>,
    task: Option<String>,
    incident_scope: Option<String>,
    failure_kind: Option<String>,
    agent_count: usize,
    blocked_count: usize,
    validating_count: usize,
    consecutive_stall_failures: usize,
    total_interventions: usize,
    output_chunks: usize,
    mail_thread_count: usize,
    files_touched: Vec<String>,
}

fn parse_session_rest(rest: &str) -> ParsedSessionMeta {
    let mut meta = ParsedSessionMeta::default();
    let mut summary_parts = Vec::new();

    for part in split_session_tokens(rest) {
        if let Some(val) = part.strip_prefix("stalls=") {
            meta.consecutive_stall_failures = val.parse::<usize>().unwrap_or(0);
        } else if let Some(val) = part.strip_prefix("interventions=") {
            meta.total_interventions = val.parse::<usize>().unwrap_or(0);
        } else if let Some(val) = part.strip_prefix("outputs=") {
            meta.output_chunks = val.parse::<usize>().unwrap_or(0);
        } else if let Some(val) = part.strip_prefix("mail=") {
            meta.mail_thread_count = val.parse::<usize>().unwrap_or(0);
        } else if let Some(val) = part.strip_prefix("files=") {
            meta.files_touched = val
                .split(',')
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .collect();
        } else if let Some(val) = part.strip_prefix("liveness=") {
            meta.liveness = Some(parse_meta_value(val));
        } else if let Some(val) = part.strip_prefix("owner=") {
            meta.owner = Some(parse_meta_value(val));
        } else if let Some(val) = part.strip_prefix("branch=") {
            meta.branch = Some(parse_meta_value(val));
        } else if let Some(val) = part.strip_prefix("role=") {
            meta.role = Some(parse_meta_value(val));
        } else if let Some(val) = part.strip_prefix("task=") {
            meta.task = Some(parse_meta_value(val));
        } else if let Some(val) = part.strip_prefix("incident=") {
            meta.incident_scope = Some(parse_meta_value(val));
        } else if let Some(val) = part.strip_prefix("failure=") {
            meta.failure_kind = Some(parse_meta_value(val));
        } else if let Some(val) = part.strip_prefix("agents=") {
            meta.agent_count = val.parse::<usize>().unwrap_or(0);
        } else if let Some(val) = part.strip_prefix("blocked=") {
            meta.blocked_count = val.parse::<usize>().unwrap_or(0);
        } else if let Some(val) = part.strip_prefix("validating=") {
            meta.validating_count = val.parse::<usize>().unwrap_or(0);
        } else if let Some(val) = part.strip_prefix("summary=") {
            meta.summary = parse_meta_value(val);
        } else {
            summary_parts.push(part);
        }
    }

    if meta.summary.is_empty() {
        meta.summary = summary_parts.join(" ");
    }
    meta
}

fn split_session_tokens(rest: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escaped = false;
    for ch in rest.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_quotes => {
                current.push(ch);
                escaped = true;
            }
            '"' => {
                in_quotes = !in_quotes;
                current.push(ch);
            }
            ch if ch.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn parse_meta_value(raw: &str) -> String {
    if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
        serde_json::from_str::<String>(raw).unwrap_or_else(|_| raw.trim_matches('"').to_owned())
    } else {
        raw.to_owned()
    }
}

fn parse_worker_line(line: &str) -> Option<LiveSessionStatus> {
    let line = line.strip_prefix("- ").unwrap_or(line);
    parse_named_session(line)
}

fn parse_name_list(line: &str) -> Vec<String> {
    if line.trim() == "none" {
        return Vec::new();
    }
    line.split(',')
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_pods_line(line: &str) -> Vec<PodLine> {
    line.split('|')
        .filter_map(|part| {
            let part = part.trim();
            if part == "none" {
                return None;
            }
            let mut tokens = part.split_whitespace();
            let name = tokens.next()?.to_owned();
            let mut members = 0usize;
            let mut blocked = 0usize;
            let mut threads = 0usize;
            for token in tokens {
                if let Some(v) = token.strip_prefix("members=") {
                    members = v.parse().unwrap_or(0);
                } else if let Some(v) = token.strip_prefix("blocked=") {
                    blocked = v.parse().unwrap_or(0);
                } else if let Some(v) = token.strip_prefix("threads=") {
                    threads = v.parse().unwrap_or(0);
                }
            }
            Some(PodLine {
                name,
                members: vec![format!("{members} members")],
                blocked: if blocked > 0 {
                    vec![format!("{blocked} blocked")]
                } else {
                    vec![]
                },
                open_threads: threads,
            })
        })
        .collect()
}
