use crate::db::Database;
use crate::models::*;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::json;
use std::collections::{HashSet, VecDeque};
use uuid::Uuid;

pub struct TimeSpanDigestEngine<'a> {
    db: &'a Database,
}

impl<'a> TimeSpanDigestEngine<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Evaluates events across an arbitrary time window for a project (or globally)
    /// and generates a structured TimeSpanDigest of edits, achievements, and failures.
    pub fn generate_digest(
        &self,
        project_id: Option<&str>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<TimeSpanDigest> {
        let events = self
            .db
            .get_events_between(project_id, start_time, end_time)?;
        self.digest_events(&events, project_id, start_time, end_time)
    }

    /// Evaluates all events within an active or completed session and generates a TimeSpanDigest.
    pub fn generate_session_digest(&self, session_id: &str) -> Result<TimeSpanDigest> {
        let events = self.db.get_session_events(session_id)?;
        if events.is_empty() {
            let mut digest = TimeSpanDigest::empty(None);
            digest.executive_summary = "No activity recorded for session.".to_string();
            return Ok(digest);
        }

        let start_time = events.first().map(|e| e.timestamp).unwrap_or_else(Utc::now);
        let end_time = events.last().map(|e| e.timestamp).unwrap_or_else(Utc::now);
        let project_id = events.first().and_then(|e| e.project.id.clone());

        self.digest_events(&events, project_id.as_deref(), start_time, end_time)
    }

    /// Processes a chronological slice of NormalizedEvents and synthesizes activity metrics,
    /// files modified/inspected, achievements reached, and failures resolved.
    pub fn digest_events(
        &self,
        events: &[NormalizedEvent],
        project_id: Option<&str>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<TimeSpanDigest> {
        let duration_minutes = ((end_time - start_time).num_seconds().max(0) as f64) / 60.0;

        let mut active_agents_set = HashSet::new();
        let mut files_modified_set = HashSet::new();
        let mut files_inspected_set = HashSet::new();
        let mut key_commands = Vec::new();

        let mut metrics = ActivityMetrics::default();
        let mut had_prior_test_failure = false;
        let mut test_failure_msg: Option<String> = None;

        let mut pending_failures: VecDeque<TimeSpanFailureSummary> = VecDeque::new();
        let mut finalized_failures: Vec<TimeSpanFailureSummary> = Vec::new();
        let mut achievements: Vec<String> = Vec::new();

        for ev in events {
            active_agents_set.insert(ev.agent.adapter_id.clone());

            if let Some(ref tool) = ev.tool {
                metrics.total_tool_calls += 1;

                let is_edit = tool.category == "file_write"
                    || tool.category == "edit"
                    || tool.name.contains("write")
                    || tool.name.contains("replace")
                    || tool.name.contains("edit");

                let is_read = tool.category == "file_read"
                    || tool.name.contains("view")
                    || tool.name.contains("read")
                    || tool.name.contains("grep")
                    || tool.name.contains("list")
                    || tool.name.contains("search");

                let is_command = tool.category == "command"
                    || tool.name.contains("command")
                    || tool.name.contains("bash")
                    || tool.name.contains("terminal")
                    || tool.name.contains("exec");

                if is_edit {
                    metrics.edits_count += 1;
                    if let Some(ref input) = tool.input {
                        if let Some(f) = input
                            .get("TargetFile")
                            .or_else(|| input.get("AbsolutePath"))
                            .or_else(|| input.get("path"))
                            .or_else(|| input.get("file"))
                            .and_then(|v| v.as_str())
                        {
                            files_modified_set.insert(f.to_string());
                        }
                    }
                } else if is_read {
                    metrics.reads_count += 1;
                    if let Some(ref input) = tool.input {
                        if let Some(f) = input
                            .get("AbsolutePath")
                            .or_else(|| input.get("SearchPath"))
                            .or_else(|| input.get("path"))
                            .or_else(|| input.get("file"))
                            .and_then(|v| v.as_str())
                        {
                            files_inspected_set.insert(f.to_string());
                        }
                    }
                }

                let cmd_opt = tool.input.as_ref().and_then(|i| {
                    i.get("CommandLine")
                        .or_else(|| i.get("command"))
                        .and_then(|v| v.as_str())
                });

                if is_command {
                    metrics.commands_count += 1;
                    if let Some(cmd) = cmd_opt {
                        let trimmed = cmd.trim();
                        if !trimmed.is_empty()
                            && !key_commands.iter().any(|c: &String| c == trimmed)
                            && !trimmed.starts_with("pwd")
                            && !trimmed.starts_with("whoami")
                            && key_commands.len() < 10
                        {
                            key_commands.push(trimmed.to_string());
                        }
                    }
                }

                // Check for test execution
                let is_test_run = cmd_opt.is_some_and(|c| {
                    let lc = c.to_lowercase();
                    lc.contains("test")
                        || lc.contains("check")
                        || lc.contains("pytest")
                        || lc.contains("cargo test")
                        || lc.contains("npm test")
                        || lc.contains("pnpm test")
                        || lc.contains("yarn test")
                });

                let tool_failed = tool.status == "failed"
                    || tool.exit_code.unwrap_or(0) != 0
                    || ev.event_type.contains("fail");

                if is_test_run {
                    metrics.tests_run += 1;
                    if tool_failed {
                        metrics.tests_failed += 1;
                        had_prior_test_failure = true;
                        let err_snippet = tool
                            .output
                            .as_ref()
                            .and_then(|o| o.as_str())
                            .unwrap_or("Test command failed with non-zero exit code")
                            .lines()
                            .take(3)
                            .collect::<Vec<_>>()
                            .join(" ");
                        test_failure_msg = Some(err_snippet);
                    } else {
                        metrics.tests_passed += 1;
                        if had_prior_test_failure {
                            let resolved_note = if let Some(ref prev_err) = test_failure_msg {
                                format!(
                                    "Resolved test failure: verified passing test suite after '{}'",
                                    prev_err
                                )
                            } else {
                                "Resolved test failure: verified passing test suite".to_string()
                            };
                            achievements.push(resolved_note);
                            had_prior_test_failure = false;
                            test_failure_msg = None;
                        } else if !achievements.iter().any(|a| a.contains("Verified passing")) {
                            achievements.push(format!(
                                "Verified passing test suite via `{}`",
                                cmd_opt.unwrap_or(&tool.name)
                            ));
                        }
                    }
                }

                // Check for git commit achievements
                if let Some(cmd) = cmd_opt {
                    if cmd.contains("git commit") {
                        achievements.push(format!("Committed repository changes: {}", cmd.trim()));
                    }
                }

                // Failure Tracking & Resolution Pairing
                if tool_failed {
                    let err_output = tool
                        .output
                        .as_ref()
                        .and_then(|o| o.as_str())
                        .unwrap_or("Non-zero exit code or tool failure");
                    let snippet: String = err_output.chars().take(250).collect();

                    pending_failures.push_back(TimeSpanFailureSummary {
                        tool_or_command: cmd_opt
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| tool.name.clone()),
                        error_snippet: snippet,
                        is_resolved: false,
                        fix_applied: None,
                        event_id: ev.event_id.clone(),
                    });
                } else if !pending_failures.is_empty() {
                    // Subsequent successful edit or command resolves prior pending failure
                    if let Some(mut resolved) = pending_failures.pop_front() {
                        resolved.is_resolved = true;
                        resolved.fix_applied = Some(format!(
                            "Resolved by `{}`: {}",
                            tool.name,
                            cmd_opt.unwrap_or("step succeeded")
                        ));
                        finalized_failures.push(resolved);
                    }
                }
            }
        }

        // Move remaining unresolved failures to finalized list
        while let Some(unresolved) = pending_failures.pop_front() {
            finalized_failures.push(unresolved);
        }

        // Additional Achievements Synthesis
        if achievements.is_empty() {
            if !files_modified_set.is_empty() && metrics.tests_failed == 0 {
                achievements.push(format!(
                    "Completed {} modifications across {} codebase files",
                    metrics.edits_count,
                    files_modified_set.len()
                ));
            } else if metrics.total_tool_calls > 0 && metrics.tests_failed == 0 {
                achievements.push(format!(
                    "Executed {} tool operations cleanly without errors",
                    metrics.total_tool_calls
                ));
            }
        }

        let mut active_agents: Vec<String> = active_agents_set.into_iter().collect();
        active_agents.sort();

        let mut files_modified: Vec<String> = files_modified_set.into_iter().collect();
        files_modified.sort();

        let mut files_inspected: Vec<String> = files_inspected_set.into_iter().collect();
        files_inspected.sort();

        // Synthesize Executive Summary in high-signal Markdown
        let mut summary_lines = Vec::new();
        summary_lines.push(format!(
            "### ⏱️ Activity Window Digest ({:.1} min)",
            duration_minutes
        ));
        summary_lines.push(format!(
            "- **Active Agents**: {}",
            if active_agents.is_empty() {
                "none".to_string()
            } else {
                active_agents.join(", ")
            }
        ));
        summary_lines.push(format!(
            "- **Editing Velocity**: {} file edits across {} modified files ({} context reads, {} commands).",
            metrics.edits_count,
            files_modified.len(),
            metrics.reads_count,
            metrics.commands_count
        ));

        if metrics.tests_run > 0 {
            summary_lines.push(format!(
                "- **Test Executions**: {} passed, {} failed ({} total runs).",
                metrics.tests_passed, metrics.tests_failed, metrics.tests_run
            ));
        }

        if !achievements.is_empty() {
            summary_lines.push("- **🏆 Achievements Reached**:".to_string());
            for ach in &achievements {
                summary_lines.push(format!("  • {}", ach));
            }
        }

        if !finalized_failures.is_empty() {
            summary_lines.push("- **⚠️ Failures & Blockers**:".to_string());
            for f in &finalized_failures {
                if f.is_resolved {
                    summary_lines.push(format!(
                        "  • ✓ [Resolved] `{}`: {}",
                        f.tool_or_command,
                        f.fix_applied.as_deref().unwrap_or("fixed")
                    ));
                } else {
                    summary_lines.push(format!(
                        "  • ✗ [Open] `{}`: {}",
                        f.tool_or_command, f.error_snippet
                    ));
                }
            }
        }

        let executive_summary = summary_lines.join("\n");

        Ok(TimeSpanDigest {
            project_id: project_id.map(String::from),
            start_time,
            end_time,
            duration_minutes,
            active_agents,
            metrics,
            achievements,
            failures: finalized_failures,
            files_modified,
            files_inspected,
            executive_summary,
            key_commands_executed: key_commands,
        })
    }

    /// Synthesizes a durable milestone Memory struct from a TimeSpanDigest.
    /// The returned Memory can be directly saved using `db.save_memory(&mem)`.
    pub fn synthesize_milestone_memory(
        &self,
        digest: &TimeSpanDigest,
        custom_title: Option<&str>,
    ) -> Result<Memory> {
        let mem_id = Uuid::now_v7().to_string();
        let title = custom_title.unwrap_or("Time-Span Progress Checkpoint");

        let summary = format!(
            "{}: {} edits across {} files, {} achievements",
            title,
            digest.metrics.edits_count,
            digest.files_modified.len(),
            digest.achievements.len()
        );

        let resolved_count = digest.failures.iter().filter(|f| f.is_resolved).count();
        let open_count = digest.failures.iter().filter(|f| !f.is_resolved).count();

        let mem = Memory {
            id: mem_id.clone(),
            scope: if digest.project_id.is_some() {
                MemoryScope::Project
            } else {
                MemoryScope::Global
            },
            project_id: digest.project_id.clone(),
            memory_type: if open_count > 0 {
                MemoryType::Warning
            } else if resolved_count > 0 {
                MemoryType::Lesson
            } else {
                // A clean session summary is an observation; calling it a decision would fill the Decisions list.
                MemoryType::Observation
            },
            content: digest.executive_summary.clone(),
            summary,
            confidence: 0.95,
            importance: 0.85,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::TaskReflection,
            version: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_recalled_at: None,
            tags: vec![
                "checkpoint".to_string(),
                "time-span".to_string(),
                format!("agents-{}", digest.active_agents.join("-")),
            ],
            entities: digest.files_modified.clone(),
            metadata: json!({
                "time_span": {
                    "start": digest.start_time.to_rfc3339(),
                    "end": digest.end_time.to_rfc3339(),
                    "duration_minutes": digest.duration_minutes,
                },
                "metrics": digest.metrics,
                "achievements": digest.achievements,
                "failures_resolved_count": resolved_count,
                "failures_open_count": open_count,
                "files_modified": digest.files_modified,
                "files_inspected": digest.files_inspected,
            }),
        };

        Ok(mem)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_timespan_digest_activity_and_achievements() {
        let db = Database::open_in_memory().expect("open in-memory db");
        let engine = TimeSpanDigestEngine::new(&db);
        let now = Utc::now();
        let start = now - Duration::minutes(30);

        let mut events = Vec::new();

        // 1. Context view
        events.push(NormalizedEvent {
            event_id: Uuid::now_v7().to_string(),
            event_type: "agent.tool".to_string(),
            timestamp: start + Duration::minutes(2),
            agent: AgentInfo {
                adapter_id: "claude-code".to_string(),
                agent_instance_id: None,
            },
            session_id: "session-1".to_string(),
            task_id: Some("task-1".to_string()),
            project: ProjectRef {
                id: Some("proj-1".to_string()),
                slug: None,
                root_path: String::new(),
                source_git_remote: None,
            },
            tool: Some(ToolInfo {
                name: "view_file".to_string(),
                category: "file_read".to_string(),
                input: Some(json!({"AbsolutePath": "/src/main.rs"})),
                input_hash: None,
                output: Some(json!("fn main() {}")),
                output_hash: None,
                status: "success".to_string(),
                exit_code: Some(0),
                duration_ms: Some(10),
            }),
            artifacts: Vec::new(),
            sensitivity: "none".to_string(),
            schema_version: 1,
            raw_payload: None,
        });

        // 2. Edit file
        events.push(NormalizedEvent {
            event_id: Uuid::now_v7().to_string(),
            event_type: "agent.tool".to_string(),
            timestamp: start + Duration::minutes(5),
            agent: AgentInfo {
                adapter_id: "claude-code".to_string(),
                agent_instance_id: None,
            },
            session_id: "session-1".to_string(),
            task_id: Some("task-1".to_string()),
            project: ProjectRef {
                id: Some("proj-1".to_string()),
                slug: None,
                root_path: String::new(),
                source_git_remote: None,
            },
            tool: Some(ToolInfo {
                name: "write_to_file".to_string(),
                category: "file_write".to_string(),
                input: Some(json!({"TargetFile": "/src/main.rs"})),
                input_hash: None,
                output: Some(json!("written")),
                output_hash: None,
                status: "success".to_string(),
                exit_code: Some(0),
                duration_ms: Some(15),
            }),
            artifacts: Vec::new(),
            sensitivity: "none".to_string(),
            schema_version: 1,
            raw_payload: None,
        });

        // 3. Failing test
        events.push(NormalizedEvent {
            event_id: Uuid::now_v7().to_string(),
            event_type: "agent.tool".to_string(),
            timestamp: start + Duration::minutes(10),
            agent: AgentInfo {
                adapter_id: "cursor".to_string(),
                agent_instance_id: None,
            },
            session_id: "session-1".to_string(),
            task_id: Some("task-1".to_string()),
            project: ProjectRef {
                id: Some("proj-1".to_string()),
                slug: None,
                root_path: String::new(),
                source_git_remote: None,
            },
            tool: Some(ToolInfo {
                name: "run_command".to_string(),
                category: "command".to_string(),
                input: Some(json!({"CommandLine": "cargo test"})),
                input_hash: None,
                output: Some(json!("assertion failed: expected true, got false")),
                output_hash: None,
                status: "failed".to_string(),
                exit_code: Some(101),
                duration_ms: Some(250),
            }),
            artifacts: Vec::new(),
            sensitivity: "none".to_string(),
            schema_version: 1,
            raw_payload: None,
        });

        // 4. Passing test (resolution!)
        events.push(NormalizedEvent {
            event_id: Uuid::now_v7().to_string(),
            event_type: "agent.tool".to_string(),
            timestamp: start + Duration::minutes(15),
            agent: AgentInfo {
                adapter_id: "cursor".to_string(),
                agent_instance_id: None,
            },
            session_id: "session-1".to_string(),
            task_id: Some("task-1".to_string()),
            project: ProjectRef {
                id: Some("proj-1".to_string()),
                slug: None,
                root_path: String::new(),
                source_git_remote: None,
            },
            tool: Some(ToolInfo {
                name: "run_command".to_string(),
                category: "command".to_string(),
                input: Some(json!({"CommandLine": "cargo test"})),
                input_hash: None,
                output: Some(json!("test result: ok. 1 passed; 0 failed")),
                output_hash: None,
                status: "success".to_string(),
                exit_code: Some(0),
                duration_ms: Some(180),
            }),
            artifacts: Vec::new(),
            sensitivity: "none".to_string(),
            schema_version: 1,
            raw_payload: None,
        });

        let digest = engine
            .digest_events(&events, Some("proj-1"), start, now)
            .expect("digest events");

        assert_eq!(digest.metrics.total_tool_calls, 4);
        assert_eq!(digest.metrics.reads_count, 1);
        assert_eq!(digest.metrics.edits_count, 1);
        assert_eq!(digest.metrics.commands_count, 2);
        assert_eq!(digest.metrics.tests_run, 2);
        assert_eq!(digest.metrics.tests_passed, 1);
        assert_eq!(digest.metrics.tests_failed, 1);

        assert!(digest.files_modified.contains(&"/src/main.rs".to_string()));
        assert!(digest.files_inspected.contains(&"/src/main.rs".to_string()));
        assert_eq!(digest.active_agents, vec!["claude-code", "cursor"]);

        // Verify achievements contain the red-to-green resolution
        assert!(digest
            .achievements
            .iter()
            .any(|a| a.contains("Resolved test failure")));

        // Verify failure was matched and marked resolved
        assert_eq!(digest.failures.len(), 1);
        assert!(digest.failures[0].is_resolved);

        // Verify memory synthesis
        let mem = engine
            .synthesize_milestone_memory(&digest, Some("Checkpoint Milestone"))
            .expect("synthesize memory");
        assert_eq!(mem.scope, MemoryScope::Project);
        assert!(mem.content.contains("Activity Window Digest"));
    }
}
