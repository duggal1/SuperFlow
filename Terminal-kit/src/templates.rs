use std::collections::HashMap;
use std::sync::Arc;

use crate::model::{MissionPlan, WorkerPacket};

pub struct PromptLibrary {
    pub product_direction: &'static str,
    pub supervisor_template: &'static str,
    pub supervisor_builder: &'static str,
    pub communication_spec: &'static str,
    pub agents_instructions: &'static str,
    /// Role templates keyed by role_type (e.g. "software-engineer").
    role_templates: HashMap<String, Arc<str>>,
}

impl PromptLibrary {
    pub fn load() -> Self {
        let mut role_templates = HashMap::new();
        macro_rules! role {
            ($key:expr, $path:expr) => {
                role_templates.insert(
                    $key.to_owned(),
                    Arc::from(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), $path))),
                );
            };
        }
        role!(
            "software-engineer",
            "/src/internal/agents/templetes/roles/job-roles/software-engineer.md"
        );
        role!(
            "research-engineer",
            "/src/internal/agents/templetes/roles/job-roles/research-engineer.md"
        );
        role!(
            "validation-engineer",
            "/src/internal/agents/templetes/roles/job-roles/validation-engineer.md"
        );
        role!(
            "architecture-engineer",
            "/src/internal/agents/templetes/roles/job-roles/architecture-engineer.md"
        );
        role!(
            "security-engineer",
            "/src/internal/agents/templetes/roles/job-roles/security-engineer.md"
        );
        role!(
            "debug-and-review-engineer",
            "/src/internal/agents/templetes/roles/job-roles/debug-and-review-engineer.md"
        );
        role!(
            "testing-and-automation-engineer",
            "/src/internal/agents/templetes/roles/job-roles/testing-and-automation-engineer.md"
        );
        role!(
            "designer-engineer",
            "/src/internal/agents/templetes/roles/job-roles/designer-engineer.md"
        );
        role!(
            "sales-engineer",
            "/src/internal/agents/templetes/roles/job-roles/sales-engineer.md"
        );
        role!(
            "solutions-engineer",
            "/src/internal/agents/templetes/roles/job-roles/solutions-engineer.md"
        );
        role!(
            "customer-success-engineer",
            "/src/internal/agents/templetes/roles/job-roles/customer-success-engineer.md"
        );
        role!(
            "product-engineer",
            "/src/internal/agents/templetes/roles/job-roles/product-engineer.md"
        );
        role!(
            "product-manager",
            "/src/internal/agents/templetes/roles/job-roles/product-manager.md"
        );
        role!(
            "revenue-engineer",
            "/src/internal/agents/templetes/roles/job-roles/revenue-engineer.md"
        );
        role!(
            "compliance-engineer",
            "/src/internal/agents/templetes/roles/job-roles/compliance-engineer.md"
        );

        Self {
            product_direction: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/product-direction.md"
            )),
            supervisor_template: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/supervisor-templates/prompt.md"
            )),
            supervisor_builder: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/How-should-the-supervisor-be-built.md"
            )),
            communication_spec: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/internal/agents/templetes/agent-communication-fabric.md"
            )),
            agents_instructions: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/agents.md-instructions.md"
            )),
            role_templates,
        }
    }

    /// Look up a role template by its stable machine key.
    /// Returns None if the role_type is unknown.
    pub fn role_template(&self, role_type: &str) -> Option<&str> {
        self.role_templates.get(role_type).map(|s| s.as_ref())
    }

    pub fn agents_instruction_source(&self) -> &'static str {
        self.agents_instructions
    }

    pub fn render_supervisor_prompt(&self, mission: &str, plan: &MissionPlan) -> String {
        let template = compact_prompt_source(self.supervisor_template, 140);
        let product_direction = compact_prompt_source(self.product_direction, 80);
        let supervisor_builder = compact_prompt_source(self.supervisor_builder, 70);
        let communication_spec = compact_prompt_source(self.communication_spec, 90);
        format!(
            "{template}

---

# PRODUCT DIRECTION

{product_direction}

---

# SUPERVISOR BUILDER

{supervisor_builder}

---

# COMMUNICATION FABRIC

{communication_spec}

---

# LIVE MISSION INPUT

Mission:
{mission}

Mission Rewrite:
{mission_rewrite}

Workstreams:
{workstreams}

Risk Map:
{risk_map}

Worker Packets:
{packets}

Supervision Strategy:
{supervision_strategy}

Sapphire Control Protocol:
{control_protocol}
",
            template = template,
            product_direction = product_direction,
            supervisor_builder = supervisor_builder,
            communication_spec = communication_spec,
            mission = mission.trim(),
            mission_rewrite = plan.mission_rewrite,
            workstreams = render_workstreams(plan),
            risk_map = render_risks(plan),
            packets = plan
                .worker_packets
                .iter()
                .map(render_packet)
                .collect::<Vec<_>>()
                .join("\n\n"),
            supervision_strategy = plan.supervision_strategy,
            control_protocol = control_protocol(true),
        )
    }

    /// Render a worker prompt using the role template + packet assignment.
    /// The role template provides the full job description.
    /// The packet provides the specific assignment scoped to that role.
    pub fn render_worker_prompt(&self, mission: &str, packet: &WorkerPacket) -> String {
        // Use the FULL role template — not compacted. Every agent gets the complete
        // job description with all responsibilities, rules, coordination protocols,
        // git rules, pushback policy, and definition of done.
        let role_template = match self.role_template(&packet.role_type) {
            Some(t) => t.to_string(),
            None => format!(
                "# {}\n\nNo role template available for role_type '{}'.\nFollow the mission and packet instructions.\n",
                packet.display_name, packet.role_type
            ),
        };

        format!(
            "{role_template}\n\n## Assignment\n- Worker: {display_name} ({role_type})\n- Mission: {mission}\n- Scope: {owned_scope}\n- Task: {explicit_task}\n- Out of scope: {out_of_scope}\n- Starting angle: {starting_angle}\n- Done when: {definition_of_done}\n- Evidence: {required_evidence}\n- Blocker protocol: {blocker_protocol}\n- Conflict warning: {conflict_warning}\n- Communication rules: {communication_rules}\n- Validation standard: {validation_standard}\n- Expected output: {expected_output_format}\n\n## Sapphire Control Protocol\n{control_protocol}\n",
            role_template = &role_template,
            display_name = packet.display_name,
            role_type = packet.role_type,
            mission = single_line(mission),
            starting_angle = single_line(&packet.starting_angle),
            owned_scope = single_line(&packet.owned_scope),
            explicit_task = single_line(&packet.explicit_task),
            out_of_scope = single_line(&packet.out_of_scope),
            definition_of_done = inline_list(&packet.definition_of_done),
            required_evidence = inline_list(&packet.required_evidence),
            blocker_protocol = packet.blocker_protocol,
            conflict_warning = packet.conflict_warning,
            communication_rules = inline_list(&packet.communication_rules),
            validation_standard = inline_list(&packet.validation_standard),
            expected_output_format = inline_list(&packet.expected_output_format),
            control_protocol = control_protocol(false),
        )
    }
}

