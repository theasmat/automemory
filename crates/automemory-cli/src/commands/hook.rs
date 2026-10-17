use anyhow::Result;
use automemory_core::config::{Config, GlobalConfig};
use automemory_core::db::Database;
use automemory_core::hooks::{
    merge_decision, run_pipeline, CanonicalEvent, GateDecision, GuardrailHandler, HooksConfig,
    LockCheckHandler, PipelineCtx,
};
use automemory_core::models::*;
use automemory_core::ports::{HandlerEnv, HookHandler};
use automemory_core::redaction::redact_json;
use chrono::Utc;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

/// Main entrypoint for the CLI `hook` command.
/// Evaluates guardrails on `pre-tool` events and records append-only event traces.
pub fn execute(agent: &str, event: &str, project_arg: &str) -> Result<()> {
    // Read stdin payload if piped; skip if running interactively in terminal
    let mut input = String::new();
    if !io::stdin().is_terminal() {
        let _ = io::stdin().read_to_string(&mut input);
    }
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
        .and_then(|id| db.get_project_by_id(id).ok().flatten())
        .or_else(|| {
            project_ref
                .slug
                .as_deref()
                .and_then(|s| db.get_project_by_slug(s).ok().flatten())
        });
    let autopilot_mode = current_project
        .as_ref()
        .map(|p| p.capture_policy.mode)
        .unwrap_or(AutopilotMode::Vibe);

    // Normalize Antigravity / AGY tool call
    let (tool_info, session_id, task_id) = normalize_payload(event, &raw_payload);

    let project_id_owned = project_ref.id.clone();
    let effective_project_id = current_project
        .as_ref()
        .map(|p| p.id.as_str())
        .or(project_id_owned.as_deref());

    // ── Boot / Pre-Invocation Context Injection ────────────────────────────────
    // Gated off by default: PRD pull-principle (§5) reserves context for
    // explicit queries; enable with [defaults] boot_injection = true.
    let canonical = CanonicalEvent::from_cli(event);
    if canonical == Some(CanonicalEvent::SessionStart) {
        let boot_on = GlobalConfig::load()
            .map(|g| g.defaults.boot_injection)
            .unwrap_or(false);
        if !boot_on {
            println!("{{}}");
            return Ok(());
        }
        let engine = automemory_core::retrieval::RetrievalEngine::new(&db);
        let ctx = engine
            .assemble_context(effective_project_id, None, 2000)
            .unwrap_or_default();
        if !ctx.trim().is_empty() {
            if native_hook_style(agent) {
                println!("{}", native_context("SessionStart", &ctx));
            } else {
                println!(
                    "{}",
                    json!({
                        "context": ctx,
                        "additionalContext": ctx,
                        "prompt": ctx,
                    })
                );
            }
        } else {
            println!("{{}}");
        }
        return Ok(());
    }

    // ── Pending-work nudge: only speaks when something is assigned ─────────
    if canonical == Some(CanonicalEvent::UserPromptSubmit) {
        if let Some(nudge) = pending_work_nudge(&db, effective_project_id, &session_id) {
            if native_hook_style(agent) {
                println!("{}", native_context("UserPromptSubmit", &nudge));
            } else {
                println!("{}", json!({ "nudge": nudge }));
            }
        } else {
            println!("{{}}");
        }
        return Ok(());
    }
    // ── Ordered Gate Pipeline (lock check → guardrail → custom exec) ──────────
    // Strictest decision wins, so a catastrophic command is denied even when a
    // lease check merely asks. Hot-reloaded: hooks.toml is re-read every call.
    if matches!(
        canonical,
        Some(CanonicalEvent::PreToolUse | CanonicalEvent::PreCommit)
    ) {
        let gate_event = canonical.unwrap_or(CanonicalEvent::PreToolUse);
        let tool_name = tool_info
            .as_ref()
            .map(|t| t.name.as_str())
            .or_else(|| raw_payload.get("toolName").and_then(|v| v.as_str()))
            .or_else(|| raw_payload.get("name").and_then(|v| v.as_str()))
            .unwrap_or("unknown_tool");
        let tool_args = tool_info
            .as_ref()
            .and_then(|t| t.input.as_ref())
            .unwrap_or(&raw_payload);
        let (mut paths, symbols) = extract_target_paths_and_symbols(tool_name, tool_args);
        if gate_event == CanonicalEvent::PreCommit {
            for f in staged_files(&project_ref.root_path) {
                if !paths.contains(&f) {
                    paths.push(f);
                }
            }
        }
        let pipe_ctx = PipelineCtx {
            event: gate_event,
            agent,
            session_id: session_id.as_str(),
            tool: tool_name,
            args: tool_args,
            paths: &paths,
            symbols: &symbols,
            is_mutation: gate_event == CanonicalEvent::PreCommit || is_mutation_tool(tool_name),
        };
        let hooks_cfg = HooksConfig::load(
            &config.home_dir.join("hooks.toml"),
            &project_hooks_path(&project_ref),
        );
        let env = HandlerEnv {
            db: &db,
            project_id: effective_project_id,
            mode: autopilot_mode,
        };
        let lock_check = LockCheckHandler;
        let guardrail = GuardrailHandler;
        let handlers: Vec<&dyn HookHandler> = vec![&lock_check, &guardrail];
        let results = run_pipeline(&hooks_cfg.handler, &pipe_ctx, &env, &handlers);
        let merged = merge_decision(&results);
        if native_hook_style(agent) {
            println!(
                "{}",
                native_gate_output(gate_event, merged.decision, &merged.reason)
            );
        } else if merged.reason.is_empty() {
            println!("{}", json!({"decision": merged.decision.as_str()}));
        } else {
            println!(
                "{}",
                json!({"decision": merged.decision.as_str(), "reason": merged.reason})
            );
        }
        if merged.decision == GateDecision::Deny {
            return Ok(());
        }
    }

    // ── Record Event ──────────────────────────────────────────────────────────
    let project_root = project_ref.root_path.clone();
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

    // ── Session Completion & Major Milestone Distillation ────────────────────
    // Intermediate tool calls (view, grep, edit, read) only append to raw telemetry.
    // Full distillation only runs at session stop or major git commit completion.
    let is_major_cmd = normalized_event
        .tool
        .as_ref()
        .and_then(|t| t.input.as_ref())
        .and_then(|i| {
            i.get("CommandLine")
                .or_else(|| i.get("command"))
                .and_then(|v| v.as_str())
        })
        .is_some_and(|cmd| {
            let c = cmd.to_lowercase();
            c.contains("git commit") || c.contains("git merge")
        });

    if canonical == Some(CanonicalEvent::Stop)
        || canonical == Some(CanonicalEvent::PostCommit)
        || is_major_cmd
    {
        run_post_tool_distillation(&db, &session_id);
    }

    // Auto-rescan and update document intelligence if any plan/spec/doc was touched
    auto_rescan_doc_if_applicable(
        &db,
        &project_root,
        effective_project_id,
        normalized_event.tool.as_ref(),
        &raw_payload,
    );

    // Release all active leases held by this session upon stop — unless work
    // is owed, in which case the stop is blocked (PRD §7.2) and the human is
    // told why through the Notifier port.
    if canonical == Some(CanonicalEvent::Stop) {
        if let Some(owed) = pending_work_nudge(&db, effective_project_id, &session_id) {
            active_notifier().notify("stop blocked", &owed);
            println!("{}", json!({"decision": "block", "reason": owed}));
            return Ok(());
        }
        let _ = db.release_leases(&session_id, None);
    }

    // PostCommit staleness: committed files flip linked memories to `changed`.
    if canonical == Some(CanonicalEvent::PostCommit) {
        let files = commit_files(&project_root);
        match db.flag_memories_changed_by_files(effective_project_id, &files) {
            Ok(flipped) if !flipped.is_empty() => eprintln!(
                "[AutoMemory] PostCommit: {} linked memories marked changed ({} files).",
                flipped.len(),
                files.len()
            ),
            _ => {}
        }
    }

    if !matches!(
        canonical,
        Some(CanonicalEvent::PreToolUse | CanonicalEvent::PreCommit)
    ) {
        println!("{{}}");
    }
    Ok(())
}

