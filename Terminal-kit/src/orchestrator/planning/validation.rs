use std::collections::BTreeSet;

use anyhow::Result;

use crate::model::MissionPlan;

/// Hard gate that prevents “same mission broadcast” failures.
///
/// This rejects plans where worker packets are near-duplicates or overlap on scope.
/// No trimming, no auto-fixing: if the supervisor fails to decompose, the launch fails.
pub fn validate_supervisor_plan_packets(
    plan: &MissionPlan,
    expected_worker_packets: usize,
    planner_label: &str,
) -> Result<()> {
    if plan.worker_packets.len() != expected_worker_packets {
        anyhow::bail!(
            "{planner_label} returned an invalid plan: expected exactly {} worker packets, got {}",
            expected_worker_packets,
            plan.worker_packets.len()
        );
    }

    // Pairwise similarity + scope overlap checks.
    for i in 0..plan.worker_packets.len() {
        let a = &plan.worker_packets[i];
        let a_scope = normalize_scope_tokens(&a.owned_scope);
        let a_primary_scope = substantive_scope_tokens(&a_scope);
        for j in (i + 1)..plan.worker_packets.len() {
            let b = &plan.worker_packets[j];
            let task_sim = text_similarity_sim(&a.explicit_task, &b.explicit_task);
            let b_scope = normalize_scope_tokens(&b.owned_scope);
            let b_primary_scope = substantive_scope_tokens(&b_scope);
            let scope_sim = scope_similarity(&a_primary_scope, &b_primary_scope);

            // Reject “same task/same scope” and also “very similar task/scope”.
            if (task_sim >= 0.78 && scope_sim >= 0.78) || task_sim >= 0.9 || scope_sim >= 0.9 {
                anyhow::bail!(
                    "{planner_label} produced non-differentiated worker packets: '{}' and '{}' are too similar (task_sim={:.2}, scope_sim={:.2}). Each worker must have a distinct objective and distinct owned scope.",
                    a.display_name,
                    b.display_name,
                    task_sim,
                    scope_sim
                );
            }

            let overlap: Vec<String> = a_scope.intersection(&b_scope).cloned().collect::<Vec<_>>();
            let actionable_overlap: Vec<String> = overlap
                .iter()
                .filter(|token| !is_shareable_coordination_scope(token))
                .cloned()
                .collect();
            if !actionable_overlap.is_empty() {
                anyhow::bail!(
                    "{planner_label} produced overlapping owned_scope between '{}' and '{}': overlap={:?}. Owned scopes must be disjoint unless overlap is explicitly intended.",
                    a.display_name,
                    b.display_name,
                    actionable_overlap
                );
            }
            if !overlap.is_empty() && (a_primary_scope.is_empty() || b_primary_scope.is_empty()) {
                anyhow::bail!(
                    "{planner_label} assigned coordination-file-only ownership to '{}' or '{}': overlap={:?}. Shared manifest/config files cannot be the full owned_scope for a worker packet.",
                    a.display_name,
                    b.display_name,
                    overlap
                );
            }
        }
    }

    // Role-type guardrails:
    // - QA/validation roles must not be given builder scopes.
    // - Designer roles must not be given backend/scanner ownership.
    for packet in &plan.worker_packets {
        let role = packet.role_type.trim().to_ascii_lowercase();
        let name = packet.display_name.trim().to_ascii_lowercase();
        let owned = packet.owned_scope.to_ascii_lowercase();
        let task = packet.explicit_task.to_ascii_lowercase();

        if !display_name_matches_role(&packet.display_name, &role) {
            anyhow::bail!(
                "{planner_label} produced a role/display mismatch: '{}' does not match role_type '{}'. Use canonical names like Engineer-N, QA-N, Product-N, Designer-N, Architect-N, Security-N, or Validator-N.",
                packet.display_name,
                packet.role_type
            );
        }

        if !owned_scope_is_concrete(&packet.owned_scope) {
            anyhow::bail!(
                "{planner_label} produced a vague owned_scope for '{}': '{}'. owned_scope must point to concrete repo paths or explicit artifact files.",
                packet.display_name,
                packet.owned_scope
            );
        }

        if contains_vague_prompt_language(&packet.explicit_task)
            || contains_vague_prompt_language(&packet.owned_scope)
        {
            anyhow::bail!(
                "{planner_label} copied vague mission language into '{}' packet fields. Rewrite subjective wording into concrete, testable engineering language.",
                packet.display_name
            );
        }

        let is_testing = role.contains("testing-and-automation-engineer")
            || role.contains("validation-engineer")
            || name.starts_with("qa-")
            || name.starts_with("validator-");
        if is_testing {
            if owned.contains("src/") || owned.contains("src\\") {
                anyhow::bail!(
                    "{planner_label} assigned builder scope to a QA/validation role ({}): owned_scope='{}'. QA/validation roles must own tests-only scope.",
                    packet.display_name,
                    packet.owned_scope
                );
            }
            if task.contains("implement") || task.contains("build ") || task.contains("rewrite") {
                anyhow::bail!(
                    "{planner_label} assigned a builder task to a QA/validation role ({}): explicit_task='{}'. QA/validation roles must be validation/testing-only.",
                    packet.display_name,
                    packet.explicit_task
                );
            }
        }

        let is_designer = role.contains("designer-engineer") || name.starts_with("designer-");
        if is_designer {
            if owned.contains("scanner") || owned.contains("languages") || owned.contains("travers")
            {
                anyhow::bail!(
                    "{planner_label} assigned scanner/core ownership to a designer role ({}): owned_scope='{}'. Designer roles must not own scanner/core logic unless explicitly intended.",
                    packet.display_name,
                    packet.owned_scope
                );
            }
            if task.contains("scanner")
                || task.contains("count")
                || task.contains("travers")
                || task.contains("ignore")
            {
                anyhow::bail!(
                    "{planner_label} assigned scanner/core work to a designer role ({}): explicit_task='{}'. Designer roles must focus on UI/UX and presentation.",
                    packet.display_name,
                    packet.explicit_task
                );
            }
        }
    }

    Ok(())
}

