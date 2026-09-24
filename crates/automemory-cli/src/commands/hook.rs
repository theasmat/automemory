use anyhow::Result;
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_core::guardrails::{GuardrailAction, GuardrailEvaluator, GuardrailSeverity};
use automemory_core::models::*;
use automemory_core::redaction::redact_json;
use chrono::Utc;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, Read};
use std::process::Command;
use uuid::Uuid;

/// Main entrypoint for the CLI `hook` command.
/// Evaluates guardrails on `pre-tool` events and records append-only event traces.
pub fn execute(agent: &str, event: &str, project_arg: &str) -> Result<()> {
    // Read stdin payload if any
    let mut input = String::new();
    let _ = io::stdin().read_to_string(&mut input);
    let raw_payload: Value = serde_json::from_str(&input).unwrap_or_else(|_| json!({}));

    // Quick open database; gracefully fail-open if DB is unreachable
    let config = Config::default_config();
    let db = match Database::open(&config.db_path) {
        Ok(d) => d,
        Err(_) => {
            if event == "pre-tool" {
                println!("{}", json!({"decision": "allow"}));
            } else {
                println!("{{}}");
            }
            return Ok(());
        }
    };

    // Project resolution & autopilot mode lookup
    let project_ref = resolve_project(&db, project_arg, &raw_payload);
    let current_project = project_ref
        .id
        .as_deref()
        .and_then(|id| db.get_project_by_id(id).ok().flatten());
    let autopilot_mode = current_project
        .as_ref()
        .map(|p| p.capture_policy.mode)
        .unwrap_or(AutopilotMode::Vibe);

    // Normalize Antigravity / AGY tool call
    let (tool_info, session_id, task_id) = normalize_payload(event, &raw_payload);

    // ── Pre-Tool Guardrail Evaluation ─────────────────────────────────────────
    if event == "pre-tool" {
        if !evaluate_pre_tool_guardrail(&db, &raw_payload, tool_info.as_ref(), project_ref.id.as_deref(), autopilot_mode)? {
            return Ok(());
        }
    }

    // ── Record Event ──────────────────────────────────────────────────────────
    let normalized_event = build_normalized_event(
        agent,
        event,
        session_id.clone(),
        task_id,
        project_ref,
        tool_info,
        &raw_payload,
    );
    let _ = db.append_event(&normalized_event);

    // ── Post-Tool Implicit Task Segmentation ──────────────────────────────────
    if event == "post-tool" || event == "stop" {
        run_post_tool_distillation(&db, &session_id);
    }

    if event != "pre-tool" {
        println!("{{}}");
    }
    Ok(())
}

/// Evaluates pre-tool guardrails against registered security & workflow policies.
/// Returns `false` if the process should stop or has exited.
fn evaluate_pre_tool_guardrail(
    db: &Database,
    raw_payload: &Value,
    tool_info: Option<&ToolInfo>,
    project_id: Option<&str>,
    autopilot_mode: AutopilotMode,
) -> Result<bool> {
    let tool_name = tool_info
        .map(|t| t.name.as_str())
        .or_else(|| raw_payload.get("toolName").and_then(|v| v.as_str()))
        .or_else(|| raw_payload.get("name").and_then(|v| v.as_str()))
        .unwrap_or("unknown_tool");

    let tool_args = tool_info
        .and_then(|t| t.input.as_ref())
        .unwrap_or(raw_payload);

    let evaluator = GuardrailEvaluator::new(Some(db));
    let decision = evaluator.evaluate(tool_name, tool_args, project_id, autopilot_mode);

    match decision.action {
        GuardrailAction::Block => {
            eprintln!("🛑 [AutoMemory Guardrail] CRITICAL STOP: {}", decision.title);
            eprintln!("   Reason: {}", decision.risk_summary);
            if let Some(hint) = &decision.mitigation_hint {
                eprintln!("   💡 Mitigation: {}", hint);
            }
            println!("{}", serde_json::to_string(&decision)?);
            if decision.severity == GuardrailSeverity::Critical {
                std::process::exit(1);
            }
            Ok(false)
        }
        GuardrailAction::Warn => {
            eprintln!("⚠️ [AutoMemory Guardrail Warning]: {}", decision.title);
            if let Some(hint) = &decision.mitigation_hint {
                eprintln!("   💡 Tip: {}", hint);
            }
            println!("{}", serde_json::to_string(&decision)?);
            Ok(true)
        }
        GuardrailAction::Allow => {
            println!("{}", json!({"decision": "allow", "action": "allow"}));
            Ok(true)
        }
    }
}

/// Constructs a `NormalizedEvent` from parsed parameters and sanitized raw payload.
fn build_normalized_event(
    agent: &str,
    event: &str,
    session_id: String,
    task_id: Option<String>,
    project_ref: ProjectRef,
    tool_info: Option<ToolInfo>,
    raw_payload: &Value,
) -> NormalizedEvent {
    NormalizedEvent {
        event_id: Uuid::now_v7().to_string(),
        event_type: format!("{}.{}", agent, event),
        timestamp: Utc::now(),
        agent: AgentInfo {
            adapter_id: agent.to_string(),
            agent_instance_id: None,
        },
        session_id,
        task_id,
        project: project_ref,
        tool: tool_info,
        artifacts: Vec::new(),
        sensitivity: "redacted".to_string(),
        schema_version: 1,
        raw_payload: Some(redact_json(raw_payload)),
    }
}

