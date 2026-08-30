//! Tests for the TUI control status parser.
//!
//! These tests verify that `.sp/control/status.txt` parsing handles real-world
//! terminal output correctly — the core claim that "the terminal user interface
//! is extremely clean" depends on this parser working reliably.

use super::control_status::*;

#[test]
fn parses_minimal_status_file() {
    let input = "Session: test-mission\nUpdated: 2026-04-11T00:00:00Z\nWorkers: 4 | Directives: 0 | Mail: 0 | Validation: 0 | Stalls: 0 | Lease Conflicts: 0 | Protocol Reminders: 0 | Supervisor Health: 0 | Critical Failures: 0 | Crash Loops: 0\nSupervisor: supervisor-01 [progressing] summary=\"Planning\"\nWorkers:\n";
    let snap = parse_control_status(input);

    assert_eq!(snap.session_id.as_deref(), Some("test-mission"));
    assert_eq!(snap.updated_at.as_deref(), Some("2026-04-11T00:00:00Z"));
    assert!(snap.supervisor.is_some());
    assert_eq!(snap.watchdog.worker_count, 4);
    assert_eq!(snap.watchdog.directives, 0);
    assert_eq!(snap.watchdog.stall_interventions, 0);
    assert!(snap.blocked.is_empty());
    assert!(snap.validation_queue.is_empty());
    assert!(snap.contradictions.is_empty());
}

#[test]
fn parses_supervisor_status_with_full_meta() {
    let input = "Session: abc\nUpdated: now\nSupervisor: supervisor-01 [progressing] branch=planning agents=4 blocked=0 validating=2 summary=\"Active planning\" liveness=\"alive\" role=supervisor task=\"Plan mission\"\n";
    let snap = parse_control_status(input);

    let sup = snap.supervisor.expect("supervisor should be present");
    assert_eq!(sup.state, "progressing");
    assert_eq!(sup.branch.as_deref(), Some("planning"));
    assert_eq!(sup.agent_count, 4);
    assert_eq!(sup.blocked_count, 0);
    assert_eq!(sup.validating_count, 2);
    assert_eq!(sup.summary, "Active planning");
    assert_eq!(sup.liveness.as_deref(), Some("alive"));
    assert_eq!(sup.role.as_deref(), Some("supervisor"));
    assert_eq!(sup.task.as_deref(), Some("Plan mission"));
}

#[test]
fn parses_multiple_workers() {
    let input = "Session: abc\nUpdated: now\nWorkers: Workers: 3 | Directives: 5 | Mail: 2 | Validation: 1 | Stalls: 0 | Lease Conflicts: 0 | Protocol Reminders: 0 | Supervisor Health: 0 | Critical Failures: 0 | Crash Loops: 0\nWorkers:\n- Engineer-1 [progressing] branch=build outputs=12 summary=\"Building parser\"\n- Designer-1 [done_claimed] branch=ui outputs=8 summary=\"UI complete\"\n- Reviewer-1 [blocked] branch=review blocker=\"waiting for Engineer-1\" summary=\"Review pending\"\n";
    let snap = parse_control_status(input);

    assert_eq!(snap.workers.len(), 3);
    assert_eq!(snap.watchdog.directives, 5);
    assert_eq!(snap.watchdog.mail_routed, 2);
    assert_eq!(snap.watchdog.validation_challenges, 1);

    let eng1 = snap
        .workers
        .get("Engineer-1")
        .expect("Engineer-1 should exist");
    assert_eq!(eng1.state, "progressing");
    assert_eq!(eng1.output_chunks, 12);

    let designer = snap
        .workers
        .get("Designer-1")
        .expect("Designer-1 should exist");
    assert_eq!(designer.state, "done_claimed");

    let reviewer = snap
        .workers
        .get("Reviewer-1")
        .expect("Reviewer-1 should exist");
    assert_eq!(reviewer.state, "blocked");
}