fn normalize_scope_tokens(scope: &str) -> BTreeSet<String> {
    scope
        .split(|ch: char| ch == ',' || ch == '\n' || ch == ';')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_ascii_lowercase()
        })
        .collect()
}

fn substantive_scope_tokens(tokens: &BTreeSet<String>) -> BTreeSet<String> {
    tokens
        .iter()
        .filter(|token| !is_shareable_coordination_scope(token))
        .cloned()
        .collect()
}

fn is_shareable_coordination_scope(token: &str) -> bool {
    matches!(
        token.trim().to_ascii_lowercase().as_str(),
        "cargo.toml"
            | "./cargo.toml"
            | "cargo.lock"
            | "./cargo.lock"
            | "package.json"
            | "./package.json"
            | "package-lock.json"
            | "./package-lock.json"
            | "pnpm-lock.yaml"
            | "./pnpm-lock.yaml"
            | "bun.lock"
            | "./bun.lock"
            | "bun.lockb"
            | "./bun.lockb"
            | "go.mod"
            | "./go.mod"
            | "go.sum"
            | "./go.sum"
            | "composer.json"
            | "./composer.json"
            | "composer.lock"
            | "./composer.lock"
            | "pyproject.toml"
            | "./pyproject.toml"
            | "poetry.lock"
            | "./poetry.lock"
            | "requirements.txt"
            | "./requirements.txt"
            | "requirements-dev.txt"
            | "./requirements-dev.txt"
            | "turbo.json"
            | "./turbo.json"
            | "tsconfig.json"
            | "./tsconfig.json"
            | ".gitignore"
            | "./.gitignore"
            | ".editorconfig"
            | "./.editorconfig"
    )
}

fn scope_similarity(a: &BTreeSet<String>, b: &BTreeSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let left = a.iter().map(String::as_str).collect::<Vec<_>>().join(" ");
    let right = b.iter().map(String::as_str).collect::<Vec<_>>().join(" ");
    text_similarity_sim(&left, &right)
}

