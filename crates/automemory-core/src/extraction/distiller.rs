use crate::db::Database;
use crate::models::*;
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistillationReport {
    pub session_id: String,
    pub project_id: Option<String>,
    pub total_raw_events: usize,
    pub pruned_events: usize,
    pub synthesized_trace: Option<SuccessTrace>,
    pub synthesized_memory: Option<Memory>,
    pub resolved_failures: Vec<FailureRecord>,
}

pub struct TrajectoryDistiller<'a> {
    db: &'a Database,
}

impl<'a> TrajectoryDistiller<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Evaluates a session's entire chronological event ledger (e.g. 50-100 tool calls)
    /// and distills it into 1 clean SuccessTrace, 1 high-signal Memory, and any resolved Failures,
    /// while strictly preserving evidence anchors (last key read before edits and error outputs).
    pub fn distill_session(&self, session_id: &str) -> Result<DistillationReport> {
        let events = self.db.get_session_events(session_id)?;
        self.distill_events(&events, Some(session_id))
    }

    /// Distills a slice of events into a coherent SuccessTrace and Memory.
    pub fn distill_events(
        &self,
        events: &[NormalizedEvent],
        default_session_id: Option<&str>,
    ) -> Result<DistillationReport> {
        let total_raw_events = events.len();
        let session_id = events
            .first()
            .map(|e| e.session_id.as_str())
            .or(default_session_id)
            .unwrap_or("unknown_session");

        if events.is_empty() {
            return Ok(DistillationReport {
                session_id: session_id.to_string(),
                project_id: None,
                total_raw_events: 0,
                pruned_events: 0,
                synthesized_trace: None,
                synthesized_memory: None,
                resolved_failures: Vec::new(),
            });
        }

        let project_id = events.first().and_then(|e| e.project.id.clone());
        let adapter_id = events
            .first()
            .map(|e| e.agent.adapter_id.clone())
            .unwrap_or_else(|| "agent".to_string());

        // 1. Extract Task Goal / Intent
        let mut task_title = String::new();
        for ev in events {
            if let Some(ref t) = ev.task_id {
                if !t.is_empty() {
                    task_title = t.clone();
                    break;
                }
            }
            if let Some(ref tool) = ev.tool {
                if let Some(ref input) = tool.input {
                    if let Some(goal) = input
                        .get("Task")
                        .or_else(|| input.get("task"))
                        .or_else(|| input.get("Instruction"))
                        .and_then(|v| v.as_str())
                    {
                        task_title = goal.to_string();
                        break;
                    }
                }
            }
        }
        if task_title.is_empty() {
            task_title = format!("{} session execution trajectory", adapter_id);
        }

        // 2. Identify Evidence Anchors (Last key read before an edit/mutation)
        let mut evidence_anchor_indices = HashSet::new();
        let mut last_read_idx: Option<usize> = None;

        for (idx, ev) in events.iter().enumerate() {
            if let Some(ref tool) = ev.tool {
                let is_read = tool.category == "file_read"
                    || tool.name.contains("view")
                    || tool.name.contains("read");
                let is_edit = tool.category == "file_write"
                    || tool.category == "edit"
                    || tool.name.contains("write")
                    || tool.name.contains("replace");

                if is_read {
                    last_read_idx = Some(idx);
                } else if is_edit {
                    if let Some(read_idx) = last_read_idx.take() {
                        evidence_anchor_indices.insert(read_idx);
                    }
                }
            }
        }

        // 3. Classify and extract key steps, pruning redundant noise while keeping evidence anchors
        let mut key_steps: Vec<TraceStep> = Vec::new();
        let mut files_modified = HashSet::new();
        let mut files_inspected = HashSet::new();
        let mut failed_tools = Vec::new();
        let mut resolved_failures = Vec::new();
        let mut evidence_ids = Vec::new();

        let mut last_error: Option<(String, String)> = None;

        for (idx, ev) in events.iter().enumerate() {
            evidence_ids.push(ev.event_id.clone());

            if let Some(ref tool) = ev.tool {
                let is_failed = tool.status == "failed"
                    || tool.exit_code.unwrap_or(0) != 0
                    || ev.event_type == "tool.failed";

                if is_failed {
                    let err_msg = tool
                        .output
                        .as_ref()
                        .and_then(|o| o.as_str())
                        .unwrap_or("Non-zero exit or tool failure")
                        .to_string();
                    last_error = Some((tool.name.clone(), err_msg));
                    failed_tools.push(tool.name.clone());
                } else {
                    // Check if this tool call resolved a previous error
                    if let Some((err_tool, err_msg)) = last_error.take() {
                        let failure_rec = FailureRecord {
                            id: Uuid::now_v7().to_string(),
                            project_id: project_id.clone(),
                            task_title: format!(
                                "Error in `{}` resolved by `{}`",
                                err_tool, tool.name
                            ),
                            category: Some(tool.category.clone()),
                            failed_tool: Some(err_tool),
                            error_message: err_msg,
                            root_cause_hypothesis: Some(
                                "Encountered compilation or test failure during workflow"
                                    .to_string(),
                            ),
                            fix_applied: Some(format!("Applied fix via `{}`", tool.name)),
                            verification_result: Some("Verification step succeeded".to_string()),
                            status: "resolved".to_string(),
                            evidence_event_ids: vec![ev.event_id.clone()],
                            created_at: Utc::now(),
                            updated_at: Utc::now(),
                        };
                        let _ = self.db.save_failure(&failure_rec);
                        resolved_failures.push(failure_rec);
                    }

                    // Extract modified and inspected files
                    if tool.category == "file_write"
                        || tool.name.contains("write")
                        || tool.name.contains("replace")
                    {
                        if let Some(ref input) = tool.input {
                            if let Some(f) = input
                                .get("TargetFile")
                                .or_else(|| input.get("AbsolutePath"))
                                .and_then(|v| v.as_str())
                            {
                                files_modified.insert(f.to_string());
                            }
                        }
                    } else if tool.category == "file_read"
                        || tool.name.contains("view")
                        || tool.name.contains("grep")
                    {
                        if let Some(ref input) = tool.input {
                            if let Some(f) = input
                                .get("AbsolutePath")
                                .or_else(|| input.get("SearchPath"))
                                .and_then(|v| v.as_str())
                            {
                                files_inspected.insert(f.to_string());
                            }
                        }
                    }

                    // Evidence Anchor / Modification / Verification Step Ingestion
                    let is_evidence_anchor = evidence_anchor_indices.contains(&idx);
                    let is_modification = tool.category == "file_write"
                        || tool.category == "edit"
                        || tool.name.contains("write")
                        || tool.name.contains("replace");
                    let is_verification = tool.category == "command"
                        || tool.name.contains("command")
                        || tool.name.contains("test");

                    let cmd_opt = tool.input.as_ref().and_then(|i| {
                        i.get("CommandLine")
                            .or_else(|| i.get("command"))
                            .and_then(|v| v.as_str())
                    });

                    let is_redundant_cmd = if let Some(cmd) = cmd_opt {
                        let base_cmd = cmd.split_whitespace().next().unwrap_or(cmd);
                        if (base_cmd == "git" && cmd.contains("status"))
                            || (base_cmd == "ls")
                            || (base_cmd == "pwd")
                        {
                            true
                        } else {
                            key_steps.iter().any(|s| s.command.as_deref() == Some(cmd))
                        }
                    } else {
                        false
                    };

                    let should_include = (is_evidence_anchor
                        || is_modification
                        || is_verification
                        || key_steps.is_empty())
                        && !is_redundant_cmd
                        && key_steps.len() < 10;

                    if should_include {
                        let action_desc = if is_evidence_anchor {
                            let path = tool
                                .input
                                .as_ref()
                                .and_then(|i| i.get("AbsolutePath").or_else(|| i.get("TargetFile")))
                                .and_then(|v| v.as_str())
                                .unwrap_or("file");
                            format!("Inspected key evidence in `{}`", path)
                        } else if is_modification {
                            format!("Modified codebase files ({})", files_modified.len().max(1))
                        } else if is_verification {
                            let cmd = cmd_opt.unwrap_or(&tool.name);
                            format!("Executed verification: {}", cmd)
                        } else {
                            format!("Investigated context via `{}`", tool.name)
                        };

                        key_steps.push(TraceStep {
                            position: (key_steps.len() + 1) as i32,
                            action: action_desc,
                            tool: tool.name.clone(),
                            command: cmd_opt.map(|s| s.to_string()),
                            verification: if is_verification {
                                Some("Completed without error".to_string())
                            } else {
                                None
                            },
                        });
                    }
                }
            }
        }

        let pruned_events = total_raw_events.saturating_sub(key_steps.len());

        // 4. Synthesize 1 Clean SuccessTrace
        let trace_id = Uuid::now_v7().to_string();
        let reproducibility_score = if failed_tools.is_empty() { 0.98 } else { 0.92 };
        let category = if !failed_tools.is_empty() {
            "bugfix".to_string()
        } else if !files_modified.is_empty() {
            "feature".to_string()
        } else {
            "investigation".to_string()
        };

        let synthesized_trace = SuccessTrace {
            id: trace_id.clone(),
            project_id: project_id.clone(),
            title: task_title.clone(),
            category,
            status: "active".to_string(),
            preconditions: vec!["Workspace repository initialized".to_string()],
            steps: key_steps.clone(),
            reproducibility_score,
            caveats: if failed_tools.is_empty() {
                Vec::new()
            } else {
                vec![format!(
                    "Encountered and resolved errors in: {}",
                    failed_tools.join(", ")
                )]
            },
            source_task_id: Some(session_id.to_string()),
            evidence_event_ids: evidence_ids.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let _ = self.db.save_trace(&synthesized_trace);

        // 5. Synthesize 1 High-Signal Crystallized Memory
        let mem_type = if !resolved_failures.is_empty() {
            MemoryType::Lesson
        } else if !files_modified.is_empty() {
            MemoryType::Decision
        } else {
            MemoryType::Pattern
        };

        let modified_files_str = if files_modified.is_empty() {
            "workspace context".to_string()
        } else {
            files_modified
                .into_iter()
                .take(5)
                .collect::<Vec<_>>()
                .join(", ")
        };

        let summary = format!("Execution outcome for: {}", task_title);

        let mut retrospective_lines = Vec::new();
        retrospective_lines.push(format!("### 🎯 Task: {}", task_title));
        retrospective_lines.push(format!(
            "- **Session**: `{}` | **Agent**: `{}`",
            session_id, adapter_id
        ));
        retrospective_lines.push(format!(
            "- **Trajectory Quality**: Distilled {} raw events into {} golden verified steps (reproducibility: {:.2}).",
            total_raw_events, key_steps.len(), reproducibility_score
        ));

        retrospective_lines.push(format!(
            "\n#### 🛠️ What Was Done\n- Modified files: {}.",
            modified_files_str
        ));
        if !key_steps.is_empty() {
            let key_actions = key_steps
                .iter()
                .take(4)
                .map(|s| s.action.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            retrospective_lines.push(format!("- Key operations: {}.", key_actions));
        }

        retrospective_lines.push("\n#### 🏆 What Was Achieved".to_string());
        if failed_tools.is_empty() {
            retrospective_lines.push(format!(
                "- Successfully completed all {} procedural steps with zero unhandled errors.",
                key_steps.len()
            ));
        } else {
            retrospective_lines.push(format!(
                "- Successfully achieved task outcome after resolving errors in: {}.",
                failed_tools.join(", ")
            ));
        }

        if !resolved_failures.is_empty() {
            retrospective_lines.push("\n#### ⚠️ What Went Wrong & How It Was Resolved".to_string());
            for f in &resolved_failures {
                retrospective_lines.push(format!("- **Issue**: {}", f.task_title));
                retrospective_lines.push(format!(
                    "  • Error: {}",
                    f.error_message.lines().next().unwrap_or("error")
                ));
                if let Some(ref fix) = f.fix_applied {
                    retrospective_lines.push(format!("  • Fix: {}", fix));
                }
            }
        }

        let content = retrospective_lines.join("\n");

        let synthesized_memory = Memory {
            id: Uuid::now_v7().to_string(),
            scope: if project_id.is_some() {
                MemoryScope::Project
            } else {
                MemoryScope::Global
            },
            project_id: project_id.clone(),
            memory_type: mem_type,
            content,
            summary,
            confidence: reproducibility_score,
            importance: 0.90,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::TaskReflection,
            version: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_recalled_at: None,
            tags: vec!["distilled-session".to_string(), adapter_id],
            entities: vec![task_title],
            metadata: serde_json::json!({
                "session_id": session_id,
                "total_events": total_raw_events,
                "trace_id": trace_id,
                "sync_origin": "local",
                "sync_state": "local_only"
            }),
        };
        let _ = self.db.save_memory(&synthesized_memory);

        Ok(DistillationReport {
            session_id: session_id.to_string(),
            project_id,
            total_raw_events,
            pruned_events,
            synthesized_trace: Some(synthesized_trace),
            synthesized_memory: Some(synthesized_memory),
            resolved_failures,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trajectory_distillation_evaluation() {
        let db = Database::open_in_memory().expect("in-memory db open failed");
        let distiller = TrajectoryDistiller::new(&db);

        let session_id = "test-session-100";
        for i in 1..=15 {
            let is_error = i == 5;
            let status = if is_error { "failed" } else { "success" };
            let tool_name = if i <= 4 {
                "view_file"
            } else if is_error {
                "run_command"
            } else if i == 6 {
                "replace_file_content"
            } else {
                "run_command"
            };

            let ev = NormalizedEvent {
                event_id: format!("ev-{}", i),
                event_type: if is_error {
                    "tool.failed".to_string()
                } else {
                    "tool.completed".to_string()
                },
                timestamp: Utc::now(),
                agent: AgentInfo {
                    adapter_id: "antigravity".to_string(),
                    agent_instance_id: None,
                },
                session_id: session_id.to_string(),
                task_id: Some("Fix WebSocket buffer leak".to_string()),
                project: ProjectRef {
                    id: Some("proj-1".to_string()),
                    slug: Some("automemory".to_string()),
                    root_path: "/test".to_string(),
                    source_git_remote: None,
                },
                tool: Some(ToolInfo {
                    name: tool_name.to_string(),
                    category: if tool_name.contains("file") {
                        "file_read".to_string()
                    } else {
                        "command".to_string()
                    },
                    input: Some(
                        serde_json::json!({ "CommandLine": "cargo test", "AbsolutePath": "/src/lib.rs" }),
                    ),
                    input_hash: None,
                    output: Some(serde_json::json!(if is_error {
                        "assertion failed at line 42"
                    } else {
                        "test result: ok"
                    })),
                    output_hash: None,
                    status: status.to_string(),
                    exit_code: if is_error { Some(1) } else { Some(0) },
                    duration_ms: Some(150),
                }),
                artifacts: Vec::new(),
                sensitivity: "internal".to_string(),
                schema_version: 1,
                raw_payload: None,
            };
            db.append_event(&ev).expect("failed to record test event");
        }

        let report = distiller
            .distill_session(session_id)
            .expect("distillation failed");
        assert_eq!(report.total_raw_events, 15);
        assert!(
            report.pruned_events > 0,
            "Should have pruned redundant events"
        );
        assert!(report.synthesized_trace.is_some());
        assert!(report.synthesized_memory.is_some());
        assert_eq!(
            report.resolved_failures.len(),
            1,
            "Should have resolved the single failed tool event"
        );

        let trace = report.synthesized_trace.unwrap();
        assert_eq!(trace.category, "bugfix");
        assert_eq!(trace.title, "Fix WebSocket buffer leak");
    }
}
