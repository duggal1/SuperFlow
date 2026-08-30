mod agent_state;
mod supervisor_state;
mod team_state;

use crate::model::WorkerPacket;

pub use team_state::{
    build_supervisor_cards, rebalance_worker_supervision, routed_supervisor_for_worker,
};

pub fn recommended_supervisor_count(worker_count: usize) -> usize {
    // Hard thresholds for usability:
    // - >8 workers => at least 2 supervisors
    // - >16 workers => at least 3 supervisors
    // - >24 workers => at least 4 supervisors
    if worker_count <= 8 {
        1
    } else if worker_count <= 16 {
        2
    } else if worker_count <= 24 {
        3
    } else {
        4
    }
}

pub fn supervisor_branch_name(index: usize) -> String {
    format!("supervisor-{:02}", index + 1)
}

pub fn assign_packets_to_supervisors(
    packets: &[WorkerPacket],
    supervisor_count: usize,
) -> Vec<Vec<String>> {
    let mut buckets = vec![Vec::new(); supervisor_count.max(1)];
    for (idx, packet) in packets.iter().enumerate() {
        let bucket = idx % buckets.len();
        buckets[bucket].push(packet.display_name.clone());
    }
    buckets
}

pub fn branch_prompt(base_prompt: &str, branch_name: &str, assigned_workers: &[String]) -> String {
    let workers = if assigned_workers.is_empty() {
        "(none)".to_owned()
    } else {
        assigned_workers.join(", ")
    };
    format!(
        "{base_prompt}\n\n---\n\n# SUPERVISOR TEAM BRANCH\n\n- You are {branch_name}.\n- You are an active supervisor, not a standby.\n- Your primary responsibility is supervising ONLY these workers: {workers}\n- Do not issue actions for unassigned workers; instead, report the issue concisely in a supervisor notice.\n- Always act: if a worker is stalled, weak, looping, or fake-done, issue a corrective action.\n",
        branch_name = branch_name,
        workers = workers
    )
}