/// Claude Code and Codex share one hook wire format (`hookSpecificOutput`);
/// Antigravity and the rest use the flat `{"decision": ...}` shape.
fn native_hook_style(agent: &str) -> bool {
    matches!(
        agent.to_ascii_lowercase().as_str(),
        "claude" | "claude-code" | "claudecode" | "codex" | "openai"
    )
}

/// `hookSpecificOutput` carrying extra model-visible context.
fn native_context(event_name: &str, text: &str) -> Value {
    json!({
        "hookSpecificOutput": {
            "hookEventName": event_name,
            "additionalContext": text,
        }
    })
}

/// Maps a pipeline decision onto Claude Code / Codex PreToolUse output.
/// Only a deny blocks; warn and ask surface as context so the agent sees why.
fn native_gate_output(event: CanonicalEvent, decision: GateDecision, reason: &str) -> Value {
    let name = if event == CanonicalEvent::PreCommit {
        "PreToolUse"
    } else {
        event.as_str()
    };
    match decision {
        GateDecision::Deny => json!({
            "hookSpecificOutput": {
                "hookEventName": name,
                "permissionDecision": "deny",
                "permissionDecisionReason": reason,
            }
        }),
        GateDecision::Allow if reason.is_empty() => json!({}),
        _ => native_context(name, &format!("[AutoMemory] {reason}")),
    }
}

