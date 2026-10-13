use crate::db::Database;
use crate::extraction::distiller::TrajectoryDistiller;
use crate::models::NormalizedEvent;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskBoundaryReason {
    GitCommit,
    TestPassAfterWork,
    DeploymentAction,
    BehaviorShift,
    IdleGap,
    ExplicitCompletion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSegment {
    pub session_id: String,
    pub project_id: Option<String>,
    pub task_title: String,
    pub boundary_reason: TaskBoundaryReason,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub events: Vec<NormalizedEvent>,
}

pub struct ImplicitTaskSegmenter<'a> {
    db: &'a Database,
    idle_threshold_seconds: i64,
}

impl<'a> ImplicitTaskSegmenter<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self {
            db,
            idle_threshold_seconds: 60,
        }
    }

    /// Evaluates a session's events and partitions them into coherent implicit task segments
    /// based on git commits, test passes, behavior shifts, and temporal idle gaps.
    pub fn segment_session_events(&self, events: &[NormalizedEvent]) -> Vec<TaskSegment> {
        let mut segments = Vec::new();
        if events.is_empty() {
            return segments;
        }

        let mut current_bucket: Vec<NormalizedEvent> = Vec::new();
        let mut had_file_edits_or_failures = false;
        let mut consecutive_reads = 0;

        for (i, event) in events.iter().enumerate() {
            let is_read = event
                .tool
                .as_ref()
                .map(|t| {
                    t.category == "file_read"
                        || t.name.contains("view")
                        || t.name.contains("read")
                        || t.name.contains("grep")
                        || t.name.contains("list")
                })
                .unwrap_or(false);

            let is_edit = event
                .tool
                .as_ref()
                .map(|t| {
                    t.category == "file_write"
                        || t.category == "edit"
                        || t.name.contains("write")
                        || t.name.contains("replace")
                        || t.name.contains("edit")
                })
                .unwrap_or(false);

            if is_read {
                consecutive_reads += 1;
            }

            // Behavioral Shift: transitioning from prolonged research (>=6 reads) to modification
            let mut boundary: Option<(TaskBoundaryReason, String)> = None;
            if is_edit && consecutive_reads >= 6 && current_bucket.len() >= 6 {
                boundary = Some((
                    TaskBoundaryReason::BehaviorShift,
                    "Investigation & Research Phase".to_string(),
                ));
                consecutive_reads = 0;
            }

            if is_edit
                || event
                    .tool
                    .as_ref()
                    .map(|t| t.status == "failed")
                    .unwrap_or(false)
            {
                had_file_edits_or_failures = true;
                consecutive_reads = 0;
            }

            if let Some(tool) = &event.tool {
                let cmd = tool
                    .input
                    .as_ref()
                    .and_then(|v| {
                        v.get("CommandLine")
                            .or_else(|| v.get("command"))
                            .and_then(|c| c.as_str())
                    })
                    .unwrap_or("");

                // 1. Git Commit Boundary
                if cmd.contains("git commit") || cmd.contains("git merge") {
                    boundary = Some((
                        TaskBoundaryReason::GitCommit,
                        format!("Git Commit: {}", cmd.trim()),
                    ));
                    consecutive_reads = 0;
                }
                // 2. Test Pass after work
                else if had_file_edits_or_failures
                    && (cmd.contains("test") || cmd.contains("pytest") || cmd.contains("check"))
                    && tool.status == "success"
                {
                    boundary = Some((
                        TaskBoundaryReason::TestPassAfterWork,
                        format!("Verified Task via: {}", cmd.trim()),
                    ));
                    had_file_edits_or_failures = false;
                    consecutive_reads = 0;
                }
                // 3. Deployment Action
                else if cmd.contains("publish")
                    || cmd.contains("deploy")
                    || cmd.contains("kubectl apply")
                {
                    boundary = Some((
                        TaskBoundaryReason::DeploymentAction,
                        format!("Deployment: {}", cmd.trim()),
                    ));
                    consecutive_reads = 0;
                }
            }

            // 4. Temporal Idle Gap
            if boundary.is_none() && i + 1 < events.len() {
                let next_time = events[i + 1].timestamp;
                let diff = next_time
                    .signed_duration_since(event.timestamp)
                    .num_seconds();
                if diff >= self.idle_threshold_seconds {
                    boundary = Some((
                        TaskBoundaryReason::IdleGap,
                        format!("Session Activity Window ({}s idle gap)", diff),
                    ));
                    consecutive_reads = 0;
                }
            }

            // Append event to current bucket
            current_bucket.push(event.clone());

            // If boundary triggered, flush bucket to segment
            if let Some((reason, title)) = boundary {
                if !current_bucket.is_empty() {
                    let start_time = current_bucket
                        .first()
                        .map(|e| e.timestamp)
                        .unwrap_or_else(Utc::now);
                    let end_time = current_bucket
                        .last()
                        .map(|e| e.timestamp)
                        .unwrap_or_else(Utc::now);
                    let session_id = current_bucket[0].session_id.clone();
                    let project_id = current_bucket[0].project.id.clone();

                    segments.push(TaskSegment {
                        session_id,
                        project_id,
                        task_title: title,
                        boundary_reason: reason,
                        start_time,
                        end_time,
                        events: std::mem::take(&mut current_bucket),
                    });
                }
            }
        }

        // Remaining events flush
        if !current_bucket.is_empty() {
            let start_time = current_bucket
                .first()
                .map(|e| e.timestamp)
                .unwrap_or_else(Utc::now);
            let end_time = current_bucket
                .last()
                .map(|e| e.timestamp)
                .unwrap_or_else(Utc::now);
            let session_id = current_bucket[0].session_id.clone();
            let project_id = current_bucket[0].project.id.clone();

            segments.push(TaskSegment {
                session_id,
                project_id,
                task_title: "Active Task Episode".to_string(),
                boundary_reason: TaskBoundaryReason::ExplicitCompletion,
                start_time,
                end_time,
                events: current_bucket,
            });
        }

        segments
    }

    /// Automatically distills all detected segments in a session and stores success traces.
    pub fn process_and_distill_session(&self, session_id: &str) -> Result<usize> {
        let events = self.db.get_session_events(session_id)?;
        let segments = self.segment_session_events(&events);
        let distiller = TrajectoryDistiller::new(self.db);
        let mut traces_created = 0;

        for seg in segments {
            if seg.events.len() >= 2 {
                let report = distiller.distill_events(&seg.events, Some(session_id))?;
                if report.synthesized_trace.is_some() {
                    traces_created += 1;
                }
            }
        }

        Ok(traces_created)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AgentInfo, ProjectRef, ToolInfo};
    use chrono::Duration;

    #[test]
    fn test_implicit_segmentation_on_git_commit() {
        let db = Database::open_in_memory().unwrap();
        let segmenter = ImplicitTaskSegmenter::new(&db);

        let now = Utc::now();
        let events = vec![
            NormalizedEvent {
                event_id: "e1".to_string(),
                event_type: "tool.failed".to_string(),
                timestamp: now,
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
                    name: "run_command".to_string(),
                    category: "command".to_string(),
                    input: Some(serde_json::json!({ "CommandLine": "cargo test" })),
                    input_hash: None,
                    output: Some(serde_json::json!("failed")),
                    output_hash: None,
                    status: "failed".to_string(),
                    exit_code: Some(1),
                    duration_ms: Some(100),
                }),
                artifacts: vec![],
                sensitivity: "public".to_string(),
                schema_version: 1,
                raw_payload: None,
            },
            NormalizedEvent {
                event_id: "e2".to_string(),
                event_type: "tool.completed".to_string(),
                timestamp: now + Duration::seconds(5),
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
                    name: "write_to_file".to_string(),
                    category: "file_write".to_string(),
                    input: Some(serde_json::json!({ "TargetFile": "src/lib.rs" })),
                    input_hash: None,
                    output: Some(serde_json::json!("ok")),
                    output_hash: None,
                    status: "success".to_string(),
                    exit_code: Some(0),
                    duration_ms: Some(50),
                }),
                artifacts: vec![],
                sensitivity: "public".to_string(),
                schema_version: 1,
                raw_payload: None,
            },
            NormalizedEvent {
                event_id: "e3".to_string(),
                event_type: "tool.completed".to_string(),
                timestamp: now + Duration::seconds(10),
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
                    name: "run_command".to_string(),
                    category: "command".to_string(),
                    input: Some(
                        serde_json::json!({ "CommandLine": "git commit -m 'fix: resolve compilation bug'" }),
                    ),
                    input_hash: None,
                    output: Some(serde_json::json!("[main 12345] fix")),
                    output_hash: None,
                    status: "success".to_string(),
                    exit_code: Some(0),
                    duration_ms: Some(200),
                }),
                artifacts: vec![],
                sensitivity: "public".to_string(),
                schema_version: 1,
                raw_payload: None,
            },
        ];

        let segments = segmenter.segment_session_events(&events);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].boundary_reason, TaskBoundaryReason::GitCommit);
        assert_eq!(segments[0].events.len(), 3);
    }
}
