pub mod executor;
pub mod parser;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AutomationService {
    #[serde(rename = "mail")]
    Mail,
    #[serde(rename = "calendar")]
    Calendar,
    #[serde(rename = "notes")]
    Notes,
    #[serde(rename = "reminders")]
    Reminders,
}

impl AutomationService {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Mail => "mail",
            Self::Calendar => "calendar",
            Self::Notes => "notes",
            Self::Reminders => "reminders",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationStep {
    pub id: String,
    pub service: AutomationService,
    pub action: String,
    pub parameters: serde_json::Value,
    #[serde(
        default,
        rename = "taskStatus",
        skip_serializing_if = "Option::is_none"
    )]
    pub task_status: Option<String>,
    #[serde(
        default,
        rename = "requiresConfirmation",
        skip_serializing_if = "Option::is_none"
    )]
    pub requires_confirmation: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerResponse {
    pub status: String, // "continue" | "done"
    pub steps: Vec<AutomationStep>,
    #[serde(
        default,
        rename = "finalMessage",
        skip_serializing_if = "Option::is_none"
    )]
    pub final_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepObservation {
    #[serde(rename = "stepID")]
    pub step_id: String,
    pub service: AutomationService,
    pub action: String,
    pub output: serde_json::Value,
    #[serde(
        default,
        rename = "taskStatus",
        skip_serializing_if = "Option::is_none"
    )]
    pub task_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReport {
    pub observations: Vec<StepObservation>,
    #[serde(
        default,
        rename = "finalMessage",
        skip_serializing_if = "Option::is_none"
    )]
    pub final_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AutomationHandleResult {
    NotAutomation,
    Success(ExecutionReport),
    NeedsClarification(AutomationClarification),
    Failure(AutomationErrorResult),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationClarification {
    pub message: String,
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationErrorResult {
    pub ok: bool,
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ActionDefinition {
    pub service: AutomationService,
    pub action: String,
    pub required: Vec<String>,
    pub optional: Vec<String>,
    pub risk: ActionRisk,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActionRisk {
    ReadOnly,
    ReversibleWrite,
    ExternalWrite,
    Destructive,
}

pub fn action_registry() -> Vec<ActionDefinition> {
    vec![
        ActionDefinition {
            service: AutomationService::Mail,
            action: "find".into(),
            required: vec!["query".into()],
            optional: vec!["limit".into()],
            risk: ActionRisk::ReadOnly,
        },
        ActionDefinition {
            service: AutomationService::Mail,
            action: "read".into(),
            required: vec!["localId".into()],
            optional: vec![],
            risk: ActionRisk::ReadOnly,
        },
        ActionDefinition {
            service: AutomationService::Mail,
            action: "draft_reply".into(),
            required: vec!["localId".into(), "body".into()],
            optional: vec![],
            risk: ActionRisk::ReversibleWrite,
        },
        ActionDefinition {
            service: AutomationService::Mail,
            action: "send_reply".into(),
            required: vec!["localId".into(), "body".into()],
            optional: vec![],
            risk: ActionRisk::ExternalWrite,
        },
        ActionDefinition {
            service: AutomationService::Mail,
            action: "move".into(),
            required: vec!["localId".into(), "mailbox".into()],
            optional: vec![],
            risk: ActionRisk::ReversibleWrite,
        },
        ActionDefinition {
            service: AutomationService::Calendar,
            action: "find".into(),
            required: vec!["query".into()],
            optional: vec!["limit".into(), "from".into(), "to".into()],
            risk: ActionRisk::ReadOnly,
        },
        ActionDefinition {
            service: AutomationService::Calendar,
            action: "create".into(),
            required: vec!["title".into(), "startAt".into()],
            optional: vec![
                "endAt".into(),
                "durationMinutes".into(),
                "calendar".into(),
                "notes".into(),
                "location".into(),
            ],
            risk: ActionRisk::ReversibleWrite,
        },
        ActionDefinition {
            service: AutomationService::Calendar,
            action: "update".into(),
            required: vec!["eventId".into()],
            optional: vec![
                "title".into(),
                "startAt".into(),
                "endAt".into(),
                "notes".into(),
                "location".into(),
            ],
            risk: ActionRisk::ReversibleWrite,
        },
        ActionDefinition {
            service: AutomationService::Calendar,
            action: "delete".into(),
            required: vec!["eventId".into()],
            optional: vec![],
            risk: ActionRisk::Destructive,
        },
        ActionDefinition {
            service: AutomationService::Notes,
            action: "find".into(),
            required: vec!["query".into()],
            optional: vec!["limit".into()],
            risk: ActionRisk::ReadOnly,
        },
        ActionDefinition {
            service: AutomationService::Notes,
            action: "create".into(),
            required: vec!["title".into(), "body".into()],
            optional: vec![],
            risk: ActionRisk::ReversibleWrite,
        },
        ActionDefinition {
            service: AutomationService::Notes,
            action: "update".into(),
            required: vec!["noteId".into()],
            optional: vec!["title".into(), "body".into()],
            risk: ActionRisk::ReversibleWrite,
        },
        ActionDefinition {
            service: AutomationService::Reminders,
            action: "find".into(),
            required: vec!["query".into()],
            optional: vec!["limit".into()],
            risk: ActionRisk::ReadOnly,
        },
        ActionDefinition {
            service: AutomationService::Reminders,
            action: "create".into(),
            required: vec!["title".into()],
            optional: vec![
                "list".into(),
                "notes".into(),
                "dueAt".into(),
                "priority".into(),
            ],
            risk: ActionRisk::ReversibleWrite,
        },
        ActionDefinition {
            service: AutomationService::Reminders,
            action: "create_list".into(),
            required: vec!["name".into()],
            optional: vec![],
            risk: ActionRisk::ReversibleWrite,
        },
        ActionDefinition {
            service: AutomationService::Reminders,
            action: "complete".into(),
            required: vec!["reminderId".into()],
            optional: vec![],
            risk: ActionRisk::ReversibleWrite,
        },
    ]
}

pub fn find_definition(service: &AutomationService, action: &str) -> Option<ActionDefinition> {
    action_registry()
        .into_iter()
        .find(|d| &d.service == service && d.action == action)
}

pub fn validate_planner_response(resp: &PlannerResponse) -> Result<(), AutomationErrorResult> {
    if resp.status != "continue" && resp.status != "done" {
        return Err(AutomationErrorResult {
            ok: false,
            error: "validation_error".into(),
            message: "status must be continue or done".into(),
            details: Some(resp.status.clone()),
        });
    }
    if resp.steps.is_empty() && resp.status == "continue" {
        return Err(AutomationErrorResult {
            ok: false,
            error: "validation_error".into(),
            message: "continue with no steps".into(),
            details: None,
        });
    }
    let mut seen = std::collections::HashSet::new();
    for step in &resp.steps {
        if !seen.insert(step.id.clone()) {
            return Err(AutomationErrorResult {
                ok: false,
                error: "validation_error".into(),
                message: format!("duplicate step id {}", step.id),
                details: None,
            });
        }
        let def =
            find_definition(&step.service, &step.action).ok_or_else(|| AutomationErrorResult {
                ok: false,
                error: "validation_error".into(),
                message: format!(
                    "unsupported action {}.{}",
                    step.service.as_str(),
                    step.action
                ),
                details: None,
            })?;
        // required params
        let params = step.parameters.as_object().cloned().unwrap_or_default();
        for req in &def.required {
            if !params.contains_key(req) {
                return Err(AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: format!(
                        "missing parameter {} for {}.{}",
                        req,
                        step.service.as_str(),
                        step.action
                    ),
                    details: None,
                });
            }
        }
        let allowed: std::collections::HashSet<&String> =
            def.required.iter().chain(def.optional.iter()).collect();
        for key in params.keys() {
            if !allowed.contains(key) {
                return Err(AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: format!(
                        "unknown param {} for {}.{}",
                        key,
                        step.service.as_str(),
                        step.action
                    ),
                    details: None,
                });
            }
        }
        // safety: externalWrite must have requiresConfirmation true if present or else we will enforce later
        if def.risk == ActionRisk::ExternalWrite || def.risk == ActionRisk::Destructive {
            if step.requires_confirmation == Some(false) {
                return Err(AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: format!(
                        "{}.{} requires requiresConfirmation true",
                        step.service.as_str(),
                        step.action
                    ),
                    details: None,
                });
            }
        }
        // taskStatus validation — per-step sharp audit: 10-15 words, DJ-edit clean, not generic.
        // This is the hook you asked for: each step's status is reported alongside its action.
        if let Some(ts) = &step.task_status {
            let wc = ts.split_whitespace().count();
            if wc < 10 || wc > 15 {
                return Err(AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: format!(
                        "taskStatus for {}.{} must be 10-15 words (got {wc}): '{}'",
                        step.service.as_str(),
                        step.action,
                        ts
                    ),
                    details: None,
                });
            }
            if ts.len() < 40 {
                return Err(AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: format!(
                        "taskStatus too short (<40 chars) for {}.{}: '{}'",
                        step.service.as_str(),
                        step.action,
                        ts
                    ),
                    details: None,
                });
            }
            let lower = ts.to_lowercase();
            let generic = [
                "task completed",
                "done",
                "success",
                "completed successfully",
                "action completed",
                "operation successful",
            ];
            // Reject generic 1-2 word exact matches or contains generic phrases with very short extra
            if lower.trim() == "done"
                || lower.trim() == "task completed"
                || lower.trim() == "task completed."
                || lower.trim() == "done."
            {
                return Err(AutomationErrorResult {
                    ok: false,
                    error: "validation_error".into(),
                    message: format!(
                        "taskStatus must not be generic for {}.{}: '{}'",
                        step.service.as_str(),
                        step.action,
                        ts
                    ),
                    details: None,
                });
            }
            for g in &generic {
                if lower == *g || (lower.contains(g) && wc <= 5) {
                    return Err(AutomationErrorResult {
                        ok: false,
                        error: "validation_error".into(),
                        message: format!(
                            "taskStatus must not be generic '{}' for {}.{}: '{}'",
                            g,
                            step.service.as_str(),
                            step.action,
                            ts
                        ),
                        details: None,
                    });
                }
            }
        } else {
            // Enforce presence: taskStatus REQUIRED on every step per ultra-strict contract (hook up and edit).
            // For backward compat with old fixtures without taskStatus, we could allow but log; but new prompt requires it.
            return Err(AutomationErrorResult { ok: false, error: "validation_error".into(), message: format!("taskStatus is required on every step (10-15 words sharp) — missing for {}.{} id={}", step.service.as_str(), step.action, step.id), details: None });
        }
        // template syntax quick check: steps references must look like {{steps.id.path}}
        // we do not validate resolution now — done at execution.
    }
    Ok(())
}