/// Resolves the configured notifier, failing closed to stderr on unknown
/// names (a broken ping channel must stay loud, never silent).
fn active_notifier() -> Box<dyn automemory_core::ports::Notifier> {
    let name = automemory_core::config::GlobalConfig::load()
        .map(|g| g.defaults.notifier)
        .unwrap_or_else(|_| "stderr".to_string());
    automemory_core::ports::notifier_for(&name)
        .unwrap_or_else(|_| Box::new(automemory_core::ports::StderrNotifier))
}

/// Pending-work nudge for UserPromptSubmit: returns `Some` only when this
/// session holds a claimed task, has an addressed baton, or owes a review.
fn pending_work_nudge(db: &Database, project_id: Option<&str>, session_id: &str) -> Option<String> {
    let mut items = Vec::new();
    if let Ok(tasks) = db.list_tasks(project_id, Some(TaskState::Claimed), 10) {
        if let Some(t) = tasks
            .into_iter()
            .find(|t| t.owner_session.as_deref() == Some(session_id))
        {
            items.push(format!("claimed task: {}", t.title));
        }
    }
    if db
        .latest_baton(project_id, Some(session_id))
        .ok()
        .flatten()
        .is_some()
    {
        items.push("a handoff baton awaits pickup (baton.pickup)".to_string());
    }
    if db
        .next_review(project_id, Some(session_id))
        .ok()
        .flatten()
        .is_some()
    {
        items.push("a diff awaits your review (review.next)".to_string());
    }
    if items.is_empty() {
        None
    } else {
        Some(format!("Pending work: {}.", items.join("; ")))
    }
}

/// Staged (to-be-committed) files for PreCommit lease-overlap checks.
fn staged_files(root_path: &str) -> Vec<String> {
    git_files(root_path, &["diff", "--cached", "--name-only"])
}

/// Files touched by HEAD for PostCommit staleness marking.
fn commit_files(root_path: &str) -> Vec<String> {
    git_files(
        root_path,
        &["show", "--pretty=format:", "--name-only", "HEAD"],
    )
}

