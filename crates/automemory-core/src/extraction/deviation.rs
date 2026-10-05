use crate::models::{NormalizedEvent, SuccessTrace};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviationReport {
    pub has_significant_deviation: bool,
    pub deviation_score: f64,
    pub missing_safety_steps: Vec<String>,
    pub warning_message: Option<String>,
}

pub struct WorkflowDeviationDetector;

impl WorkflowDeviationDetector {
    /// Compares the currently executed events against a golden SuccessTrace to identify skipped safety steps.
    pub fn evaluate(
        current_events: &[NormalizedEvent],
        golden_trace: &SuccessTrace,
    ) -> DeviationReport {
        if golden_trace.steps.is_empty() {
            return DeviationReport {
                has_significant_deviation: false,
                deviation_score: 0.0,
                missing_safety_steps: Vec::new(),
                warning_message: None,
            };
        }

        // Collect all executed tool names and commands
        let mut executed_tools = HashSet::new();
        let mut executed_commands = HashSet::new();

        for ev in current_events {
            if let Some(ref tool) = ev.tool {
                executed_tools.insert(tool.name.to_lowercase());
                if let Some(ref input) = tool.input {
                    if let Some(cmd) = input
                        .get("CommandLine")
                        .or_else(|| input.get("command"))
                        .and_then(|v| v.as_str())
                    {
                        executed_commands.insert(cmd.to_lowercase());
                    }
                }
            }
        }

        let mut missing_safety_steps = Vec::new();

        for step in &golden_trace.steps {
            let tool_lower = step.tool.to_lowercase();
            let is_safety_step = tool_lower.contains("test")
                || tool_lower.contains("check")
                || step.action.to_lowercase().contains("verification")
                || step.action.to_lowercase().contains("backup")
                || step.action.to_lowercase().contains("health");

            if is_safety_step {
                let tool_found = executed_tools.contains(&tool_lower);
                let cmd_found = step
                    .command
                    .as_ref()
                    .map(|c| executed_commands.contains(&c.to_lowercase()))
                    .unwrap_or(false);

                if !tool_found && !cmd_found {
                    missing_safety_steps.push(format!(
                        "Missing safety step: {} (`{}`)",
                        step.action, step.tool
                    ));
                }
            }
        }

        let total_steps = golden_trace.steps.len() as f64;
        let missing_count = missing_safety_steps.len() as f64;
        let deviation_score = (missing_count / total_steps.max(1.0)).clamp(0.0, 1.0);

        let has_significant_deviation = !missing_safety_steps.is_empty();
        let warning_message = if has_significant_deviation {
            Some(format!(
                "Workflow deviation detected: {} safety verification step(s) from golden trace '{}' have not been executed.",
                missing_safety_steps.len(),
                golden_trace.title
            ))
        } else {
            None
        };

        DeviationReport {
            has_significant_deviation,
            deviation_score,
            missing_safety_steps,
            warning_message,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AgentInfo, ProjectRef, ToolInfo, TraceStep};
    use chrono::Utc;

    #[test]
    fn test_deviation_detection_on_skipped_tests() {
        let golden_trace = SuccessTrace {
            id: "t1".to_string(),
            project_id: Some("p1".to_string()),
            title: "Safe Deploy Pipeline".to_string(),
            category: "deploy".to_string(),
            status: "active".to_string(),
            preconditions: vec![],
            steps: vec![
                TraceStep {
                    position: 1,
                    action: "Executed verification: cargo test".to_string(),
                    tool: "run_command".to_string(),
                    command: Some("cargo test".to_string()),
                    verification: Some("Passed".to_string()),
                },
                TraceStep {
                    position: 2,
                    action: "Deployed artifacts".to_string(),
                    tool: "deploy".to_string(),
                    command: Some("deploy prod".to_string()),
                    verification: None,
                },
            ],
            reproducibility_score: 1.0,
            caveats: vec![],
            source_task_id: None,
            evidence_event_ids: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Current session only ran deploy, skipped cargo test
        let current_events = vec![NormalizedEvent {
            event_id: "e1".to_string(),
            event_type: "tool.completed".to_string(),
            timestamp: Utc::now(),
            agent: AgentInfo {
                adapter_id: "agy".to_string(),
                agent_instance_id: None,
            },
            session_id: "s1".to_string(),
            task_id: None,
            project: ProjectRef {
                id: Some("p1".to_string()),
                slug: None,
                root_path: "".to_string(),
                source_git_remote: None,
            },
            tool: Some(ToolInfo {
                name: "deploy".to_string(),
                category: "command".to_string(),
                input: Some(serde_json::json!({ "command": "deploy prod" })),
                input_hash: None,
                output: None,
                output_hash: None,
                status: "success".to_string(),
                exit_code: Some(0),
                duration_ms: None,
            }),
            artifacts: vec![],
            sensitivity: "public".to_string(),
            schema_version: 1,
            raw_payload: None,
        }];

        let report = WorkflowDeviationDetector::evaluate(&current_events, &golden_trace);
        assert!(report.has_significant_deviation);
        assert_eq!(report.missing_safety_steps.len(), 1);
        assert!(report.missing_safety_steps[0].contains("verification"));
    }
}