#[test]
fn parses_empty_lists_as_none() {
    let input = "Session: abc\nUpdated: now\nWorkers: Workers: 1 | Directives: 0 | Mail: 0 | Validation: 0 | Stalls: 0 | Lease Conflicts: 0 | Protocol Reminders: 0 | Supervisor Health: 0 | Critical Failures: 0 | Crash Loops: 0\nBlocked: none\nValidation Queue: none\nContradictions: none\nMail Pressure: none\nProblems: none\nOwnership Gaps: none\nFirst-Status Incidents: none\nSystemic Incidents: none\nCrash Loops: none\n";
    let snap = parse_control_status(input);

    assert!(snap.blocked.is_empty());
    assert!(snap.validation_queue.is_empty());
    assert!(snap.contradictions.is_empty());
    assert!(snap.mail_pressure.is_empty());
    assert!(snap.problems.is_empty());
    assert!(snap.ownership_gaps.is_empty());
    assert!(snap.first_status_incidents.is_empty());
    assert!(snap.systemic_incidents.is_empty());
    assert!(snap.crash_loops.is_empty());
}

#[test]
fn parses_non_empty_lists() {
    let input = "Session: abc\nUpdated: now\nWorkers: Workers: 2 | Directives: 3 | Mail: 1 | Validation: 0 | Stalls: 1 | Lease Conflicts: 1 | Protocol Reminders: 0 | Supervisor Health: 0 | Critical Failures: 1 | Crash Loops: 0\nBlocked: Engineer-1, Designer-2\nValidation Queue: Reviewer-1\nContradictions: Engineer-3\nMail Pressure: Engineer-1\nProblems: Engineer-1, Engineer-3\nOwnership Gaps: src/main.rs\nFirst-Status Incidents: Engineer-2\nSystemic Incidents: pod-alpha\nCrash Loops: Engineer-1\n";
    let snap = parse_control_status(input);

    assert_eq!(
        snap.blocked,
        vec!["Engineer-1".to_string(), "Designer-2".to_string()]
    );
    assert_eq!(snap.validation_queue, vec!["Reviewer-1".to_string()]);
    assert_eq!(snap.contradictions, vec!["Engineer-3".to_string()]);
    assert_eq!(snap.mail_pressure, vec!["Engineer-1".to_string()]);
    assert_eq!(
        snap.problems,
        vec!["Engineer-1".to_string(), "Engineer-3".to_string()]
    );
    assert_eq!(snap.ownership_gaps, vec!["src/main.rs".to_string()]);
    assert_eq!(snap.first_status_incidents, vec!["Engineer-2".to_string()]);
    assert_eq!(snap.systemic_incidents, vec!["pod-alpha".to_string()]);
    assert_eq!(snap.crash_loops, vec!["Engineer-1".to_string()]);
    assert_eq!(snap.watchdog.stall_interventions, 1);
    assert_eq!(snap.watchdog.lease_conflicts, 1);
    assert_eq!(snap.watchdog.critical_failures, 1);
}

#[test]
fn parses_pods_line() {
    let input = "Session: abc\nUpdated: now\nWorkers: Workers: 4 | Directives: 0 | Mail: 0 | Validation: 0 | Stalls: 0 | Lease Conflicts: 0 | Protocol Reminders: 0 | Supervisor Health: 0 | Critical Failures: 0 | Crash Loops: 0\nPods: build members=3 blocked=1 threads=2 | review members=1 blocked=0 threads=0\n";
    let snap = parse_control_status(input);

    assert_eq!(snap.pods.len(), 2);
    assert_eq!(snap.pods[0].name, "build");
    assert_eq!(snap.pods[0].open_threads, 2);
    assert_eq!(snap.pods[1].name, "review");
    assert_eq!(snap.pods[1].open_threads, 0);
}