fn display_name_matches_role(display_name: &str, role_type: &str) -> bool {
    let expected_prefix = match role_type {
        "software-engineer" => "engineer-",
        "testing-and-automation-engineer" => "qa-",
        "validation-engineer" => "validator-",
        "product-engineer" => "product-",
        "product-manager" => "productmanager-",
        "designer-engineer" => "designer-",
        "architecture-engineer" => "architect-",
        "security-engineer" => "security-",
        "research-engineer" => "researcher-",
        "debug-and-review-engineer" => "reviewer-",
        "revenue-engineer" => "revenue-",
        "sales-engineer" => "sales-",
        "solutions-engineer" => "solutions-",
        "customer-success-engineer" => "customersuccess-",
        "compliance-engineer" => "compliance-",
        _ => "",
    };
    display_name
        .trim()
        .to_ascii_lowercase()
        .starts_with(expected_prefix)
}

fn owned_scope_is_concrete(scope: &str) -> bool {
    normalize_scope_tokens(scope).iter().any(|token| {
        token.contains('/')
            || token.contains('\\')
            || token.ends_with(".rs")
            || token.ends_with(".ts")
            || token.ends_with(".tsx")
            || token.ends_with(".js")
            || token.ends_with(".jsx")
            || token.ends_with(".go")
            || token.ends_with(".py")
            || token.ends_with(".md")
            || token.ends_with(".json")
            || token.ends_with(".toml")
            || token.ends_with(".yaml")
            || token.ends_with(".yml")
    })
}

fn contains_vague_prompt_language(text: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    [
        "thunder part",
        "really powerful",
        "extremely powerful",
        "enterprise quality",
        "enterprise readiness",
        "assessment only",
        "cleanliness",
        "super clean",
        "legitimately",
    ]
    .iter()
    .any(|needle| lowered.contains(needle))
}

