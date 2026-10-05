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

impl EventHeuristics {
    pub fn evaluate(event: &NormalizedEvent) -> ExtractionOutcome {
        // 1. Check for tool failure
        if event.event_type == "tool.failed"
            || (event.tool.as_ref().map_or(false, |t| {
                t.status == "failed" || t.exit_code.unwrap_or(0) != 0
            }))
        {
            if let Some(ref tool) = event.tool {
                let error_text = tool
                    .output
                    .as_ref()
                    .and_then(|v| v.as_str())
                    .unwrap_or("Command or tool execution returned non-zero exit code")
                    .to_string();

                let failure = FailureRecord {
                    id: Uuid::now_v7().to_string(),
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