// Observation reducer — cap large strings/arrays before feeding back to Gemini (bounded token/cost)
pub fn reduce_observations(obs: &[StepObservation]) -> Vec<StepObservation> {
    const MAX_STR: usize = 12_000;
    const MAX_ARR: usize = 20;
    fn reduce_value(v: serde_json::Value) -> serde_json::Value {
        match v {
            serde_json::Value::String(s) => {
                if s.len() <= MAX_STR {
                    serde_json::Value::String(s)
                } else {
                    serde_json::Value::String(s.chars().take(MAX_STR).collect())
                }
            }
            serde_json::Value::Array(arr) => {
                let truncated: Vec<_> = arr.into_iter().take(MAX_ARR).map(reduce_value).collect();
                serde_json::Value::Array(truncated)
            }
            serde_json::Value::Object(map) => {
                let m = map.into_iter().map(|(k, v)| (k, reduce_value(v))).collect();
                serde_json::Value::Object(m)
            }
            other => other,
        }
    }
    obs.iter()
        .map(|o| StepObservation {
            step_id: o.step_id.clone(),
            service: o.service.clone(),
            action: o.action.clone(),
            output: reduce_value(o.output.clone()),
            task_status: o.task_status.clone(),
        })
        .collect()
}

pub async fn handle_automation_transcript(
    transcript: &str,
    settings: &crate::settings::AppSettings,
    app: Option<&tauri::AppHandle>,
) -> AutomationHandleResult {
    // Quick heuristic to avoid LLM for obvious non-automation
    let lower = transcript.to_lowercase();
    let has_automation_signal = lower.contains("email")
        || lower.contains("mail")
        || lower.contains("inbox")
        || lower.contains("reply")
        || lower.contains("send")
        || lower.contains("schedule")
        || lower.contains("calendar")
        || lower.contains("event")
        || lower.contains("meeting")
        || lower.contains("reminder")
        || lower.contains("remind")
        || lower.contains("task")
        || lower.contains("todo")
        || lower.contains("notes")
        || lower.contains("note")
        || lower.contains("create")
        || lower.contains("find")
        || lower.contains("follow up")
        || lower.contains("follow-up");
    let has_action_signal = lower.contains("find")
        || lower.contains("create")
        || lower.contains("schedule")
        || lower.contains("draft")
        || lower.contains("reply")
        || lower.contains("remind");
    if !has_automation_signal && !has_action_signal {
        if lower.split_whitespace().count() < 4 {
            return AutomationHandleResult::NotAutomation;
        }
        // let AI still decide but avoid quota for "hello world"
        if !lower.contains("tomorrow")
            && !lower.contains("today")
            && !lower.contains("monday")
            && !lower.contains("tuesday")
            && !lower.contains("wednesday")
            && !lower.contains("thursday")
            && !lower.contains("friday")
        {
            // no time signal either — likely not automation
            // but we will still try if it mentions mail/calendar/notes/reminders word
        }
    }

    // Bounded agent loop — max 4 rounds (Gemini CLF + deterministic Swift)
    let mut observations: Vec<StepObservation> = Vec::new();
    let max_rounds = 4;

    for round in 1..=max_rounds {
        let reduced = reduce_observations(&observations);
        let ai_output =
            match parser::parse_automation_intent(transcript, &reduced, round, settings).await {
                Ok(Some(output)) => output,
                Ok(None) => {
                    if round == 1 {
                        return AutomationHandleResult::NotAutomation;
                    } else {
                        return AutomationHandleResult::Failure(AutomationErrorResult {
                            ok: false,
                            error: "planner_failure".into(),
                            message: "Planner returned none mid-loop".into(),
                            details: None,
                        });
                    }
                }
                Err(e) => return AutomationHandleResult::Failure(e),
            };

        // PlannerResponse already validated inside parser; re-validate safety
        if let Err(e) = validate_planner_response(&ai_output) {
            return AutomationHandleResult::Failure(e);
        }

        if ai_output.steps.is_empty() && ai_output.status == "continue" {
            return AutomationHandleResult::Failure(AutomationErrorResult {
                ok: false,
                error: "validation_error".into(),
                message: "Planner returned continue with no steps".into(),
                details: None,
            });
        }

        // Execute this round's steps deterministically
        let exec_result =
            executor::execute_steps(&ai_output.steps, &observations, app.cloned()).await;
        match exec_result {
            Ok(new_obs) => {
                // Live voice feed: surface each finished step's status before
                // the next round plans. Terminal speech rides automation-success.
                if let Some(handle) = app {
                    for obs in &new_obs {
                        if let Some(task_status) = obs.task_status.as_deref() {
                            if !task_status.trim().is_empty() {
                                crate::overlay::show_automation_step_overlay(
                                    handle,
                                    &obs.step_id,
                                    task_status,
                                );
                            }
                        }
                    }
                }
                observations.extend(new_obs);
                if ai_output.status == "done" {
                    return AutomationHandleResult::Success(ExecutionReport {
                        observations,
                        final_message: ai_output.final_message,
                    });
                }
                // else continue loop — next Gemini call will see observations
            }
            Err(e) => {
                // Execution failed — report as failure (e.g., confirmation denied, permission, unresolved reference)
                return AutomationHandleResult::Failure(e);
            }
        }
    }

    AutomationHandleResult::Failure(AutomationErrorResult {
        ok: false,
        error: "exhausted_rounds".into(),
        message: format!("Planner exceeded maximum rounds: {max_rounds}"),
        details: None,
    })
}