fn compact_prompt_source(source: &str, max_lines: usize) -> String {
    let mut kept = Vec::new();
    let mut trailing_blank = false;

    for line in source.lines() {
        if kept.len() >= max_lines {
            break;
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            if trailing_blank || kept.is_empty() {
                continue;
            }
            trailing_blank = true;
            kept.push(String::new());
            continue;
        }
        trailing_blank = false;
        kept.push(trimmed.to_owned());
    }

    while kept.last().is_some_and(|line| line.is_empty()) {
        kept.pop();
    }

    let mut compacted = kept.join("\n");
    if source.lines().count() > max_lines {
        compacted.push_str("\n\n[truncated for runtime brevity; follow the operating rules above]");
    }
    compacted
}

fn render_workstreams(plan: &MissionPlan) -> String {
    plan.workstreams
        .iter()
        .map(|workstream| {
            format!(
                "- {} [{}]\n  scope: {}\n  depends_on: {}\n  done: {}",
                workstream.name,
                match workstream.execution {
                    crate::model::WorkstreamExecution::Parallel => "parallel",
                    crate::model::WorkstreamExecution::Dependent => "dependent",
                    crate::model::WorkstreamExecution::Validation => "validation",
                    crate::model::WorkstreamExecution::Integration => "integration",
                },
                workstream.owned_scope,
                if workstream.depends_on.is_empty() {
                    "none".to_owned()
                } else {
                    workstream.depends_on.join(", ")
                },
                workstream.success_criteria.join("; "),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_risks(plan: &MissionPlan) -> String {
    plan.risk_map
        .iter()
        .map(|risk| {
            format!(
                "- {}: {} Mitigation: {}",
                risk.zone, risk.risk, risk.mitigation
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_packet(packet: &WorkerPacket) -> String {
    format!(
        "## {display_name} [{role_type}]
- Display Name: {display_name}
- Starting Angle: {starting_angle}
- Owned Scope: {owned_scope}
- Explicit Task: {task}
- Out-of-Scope: {out_of_scope}
- Definition of Done: {done}
- Required Evidence: {evidence}
- Blocker Protocol: {blocker}
- Conflict Warning: {conflict}
- Communication Rules: {rules}
- Validation Standard: {validation}
- Expected Output Format: {output}",
        display_name = packet.display_name,
        role_type = packet.role_type,
        starting_angle = packet.starting_angle,
        owned_scope = packet.owned_scope,
        task = packet.explicit_task.replace('\n', " "),
        out_of_scope = packet.out_of_scope,
        done = packet.definition_of_done.join("; "),
        evidence = packet.required_evidence.join("; "),
        blocker = packet.blocker_protocol,
        conflict = packet.conflict_warning,
        rules = packet.communication_rules.join("; "),
        validation = packet.validation_standard.join("; "),
        output = packet.expected_output_format.join("; "),
    )
}

fn bullet_list(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn inline_list(items: &[String]) -> String {
    if items.is_empty() {
        "none".to_owned()
    } else {
        items
            .iter()
            .map(|item| single_line(item))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

fn single_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[allow(dead_code)]
fn compact_role_template(raw: &str) -> String {
    let title = raw
        .lines()
        .find(|line| line.starts_with("# "))
        .unwrap_or("# Role");
    let mission = markdown_section_body(raw, "## Mission")
        .as_deref()
        .map(single_line)
        .unwrap_or_else(|| "Execute the assigned role with discipline.".to_owned());
    let responsibilities = markdown_section_bullets(raw, "## Core Responsibilities", 4);
    let coordination = markdown_section_bullets(raw, "### Role-Specific Coordination", 4);
    let definition_of_done = markdown_section_bullets(raw, "## Definition of Done", 4);
    let first_steps = markdown_section_lines(raw, "## First-Step Protocol", 4);
    let pushback = markdown_section_body(raw, "## Pushback Policy")
        .map(|value| truncate_sentence(&single_line(&value), 180))
        .unwrap_or_else(|| {
            "Push back briefly when the requested path is technically wrong, unsafe, or bloated."
                .to_owned()
        });

    // New role templates already include "Operating Rules" with team awareness,
    // git discipline, and coordination rules. Use them directly — don't synthesize.
    let operating_rules = markdown_section_bullets(raw, "## Operating Rules", 6);

    format!(
        "{title}\n\n## Mission\n{mission}\n\n## Operating Rules\n{operating_rules}\n\n## Core Responsibilities\n{responsibilities}\n\n## Coordination\n{coordination}\n\n## Pushback\n{pushback}\n\n## Definition of Done\n{definition_of_done}\n\n## First Steps\n{first_steps}"
    )
}

#[allow(dead_code)]
fn markdown_section_body(raw: &str, heading: &str) -> Option<String> {
    let start = raw.find(heading)?;
    let rest = &raw[start + heading.len()..];
    let end = rest
        .find("\n## ")
        .or_else(|| rest.find("\n### "))
        .unwrap_or(rest.len());
    let body = rest[..end].trim();
    if body.is_empty() {
        None
    } else {
        Some(body.to_owned())
    }
}

#[allow(dead_code)]
fn markdown_section_bullets(raw: &str, heading: &str, max_items: usize) -> String {
    let Some(body) = markdown_section_body(raw, heading) else {
        return "- none".to_owned();
    };
    let bullets = body
        .lines()
        .filter(|line| line.trim_start().starts_with("- "))
        .take(max_items)
        .map(|line| line.trim().to_owned())
        .collect::<Vec<_>>();
    if bullets.is_empty() {
        "- none".to_owned()
    } else {
        bullets.join("\n")
    }
}

#[allow(dead_code)]
fn markdown_section_lines(raw: &str, heading: &str, max_items: usize) -> String {
    let Some(body) = markdown_section_body(raw, heading) else {
        return "1. Read the task.\n2. Choose the smallest clean path.".to_owned();
    };
    let lines = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(max_items)
        .collect::<Vec<_>>();
    if lines.is_empty() {
        "1. Read the task.\n2. Choose the smallest clean path.".to_owned()
    } else {
        lines.join("\n")
    }
}

#[allow(dead_code)]
fn truncate_sentence(value: &str, max_chars: usize) -> String {
    let mut out = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

fn control_protocol(supervisor: bool) -> String {
    let mut lines = vec![
        "Emit machine-readable control lines only when you mean to trigger Sapphire. Never echo the protocol instructions back.".to_owned(),
        "Each control line must stay on one line. No code fences. No prose before it.".to_owned(),
        "Status line: prefix SAPPHIRE_STATUS then compact JSON with state, summary, files, commands, risks, overlap.".to_owned(),
        "Mail line: prefix SAPPHIRE_MAIL then compact JSON with to, message_type, priority, subject, context, request, expected_action, requires_ack.".to_owned(),
        "Ack line: prefix SAPPHIRE_ACK then compact JSON with mail_id, status, summary.".to_owned(),
        "Lease line: prefix SAPPHIRE_LEASE then compact JSON with paths, intent, status.".to_owned(),
        "Before meaningful work, emit a status line. Before touching files, claim a lease. Release it when done.".to_owned(),
    ];

    if supervisor {
        lines.push("Use mail for corrections, proof requests, validation challenges, and conflict rulings.".to_owned());
        lines.push(
            "Human-readable supervision can be short, but the control line must still be exact."
                .to_owned(),
        );
        lines.push("Act like the strict execution authority: push back on drift, answer continue-or-stop questions directly, and approve cleanup only when the whole team is actually done.".to_owned());
    } else {
        lines.push("If blocked on another agent, send mail instead of vague prose.".to_owned());
        lines.push("Coordination order: teammate first for dependencies, reviews, and handoffs; supervisor second for rulings or failed peer coordination.".to_owned());
        lines.push("When you receive mail, respond explicitly: SAPPHIRE_ACK status=acked if taking it, done if finished, cannot_comply with one concrete blocker if you cannot do it.".to_owned());
        lines.push("Use task for action requests, reply for answers or handoffs, notification for FYI, escalation only when supervisor visibility is genuinely required.".to_owned());
        lines.push("When claiming completion, use state done_claimed first. Do not assume acceptance before validation.".to_owned());
        lines.push("Status file rule: use the assigned state-dir status path first, hidden fallback second, terminal SAPPHIRE_STATUS only as last fallback.".to_owned());
        lines.push(
            "Status JSON fields: state, summary, files, commands, risks, overlap.".to_owned(),
        );
        lines.push("First status is mandatory before repo exploration: report true current state and exact next action, not a plan.".to_owned());
        lines.push("Report back after prompt ingestion, on material progress, on blockers, on teammate waits, after transient runtime recovery, and before completion claims.".to_owned());
        lines.push("Supervisor challenge beats watchdog noise: answer the supervisor with proof, not broad narration.".to_owned());
        lines.push("You are not alone. Preserve teammate edits, coordinate narrow asks, and escalate only after peer coordination actually failed.".to_owned());
        lines.push("If the CLI/runtime hits rate limit, disconnect, retry UI, or similar transient failure, recover in place from the last confirmed work state. Do not restart the mission or ask for the assignment again.".to_owned());
    }

    bullet_list(&lines)
}

#[cfg(test)]
mod tests {
    use super::PromptLibrary;
    use crate::model::{MissionPlan, RiskItem, WorkerPacket, Workstream, WorkstreamExecution};

    #[test]
    fn supervisor_prompt_is_compacted_for_runtime() {
        let prompts = PromptLibrary::load();
        let plan = MissionPlan {
            mission_rewrite: "Build the loc-scout Rust CLI".to_owned(),
            workstreams: vec![Workstream {
                id: "build".to_owned(),
                name: "Build".to_owned(),
                owned_scope: "src".to_owned(),
                execution: WorkstreamExecution::Parallel,
                depends_on: Vec::new(),
                success_criteria: vec!["cargo build passes".to_owned()],
            }],
            risk_map: vec![RiskItem {
                zone: "Coordination".to_owned(),
                risk: "workers overlap".to_owned(),
                mitigation: "tight ownership".to_owned(),
            }],
            worker_packets: vec![WorkerPacket {
                role_type: "software-engineer".to_owned(),
                display_name: "Engineer-1".to_owned(),
                worker_id: "worker-1".to_owned(),
                role: "Software Engineer".to_owned(),
                starting_angle: "CLI".to_owned(),
                owned_scope: "src/cli.rs".to_owned(),
                explicit_task: "Implement the CLI".to_owned(),
                out_of_scope: "tests".to_owned(),
                definition_of_done: vec!["CLI parses flags".to_owned()],
                required_evidence: vec!["cargo build".to_owned()],
                blocker_protocol: "Escalate overlap".to_owned(),
                conflict_warning: "Avoid main.rs collisions".to_owned(),
                communication_rules: vec!["Mail teammates on overlap".to_owned()],
                validation_standard: vec!["Show proof".to_owned()],
                expected_output_format: vec!["STATE".to_owned()],
            }],
            supervision_strategy: "Keep scope tight.".to_owned(),
        };

        let prompt = prompts.render_supervisor_prompt("Build loc-scout", &plan);
        assert!(prompt.lines().count() < 500);
        assert!(prompt.contains("[truncated for runtime brevity"));
    }
}