/// Runs implicit task segmentation and session distillation in the background.
fn run_post_tool_distillation(db: &Database, session_id: &str) {
    let segmenter = automemory_core::extraction::ImplicitTaskSegmenter::new(db);
    let _ = segmenter.process_and_distill_session(session_id);
}

/// Resolves the project reference based on CLI flags, environment variables,
/// payload workspace paths, git top-level, or working directory fallback.
fn resolve_project(db: &Database, project_arg: &str, payload: &Value) -> ProjectRef {
    if project_arg != "auto" {
        if let Ok(Some(p)) = db.get_project_by_slug(project_arg) {
            return ProjectRef {
                id: Some(p.id),
                slug: Some(p.slug),
                root_path: String::new(),
                source_git_remote: None,
            };
        }
    }

    // 1. Check environment variable
    if let Ok(env_name) = std::env::var("AGENTMEMORY_PROJECT_NAME") {
        if !env_name.trim().is_empty() {
            if let Ok(Some(p)) = db.get_project_by_slug(&env_name) {
                return ProjectRef {
                    id: Some(p.id),
                    slug: Some(p.slug),
                    root_path: String::new(),
                    source_git_remote: None,
                };
            }
        }
    }

    // 2. Check payload cwd or workspace paths
    let cwd_candidate = payload
        .get("cwd")
        .or_else(|| payload.get("workspacePaths").and_then(|arr| arr.get(0)))
        .and_then(|v| v.as_str())
        .map(String::from);

    let current_dir = cwd_candidate.unwrap_or_else(|| {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string())
    });

    // 3. Match against registered projects in DB
    if let Ok(Some(p)) = db.find_project_by_path(&current_dir) {
        return ProjectRef {
            id: Some(p.id),
            slug: Some(p.slug),
            root_path: current_dir,
            source_git_remote: None,
        };
    }

    // 4. Git root detection
    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&current_dir)
        .output()
    {
        if output.status.success() {
            let git_root = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Ok(Some(p)) = db.find_project_by_path(&git_root) {
                return ProjectRef {
                    id: Some(p.id),
                    slug: Some(p.slug),
                    root_path: git_root,
                    source_git_remote: None,
                };
            }
        }
    }

    // Fallback: basename of directory
    let folder = std::path::Path::new(&current_dir)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("default");

    ProjectRef {
        id: None,
        slug: Some(folder.to_lowercase().replace(' ', "-")),
        root_path: current_dir,
        source_git_remote: None,
    }
}

/// Normalizes arbitrary agent payloads (Antigravity, Claude Code, Cursor, Copilot)
/// into a structured `(Option<ToolInfo>, session_id, Option<task_id>)`.
fn normalize_payload(
    event: &str,
    raw: &Value,
) -> (Option<ToolInfo>, String, Option<String>) {
    let session_id = raw
        .get("conversationId")
        .or_else(|| raw.get("session_id"))
        .or_else(|| raw.get("sessionId"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown-session")
        .to_string();

    let task_id = raw
        .get("task_id")
        .or_else(|| raw.get("taskId"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let tool_call = raw.get("toolCall").or_else(|| raw.get("tool_call"));

    let tool_info = if let Some(tc) = tool_call {
        let raw_name = tc
            .get("name")
            .or_else(|| tc.get("toolName"))
            .and_then(|v| v.as_str())
            .unwrap_or("tool");

        // Map Antigravity tool names to canonical categories
        let (category, canonical_name) = match raw_name {
            "view_file" | "read_file" | "view_line_range" | "read_url_content" => {
                ("file_read", "read")
            }
            "write_to_file" | "create_file" => ("file_write", "write"),
            "replace_file_content" | "edit_file" | "propose_code" => ("edit", "edit"),
            "grep_search" | "codebase_search" => ("search", "grep"),
            "list_dir" | "find_by_name" => ("search", "glob"),
            "run_command" => ("command", "command"),
            other => ("custom", other),
        };

        let raw_args = tc.get("args").or_else(|| tc.get("toolArgs"));
        let normalized_args = normalize_args(raw_args);

        let result_val = tc
            .get("result")
            .or_else(|| raw.get("toolResult"))
            .or_else(|| raw.get("result"));

        let status = if event.contains("fail") {
            "failed".to_string()
        } else {
            "success".to_string()
        };

        Some(ToolInfo {
            name: canonical_name.to_string(),
            category: category.to_string(),
            input: Some(redact_json(&normalized_args)),
            input_hash: None,
            output: result_val.map(redact_json),
            output_hash: None,
            status,
            exit_code: None,
            duration_ms: None,
        })
    } else {
        None
    };

    (tool_info, session_id, task_id)
}

/// Normalizes argument keys to canonical representations across different agent adapters.
fn normalize_args(args: Option<&Value>) -> Value {
    let mut map = serde_json::Map::new();
    if let Some(Value::Object(obj)) = args {
        let key_map: HashMap<&str, &str> = [
            ("AbsolutePath", "file_path"),
            ("TargetFile", "file_path"),
            ("DirectoryPath", "path"),
            ("SearchDirectory", "path"),
            ("CommandLine", "command"),
            ("Query", "pattern"),
            ("Pattern", "pattern"),
        ]
        .iter()
        .cloned()
        .collect();

        for (k, v) in obj {
            if let Some(target_key) = key_map.get(k.as_str()) {
                map.insert(target_key.to_string(), v.clone());
            }
            map.insert(k.clone(), v.clone());
        }
    }
    Value::Object(map)
}
