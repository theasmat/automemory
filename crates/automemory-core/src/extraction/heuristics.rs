use crate::models::*;
use chrono::Utc;
use uuid::Uuid;

pub enum ExtractionOutcome {
    Skip,
    AddMemory(Memory),
    AddTrace(SuccessTrace),
    AddFailure(FailureRecord),
}

pub struct EventHeuristics;

/// Commands whose exit code 1 is an ordinary answer (no match, files differ, condition false).
fn expected_nonzero(input: Option<&serde_json::Value>) -> bool {
    let cmd = input
        .and_then(|v| ["command", "CommandLine", "cmd"].iter().find_map(|k| v.get(*k)))
        .map(|c| match c {
            serde_json::Value::Array(a) => a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(" "),
            other => other.as_str().unwrap_or("").to_string(),
        })
        .unwrap_or_default();
    let first = cmd.split(|c: char| c.is_whitespace() || c == '|').find(|w| !w.is_empty() && !w.contains('=')).unwrap_or("");
    matches!(first.rsplit('/').next().unwrap_or(""), "grep" | "egrep" | "fgrep" | "rg" | "diff" | "cmp" | "test" | "[" | "pgrep" | "which" | "command")
}

impl EventHeuristics {
    pub fn evaluate(event: &NormalizedEvent) -> ExtractionOutcome {
        // 1. Check for tool failure
        if event.event_type == "tool.failed"
            || (event
                .tool
                .as_ref()
                .is_some_and(|t| t.status == "failed" || t.exit_code.unwrap_or(0) != 0))
        {
            if let Some(ref tool) = event.tool {
                let error_text = tool
                    .output
                    .as_ref()
                    .and_then(|v| v.as_str())
                    .unwrap_or("Command or tool execution returned non-zero exit code")
                    .to_string();

                // grep, diff, test and friends exit 1 to say "no match" or "different": that is an answer, not a failure.
                if tool.exit_code == Some(1) && expected_nonzero(tool.input.as_ref()) {
                    return ExtractionOutcome::Skip;
                }
                let failure = FailureRecord {
                    id: crate::db::queries::prune::stable_id("fail", &[event.project.id.as_deref().unwrap_or(""), &tool.name, &crate::db::queries::prune::error_signature(&error_text)]),
                    project_id: event.project.id.clone(),
                    task_title: format!("Execution of tool `{}` failed", tool.name),
                    category: Some(tool.category.clone()),
                    failed_tool: Some(tool.name.clone()),
                    error_message: error_text,
                    root_cause_hypothesis: None,
                    fix_applied: None,
                    verification_result: None,
                    status: "unresolved".to_string(),
                    evidence_event_ids: vec![event.event_id.clone()],
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                return ExtractionOutcome::AddFailure(failure);
            }
        }

        // 2. Check for tool modifying critical project files (config, architecture, rules)
        if let Some(ref tool) = event.tool {
            if tool.category == "file_write"
                || tool.category == "edit"
                || tool.name.contains("write")
                || tool.name.contains("replace")
            {
                let file_path = tool
                    .input
                    .as_ref()
                    .and_then(|v| {
                        v.get("TargetFile")
                            .or_else(|| v.get("AbsolutePath"))
                            .or_else(|| v.get("file_path"))
                            .and_then(|s| s.as_str())
                    })
                    .unwrap_or("");

                let lower_path = file_path.to_lowercase();
                if lower_path.ends_with("cargo.toml")
                    || lower_path.ends_with("package.json")
                    || lower_path.ends_with("prd.md")
                    || lower_path.ends_with("architecture.md")
                    || lower_path.contains(".cursor/rules")
                    || lower_path.contains("agents.md")
                    || lower_path.contains("claude.md")
                {
                    let filename = std::path::Path::new(file_path)
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or(file_path);

                    let summary = format!(
                        "Modified key project configuration or specification: {}",
                        filename
                    );
                    let content = format!(
                        "Agent updated configuration file `{}` in session `{}`.",
                        file_path, event.session_id
                    );

                    let memory = Memory {
                        id: Uuid::now_v7().to_string(),
                        scope: if event.project.id.is_some() {
                            MemoryScope::Project
                        } else {
                            MemoryScope::Global
                        },
                        project_id: event.project.id.clone(),
                        memory_type: MemoryType::Fact,
                        content,
                        summary,
                        confidence: 0.95,
                        importance: 0.85,
                        freshness: 1.0,
                        status: MemoryStatus::Active,
                        source_kind: MemorySourceKind::ToolEvent,
                        version: Some(1),
                        created_at: Utc::now(),
                        updated_at: Utc::now(),
                        last_recalled_at: None,
                        tags: vec!["configuration".to_string(), "project-spec".to_string()],
                        entities: vec![filename.to_string()],
                        metadata: serde_json::json!({ "file_path": file_path, "event_id": event.event_id }),
                    };
                    return ExtractionOutcome::AddMemory(memory);
                }
            }
        }

        ExtractionOutcome::Skip
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn failed(cmd: &str, exit: i32, output: &str, event_id: &str) -> NormalizedEvent {
        NormalizedEvent {
            event_id: event_id.into(), event_type: "tool.failed".into(), timestamp: Utc::now(),
            agent: AgentInfo { adapter_id: "claude".into(), agent_instance_id: None }, session_id: "s".into(), task_id: None,
            project: ProjectRef { id: Some("p".into()), slug: None, root_path: "/tmp/p".into(), source_git_remote: None },
            tool: Some(ToolInfo {
                name: "Bash".into(), category: "command".into(), input: Some(serde_json::json!({ "command": cmd })), input_hash: None,
                output: Some(serde_json::json!(output)), output_hash: None, status: "failed".into(), exit_code: Some(exit), duration_ms: None,
            }),
            artifacts: vec![], sensitivity: "low".into(), schema_version: 1, raw_payload: None,
        }
    }

    #[test]
    fn grep_finding_nothing_is_not_a_failure() {
        assert!(matches!(EventHeuristics::evaluate(&failed("grep -rn foo src | head", 1, "", "e1")), ExtractionOutcome::Skip));
        assert!(matches!(EventHeuristics::evaluate(&failed("cargo test", 101, "test failed", "e2")), ExtractionOutcome::AddFailure(_)));
        assert!(matches!(EventHeuristics::evaluate(&failed("cargo test", 1, "error: could not compile", "e3")), ExtractionOutcome::AddFailure(_)), "exit 1 from a build is a failure");
    }

    #[test]
    fn the_same_error_twice_is_one_failure_row() {
        let db = Database::open_in_memory().unwrap();
        for (i, path) in ["/Users/a/x.rs:10", "/home/b/y.rs:99"].iter().enumerate() {
            let ExtractionOutcome::AddFailure(f) = EventHeuristics::evaluate(&failed("cargo build", 101, &format!("error: cannot find value `x` in {path}"), &format!("e{i}"))) else { panic!() };
            db.save_failure(&f).unwrap();
        }
        assert_eq!(db.list_failures(None, None, 100).unwrap().len(), 1);
        let ExtractionOutcome::AddFailure(other) = EventHeuristics::evaluate(&failed("cargo build", 101, "error: linker `cc` not found", "e9")) else { panic!() };
        db.save_failure(&other).unwrap();
        assert_eq!(db.list_failures(None, None, 100).unwrap().len(), 2, "a different error is a different row");
    }
}