#[test]
fn parses_files_touched_in_worker_status() {
    let input = "Session: abc\nUpdated: now\nWorkers: Workers: 1 | Directives: 0 | Mail: 0 | Validation: 0 | Stalls: 0 | Lease Conflicts: 0 | Protocol Reminders: 0 | Supervisor Health: 0 | Critical Failures: 0 | Crash Loops: 0\nWorkers:\n- Engineer-1 [progressing] files=src/parser.rs,src/adapter.rs outputs=5 summary=\"Parsing\"\n";
    let snap = parse_control_status(input);

    let eng1 = snap
        .workers
        .get("Engineer-1")
        .expect("Engineer-1 should exist");
    assert_eq!(
        eng1.files_touched,
        vec!["src/parser.rs".to_string(), "src/adapter.rs".to_string()]
    );
}

#[test]
fn parses_interventions_and_stalls() {
    let input = "Session: abc\nUpdated: now\nWorkers: Workers: 1 | Directives: 2 | Mail: 1 | Validation: 1 | Stalls: 3 | Lease Conflicts: 0 | Protocol Reminders: 5 | Supervisor Health: 2 | Critical Failures: 0 | Crash Loops: 0\nWorkers:\n- Engineer-1 [progressing] stalls=3 interventions=2 outputs=10 summary=\"Working\"\n";
    let snap = parse_control_status(input);

    assert_eq!(snap.watchdog.stall_interventions, 3);
    assert_eq!(snap.watchdog.protocol_reminders, 5);
    assert_eq!(snap.watchdog.supervisor_health_events, 2);

    let eng1 = snap
        .workers
        .get("Engineer-1")
        .expect("Engineer-1 should exist");
    assert_eq!(eng1.consecutive_stall_failures, 3);
    assert_eq!(eng1.total_interventions, 2);
    assert_eq!(eng1.output_chunks, 10);
}

#[test]
fn handles_quoted_summary_with_spaces() {
    let input = "Session: abc\nUpdated: now\nWorkers: Workers: 1 | Directives: 0 | Mail: 0 | Validation: 0 | Stalls: 0 | Lease Conflicts: 0 | Protocol Reminders: 0 | Supervisor Health: 0 | Critical Failures: 0 | Crash Loops: 0\nWorkers:\n- Engineer-1 [progressing] summary=\"Building the parser and adapter\"\n";
    let snap = parse_control_status(input);

    let eng1 = snap
        .workers
        .get("Engineer-1")
        .expect("Engineer-1 should exist");
    assert_eq!(eng1.summary, "Building the parser and adapter");
}

#[test]
fn handles_empty_input_gracefully() {
    let snap = parse_control_status("");
    assert!(snap.session_id.is_none());
    assert!(snap.supervisor.is_none());
    assert_eq!(snap.watchdog.worker_count, 0);
    assert!(snap.workers.is_empty());
    assert!(snap.blocked.is_empty());
}

#[test]
fn handles_partial_line_input() {
    let input = "Session: abc\nUpdated: now\nWorkers: 1 | Directives: 0 | Mail: 0";
    let snap = parse_control_status(input);
    assert_eq!(snap.session_id.as_deref(), Some("abc"));
    // Partial watchdog line — parser may or may not extract partial values
    // This test documents current behavior for partial lines
    assert!(snap.workers.is_empty());
}

#[test]
fn parses_mail_thread_count() {
    let input = "Session: abc\nUpdated: now\nWorkers: Workers: 1 | Directives: 0 | Mail: 3 | Validation: 0 | Stalls: 0 | Lease Conflicts: 0 | Protocol Reminders: 0 | Supervisor Health: 0 | Critical Failures: 0 | Crash Loops: 0\nWorkers:\n- Engineer-1 [progressing] mail=5 outputs=10 summary=\"Working\"\n";
    let snap = parse_control_status(input);

    assert_eq!(snap.watchdog.mail_routed, 3);
    let eng1 = snap
        .workers
        .get("Engineer-1")
        .expect("Engineer-1 should exist");
    assert_eq!(eng1.mail_thread_count, 5);
}