/// Token-set Jaccard similarity used to reject near-duplicate packets.
fn text_similarity_sim(a: &str, b: &str) -> f64 {
    let a_lower = a.to_ascii_lowercase();
    let b_lower = b.to_ascii_lowercase();
    let words_a: std::collections::HashSet<_> = a_lower.split_whitespace().collect();
    let words_b: std::collections::HashSet<_> = b_lower.split_whitespace().collect();
    if words_a.is_empty() && words_b.is_empty() {
        return 1.0;
    }
    if words_a.is_empty() || words_b.is_empty() {
        return 0.0;
    }
    let intersection = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();
    if union == 0 {
        return 0.0;
    }
    intersection as f64 / union as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{MissionPlan, RiskItem, WorkerPacket, Workstream, WorkstreamExecution};

    fn packet(
        display_name: &str,
        role_type: &str,
        owned_scope: &str,
        explicit_task: &str,
        out_of_scope: &str,
    ) -> WorkerPacket {
        WorkerPacket {
            role_type: role_type.to_owned(),
            display_name: display_name.to_owned(),
            worker_id: display_name.to_owned(),
            role: display_name.to_owned(),
            starting_angle: "start".to_owned(),
            owned_scope: owned_scope.to_owned(),
            explicit_task: explicit_task.to_owned(),
            out_of_scope: out_of_scope.to_owned(),
            definition_of_done: vec!["done".to_owned()],
            required_evidence: vec!["evidence".to_owned()],
            blocker_protocol: "block".to_owned(),
            conflict_warning: "claim".to_owned(),
            communication_rules: vec![],
            validation_standard: vec![],
            expected_output_format: vec![],
        }
    }

    fn plan(packets: Vec<WorkerPacket>) -> MissionPlan {
        MissionPlan {
            mission_rewrite: "x".to_owned(),
            workstreams: vec![Workstream {
                id: "ws-1".to_owned(),
                name: "ws".to_owned(),
                execution: WorkstreamExecution::Parallel,
                owned_scope: "scope".to_owned(),
                success_criteria: vec!["ok".to_owned()],
                depends_on: vec![],
            }],
            risk_map: vec![RiskItem {
                zone: "z".to_owned(),
                risk: "r".to_owned(),
                mitigation: "m".to_owned(),
            }],
            worker_packets: packets,
            supervision_strategy: "s".to_owned(),
        }
    }

    #[test]
    fn rejects_duplicate_scope() {
        let plan = plan(vec![
            packet(
                "Engineer-1",
                "software-engineer",
                "src/a.rs",
                "implement a",
                "none",
            ),
            packet(
                "Engineer-2",
                "software-engineer",
                "src/a.rs",
                "implement b",
                "none",
            ),
        ]);
        let err = validate_supervisor_plan_packets(&plan, 2, "test")
            .err()
            .expect("should fail");
        let message = err.to_string().to_ascii_lowercase();
        assert!(
            message.contains("overlap") || message.contains("non-differentiated"),
            "unexpected error: {message}"
        );
    }

    #[test]
    fn rejects_testing_role_owning_src() {
        let plan = plan(vec![
            packet(
                "Engineer-1",
                "software-engineer",
                "src/a.rs",
                "implement a",
                "none",
            ),
            packet(
                "QA-1",
                "testing-and-automation-engineer",
                "src/main.rs",
                "write tests",
                "none",
            ),
        ]);
        let err = validate_supervisor_plan_packets(&plan, 2, "test")
            .err()
            .expect("should fail");
        assert!(
            err.to_string()
                .to_ascii_lowercase()
                .contains("qa/validation")
        );
    }

    #[test]
    fn rejects_role_display_name_mismatch() {
        let plan = plan(vec![
            packet(
                "Engineer-1",
                "software-engineer",
                "src/a.rs",
                "implement a",
                "none",
            ),
            packet(
                "Engineer-2",
                "product-engineer",
                "docs/product.md",
                "define product lane",
                "none",
            ),
        ]);
        let err = validate_supervisor_plan_packets(&plan, 2, "test")
            .err()
            .expect("should fail");
        assert!(
            err.to_string()
                .to_ascii_lowercase()
                .contains("role/display")
        );
    }

    #[test]
    fn rejects_vague_scope_and_prompt_language() {
        let plan = plan(vec![
            packet(
                "Engineer-1",
                "software-engineer",
                "src/a.rs",
                "implement a",
                "none",
            ),
            packet(
                "Product-1",
                "product-engineer",
                "enterprise readiness and grid assessment only",
                "verify system is extremely thunder part",
                "none",
            ),
        ]);
        let err = validate_supervisor_plan_packets(&plan, 2, "test")
            .err()
            .expect("should fail");
        let message = err.to_string().to_ascii_lowercase();
        assert!(
            message.contains("vague owned_scope") || message.contains("vague mission language")
        );
    }

    #[test]
    fn allows_shared_coordination_manifest_overlap() {
        let plan = plan(vec![
            packet(
                "Engineer-1",
                "software-engineer",
                "src/main.rs, Cargo.toml",
                "implement runtime changes",
                "none",
            ),
            packet(
                "Security-1",
                "security-engineer",
                "threat-model.md, Cargo.toml",
                "audit dependency and runtime risk",
                "none",
            ),
        ]);

        validate_supervisor_plan_packets(&plan, 2, "test").expect("shareable overlap should pass");
    }

    #[test]
    fn still_rejects_real_source_overlap_even_with_manifest_overlap() {
        let plan = plan(vec![
            packet(
                "Engineer-1",
                "software-engineer",
                "src/main.rs, Cargo.toml",
                "implement runtime changes",
                "none",
            ),
            packet(
                "Security-1",
                "security-engineer",
                "src/main.rs, threat-model.md, Cargo.toml",
                "audit dependency and runtime risk",
                "none",
            ),
        ]);

        let err = validate_supervisor_plan_packets(&plan, 2, "test")
            .err()
            .expect("real overlap should still fail");
        assert!(err.to_string().to_ascii_lowercase().contains("overlap"));
    }

    #[test]
    fn rejects_coordination_only_scope_overlap() {
        let plan = plan(vec![
            packet(
                "Engineer-1",
                "software-engineer",
                "Cargo.toml",
                "update crate dependencies",
                "none",
            ),
            packet(
                "Security-1",
                "security-engineer",
                "Cargo.toml",
                "audit dependencies",
                "none",
            ),
        ]);

        let err = validate_supervisor_plan_packets(&plan, 2, "test")
            .err()
            .expect("coordination-only overlap should fail");
        assert!(
            err.to_string()
                .to_ascii_lowercase()
                .contains("coordination-file-only")
        );
    }
}
