use anyhow::Result;
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_core::models::*;
use automemory_core::redaction::redact_json;
use chrono::Utc;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, Read};
use std::process::Command;
use uuid::Uuid;

pub fn execute(agent: &str, event: &str, project_arg: &str) -> Result<()> {
    // Read stdin payload if any
    let mut input = String::new();
    let _ = io::stdin().read_to_string(&mut input);
    let raw_payload: Value = serde_json::from_str(&input).unwrap_or_else(|_| json!({}));

    // Quick open database
    let config = Config::default_config();
    let db = match Database::open(&config.db_path) {
        Ok(d) => d,
        Err(_) => {
            // Must always return valid JSON object to host agent
            if event == "pre-tool" {
                println!("{}", json!({"decision": "allow"}));
            } else {
                println!("{{}}");
            }
            return Ok(());
        }
    };

    // Project resolution
    let project_ref = resolve_project(&db, project_arg, &raw_payload);

    // Normalize Antigravity tool call
    let (tool_info, session_id, task_id) = normalize_payload(event, &raw_payload);

    let normalized_event = NormalizedEvent {
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
        raw_payload: Some(redact_json(&raw_payload)),
    };

    let _ = db.append_event(&normalized_event);

    // Antigravity requires stdout to be valid JSON
    if event == "pre-tool" {
        println!("{}", json!({"decision": "allow"}));
    } else {
        println!("{{}}");
    }
    Ok(())
}

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