fn git_files(root_path: &str, args: &[&str]) -> Vec<String> {
    let dir = if root_path.is_empty() {
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
    } else {
        std::path::PathBuf::from(root_path)
    };
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Project-level hooks file; pairs with the global `~/.automemory/hooks.toml`.
fn project_hooks_path(project_ref: &ProjectRef) -> std::path::PathBuf {
    let dir = if project_ref.root_path.is_empty() {
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
    } else {
        std::path::PathBuf::from(&project_ref.root_path)
    };
    dir.join(".automemory").join("hooks.toml")
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
    use automemory_core::extraction::TrajectoryDistiller;
    let segmenter = automemory_core::extraction::ImplicitTaskSegmenter::new(db);
    let distiller = TrajectoryDistiller::new(db);
    let _ = segmenter.process_and_distill_session_with(&distiller, session_id);
}

/// Automatically rescans and recompiles document intelligence if any tracked document was touched.
fn auto_rescan_doc_if_applicable(
    db: &Database,
    project_root: &str,
    effective_project_id: Option<&str>,
    tool_info: Option<&ToolInfo>,
    raw_payload: &Value,
) {
    let tool = match tool_info {
        Some(t) => t,
        None => return,
    };
    if tool.category != "file_write" && tool.category != "edit" {
        return;
    }
    let (paths, _) =
        extract_target_paths_and_symbols(&tool.name, tool.input.as_ref().unwrap_or(raw_payload));
    let touches_doc = paths.iter().any(|p| {
        let l = p.to_lowercase();
        l.ends_with(".md")
            && (l.contains("plan")
                || l.contains("doc")
                || l.contains("spec")
                || l.contains("prd")
                || l.contains("adr")
                || l.contains("rfc")
                || l.contains("rule")
                || l.contains("roadmap")
                || l.contains("architecture")
                || l.contains("agents")
                || l.contains("claude"))
    });
    if touches_doc {
        let p_id = effective_project_id.unwrap_or("auto");
        let root = if project_root.is_empty() {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        } else {
            PathBuf::from(project_root)
        };
        let scanner = automemory_core::docs::DocumentScanner::new(std::sync::Arc::new(db.clone()));
        let proj = db
            .get_project_by_id(p_id)
            .ok()
            .flatten()
            .or_else(|| db.get_project_by_slug(p_id).ok().flatten())
            .unwrap_or_else(|| automemory_core::models::Project {
                id: p_id.to_string(),
                slug: p_id.to_string(),
                display_name: p_id.to_string(),
                document_config: Default::default(),
                sync_config: Default::default(),
                capture_policy: Default::default(),
                context_budget: Default::default(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            });
        for p in &paths {
            let full_path = if Path::new(p).is_absolute() {
                PathBuf::from(p)
            } else {
                root.join(p)
            };
            if full_path.is_file() {
                let _ = scanner.process_file(&proj, &root, &full_path);
            }
        }
    }
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

    // 3. Match against registered projects in DB (worktree-aware via Database::find_project_by_path)
    if let Ok(Some(p)) = db.find_project_by_path(&current_dir) {
        return ProjectRef {
            id: Some(p.id),
            slug: Some(p.slug),
            root_path: current_dir,
            source_git_remote: None,
        };
    }

    // 4. Git root / worktree canonical root detection
    let canonical_root = automemory_core::sync::GitSync::resolve_git_canonical_root(
        std::path::Path::new(&current_dir),
    );
    if let Some(ref git_root) = canonical_root {
        let git_root_str = git_root.to_string_lossy().to_string();
        if let Ok(Some(p)) = db.find_project_by_path(&git_root_str) {
            return ProjectRef {
                id: Some(p.id),
                slug: Some(p.slug),
                root_path: current_dir,
                source_git_remote: None,
            };
        }
    }

    // Fallback: derive slug from canonical git root if available, so all worktrees share the same slug!
    let folder = canonical_root
        .as_ref()
        .and_then(|r| r.file_name())
        .or_else(|| std::path::Path::new(&current_dir).file_name())
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
fn normalize_payload(event: &str, raw: &Value) -> (Option<ToolInfo>, String, Option<String>) {
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

    let tool_call = raw
        .get("toolCall")
        .or_else(|| raw.get("tool_call"))
        .or_else(|| {
            if raw.get("toolName").is_some()
                || raw.get("tool_name").is_some()
                || raw.get("name").is_some()
                || raw.get("tool").is_some()
            {
                Some(raw)
            } else {
                None
            }
        });

    let tool_info = if let Some(tc) = tool_call {
        let raw_name = tc
            .get("name")
            .or_else(|| tc.get("toolName"))
            .or_else(|| tc.get("tool_name"))
            .or_else(|| tc.get("tool"))
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
            "run_command" | "Bash" | "shell" | "local_shell" | "exec_command"
            | "container.exec" => ("command", "command"),
            "Read" => ("file_read", "read"),
            "Write" => ("file_write", "write"),
            "Edit" | "MultiEdit" | "NotebookEdit" | "apply_patch" => ("edit", "edit"),
            "Grep" => ("search", "grep"),
            "Glob" => ("search", "glob"),
            other => ("custom", other),
        };

        let raw_args = tc
            .get("args")
            .or_else(|| tc.get("toolArgs"))
            .or_else(|| tc.get("tool_input"))
            .or_else(|| tc.get("arguments"))
            .or_else(|| tc.get("input"));
        let normalized_args = normalize_args(raw_args);

        let result_val = tc
            .get("result")
            .or_else(|| tc.get("output"))
            .or_else(|| tc.get("tool_response"))
            .or_else(|| raw.get("toolResult"))
            .or_else(|| raw.get("result"))
            .or_else(|| raw.get("output"));

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
            ("cmd", "command"),
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
        // Codex passes argv arrays (["bash", "-lc", "..."]); guardrails expect a string.
        if let Some(Value::Array(argv)) = map.get("command") {
            let joined = argv
                .iter()
                .filter_map(|a| a.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            map.insert("command".to_string(), Value::String(joined));
        }
    }
    Value::Object(map)
}

/// Extracts target file paths and symbols from normalized tool arguments.
fn extract_target_paths_and_symbols(
    _tool_name: &str,
    tool_args: &Value,
) -> (Vec<String>, Vec<String>) {
    let mut paths = Vec::new();
    let mut symbols = Vec::new();

    let path_keys = [
        "file_path",
        "filePath",
        "path",
        "file",
        "TargetFile",
        "AbsolutePath",
        "relative_path",
        "filename",
        "files",
        "paths",
    ];

    for key in path_keys {
        if let Some(val) = tool_args.get(key) {
            if let Some(s) = val.as_str() {
                if !s.is_empty() {
                    paths.push(s.to_string());
                }
            } else if let Some(arr) = val.as_array() {
                for item in arr {
                    if let Some(s) = item.as_str() {
                        if !s.is_empty() {
                            paths.push(s.to_string());
                        }
                    }
                }
            }
        }
    }

    let symbol_keys = [
        "symbol",
        "symbols",
        "symbol_name",
        "function_name",
        "target_symbol",
    ];

    for key in symbol_keys {
        if let Some(val) = tool_args.get(key) {
            if let Some(s) = val.as_str() {
                if !s.is_empty() {
                    symbols.push(s.to_string());
                }
            } else if let Some(arr) = val.as_array() {
                for item in arr {
                    if let Some(s) = item.as_str() {
                        if !s.is_empty() {
                            symbols.push(s.to_string());
                        }
                    }
                }
            }
        }
    }

    // Codex apply_patch carries file paths inside the patch text.
    for key in ["input", "patch"] {
        if let Some(text) = tool_args.get(key).and_then(|v| v.as_str()) {
            paths.extend(patch_paths(text));
        }
    }

    // Ensure both absolute and relative path forms are checked for lease collisions
    let mut normalized_extras = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_str = cwd.to_string_lossy();
        for p in &paths {
            if let Some(rel) = p.strip_prefix(cwd_str.as_ref()) {
                let trimmed = rel.trim_start_matches('/');
                if !trimmed.is_empty() && !paths.contains(&trimmed.to_string()) {
                    normalized_extras.push(trimmed.to_string());
                }
            }
        }
    }
    paths.extend(normalized_extras);

    (paths, symbols)
}

/// File paths named by `*** Add|Update|Delete File:` lines in an apply_patch body.
fn patch_paths(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| {
            ["*** Add File:", "*** Update File:", "*** Delete File:"]
                .iter()
                .find_map(|p| l.strip_prefix(p))
        })
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Returns true if the tool modifies, overwrites, or deletes files or symbols.
fn is_mutation_tool(tool_name: &str) -> bool {
    let lower = tool_name.to_lowercase();
    lower.contains("edit")
        || lower.contains("write")
        || lower.contains("replace")
        || lower.contains("modify")
        || lower.contains("patch")
        || lower.contains("delete")
        || lower.contains("remove")
        || lower.contains("apply")
        || lower.contains("save")
}
