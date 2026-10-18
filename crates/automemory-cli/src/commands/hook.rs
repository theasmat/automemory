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

// ── Entry point: always answers in the agent's own schema ────────────────────────────
// A hook that prints nothing, prints a second JSON object, or exits non-zero is an error to
// the agent (Antigravity even rejects unknown fields), so every path ends with exactly one
// valid reply, and the same reply for the same event arriving twice.

static EMITTED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

macro_rules! emit {
    ($($arg:tt)*) => { emit_line(format!($($arg)*)) };
}

fn emit_line(line: String) {
    println!("{line}");
    if let Ok(mut out) = EMITTED.lock() {
        out.push(line);
    }
}

fn emitted_output() -> Option<String> {
    EMITTED.lock().ok().filter(|o| !o.is_empty()).map(|o| o.join("\n"))
}

/// True when the payload carries an id unique to this step (Claude/Codex `tool_use_id`,
/// Antigravity `stepIdx`/`invocationNum`/`executionNum`, Codex `turn_id`).
fn has_unique_step_id(input: &str) -> bool {
    serde_json::from_str::<Value>(input).is_ok_and(|v| {
        ["tool_use_id", "toolUseId", "stepIdx", "invocationNum", "executionNum", "turn_id"]
            .iter()
            .any(|k| v.get(k).is_some())
    })
}

/// What to say when the hook itself failed: let the action proceed, in a valid shape.
pub(crate) fn fail_open_output(agent: &str, event: &str) -> Value {
    let gate = matches!(
        CanonicalEvent::from_cli(event),
        Some(CanonicalEvent::PreToolUse | CanonicalEvent::PreCommit | CanonicalEvent::Stop)
    );
    if gate && agy_style(agent) {
        json!({"decision": "allow"})
    } else if gate && !native_hook_style(agent) && event == "pre-tool" {
        json!({"decision": "allow"})
    } else {
        json!({})
    }
}

/// Entry point for `automemory hook`. Never fails: panics and errors become a fail-open reply
/// on stdout (the error goes to stderr), and a duplicate registration of the same hook
/// (e.g. global and workspace) replays the first reply instead of acting twice.
pub fn run(agent: &str, event: &str, project_arg: &str) {
    let mut input = String::new();
    if !io::stdin().is_terminal() {
        let _ = io::stdin().read_to_string(&mut input);
    }
    let db = Config::default_config();
    let db = Database::open(&db.db_path).ok();
    let key = automemory_core::locking::hook_key(agent, event, &input);
    // Only payloads that identify one specific step can be told apart from a genuine repeat
    // (an agent retrying the same edit a moment later must get a fresh decision).
    let claimable = has_unique_step_id(&input);
    if let (Some(db), true) = (&db, claimable) {
        if let Some(cached) = db.replay_or_claim_hook(&key) {
            println!("{cached}");
            return;
        }
    }
    let outcome = std::panic::catch_unwind(|| execute_with_input(agent, event, project_arg, &input));
    match outcome {
        Ok(Ok(())) => {}
        Ok(Err(e)) => eprintln!("[AutoMemory] hook error (failing open): {e:#}"),
        Err(_) => eprintln!("[AutoMemory] hook panicked (failing open)"),
    }
    if emitted_output().is_none() {
        emit!("{}", fail_open_output(agent, event));
    }
    if let (Some(db), Some(out), true) = (&db, emitted_output(), claimable) {
        db.finish_hook(&key, &out);
    }
}


/// Evaluates guardrails on `pre-tool` events and records append-only event traces.
pub fn execute_with_input(agent: &str, event: &str, project_arg: &str, input: &str) -> Result<()> {
    let raw_payload: Value = serde_json::from_str(input).unwrap_or_else(|_| json!({}));

    // Quick open database; gracefully fail-open if DB is unreachable
    let config = Config::default_config();
    let db = match Database::open(&config.db_path) {
        Ok(d) => d,
        Err(_) => {
            emit!("{}", fail_open_output(agent, event));
            return Ok(());
        }
    };

    // Project resolution & autopilot mode lookup
    let early_canonical = CanonicalEvent::from_cli(event);
    // On a fresh machine the first prompt or session start is where a repository this machine has
    // not seen is matched to the project synced from another one (by git remote), once.
    let attach = matches!(
        early_canonical,
        Some(CanonicalEvent::SessionStart | CanonicalEvent::UserPromptSubmit)
    );
    let project_ref = resolve_project(&db, project_arg, &raw_payload, attach);
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

    // A registered project learns its repository key once (a git call only while it has none).
    if let (Some(pid), true) = (
        effective_project_id,
        matches!(
            early_canonical,
            Some(
                CanonicalEvent::SessionStart
                    | CanonicalEvent::UserPromptSubmit
                    | CanonicalEvent::Stop
                    | CanonicalEvent::SessionEnd
            )
        ),
    ) {
        if db.project_key_of(pid).is_none() {
            let dir = if project_ref.root_path.is_empty() {
                std::env::current_dir().unwrap_or_default()
            } else {
                PathBuf::from(&project_ref.root_path)
            };
            if let Some(key) = automemory_core::sync::identity::project_key_for_dir(&dir) {
                let _ = db.set_project_key(pid, &key);
            }
        }
    }

    // ── Boot / Pre-Invocation Context Injection ────────────────────────────────
    // Gated off by default: PRD pull-principle (§5) reserves context for
    // explicit queries; enable with [defaults] boot_injection = true.
    let canonical = CanonicalEvent::from_cli(event);
    let lock_cfg = GlobalConfig::load().map(|g| g.locks).unwrap_or_default();
    let known_session = session_id != "unknown-session";
    let command_text: Option<String> = tool_info
        .as_ref()
        .filter(|t| t.category == "command")
        .and_then(|t| t.input.as_ref())
        .and_then(|i| i.get("command").or_else(|| i.get("CommandLine")))
        .and_then(|v| v.as_str())
        .map(String::from);
    // Any activity keeps the session's leases alive; they lapse when it goes quiet.
    if known_session
        && matches!(
            canonical,
            Some(
                CanonicalEvent::SessionStart
                    | CanonicalEvent::PreToolUse
                    | CanonicalEvent::PostToolUse
                    | CanonicalEvent::UserPromptSubmit
                    | CanonicalEvent::SubagentStart
            )
        )
    {
        let _ = db.heartbeat_lease(&session_id, None);
    }
    if canonical == Some(CanonicalEvent::SessionStart) {
        let boot_on = GlobalConfig::load()
            .map(|g| g.defaults.boot_injection)
            .unwrap_or(false);
        // Antigravity's PreInvocation runs before every model step and takes `injectSteps`
        // (its other output fields are rejected), so it carries waiting notes, the files
        // other agents hold, and the boot context on the first step only.
        spawn_background_sync(&db);
        if agy_style(agent) {
            let mut parts = db.drain_notes(&session_id);
            parts.extend(others_leases_note(&db, effective_project_id, &session_id));
            let first_step = raw_payload.get("invocationNum").and_then(|v| v.as_i64()) == Some(0);
            if first_step {
                parts.extend(resume_once(&db, effective_project_id, &session_id, &dir_of(&project_ref.root_path)));
            }
            if boot_on && first_step {
                let ctx = automemory_core::retrieval::RetrievalEngine::keyword_only(&db)
                    .assemble_context(effective_project_id, None, 2000)
                    .unwrap_or_default();
                if !ctx.trim().is_empty() {
                    parts.push(ctx);
                }
            }
            emit!("{}", agy_injection(&parts));
            return Ok(());
        }
        if !boot_on {
            emit!("{{}}");
            return Ok(());
        }
        let engine = automemory_core::retrieval::RetrievalEngine::keyword_only(&db);
        let ctx = engine
            .assemble_context(effective_project_id, None, 2000)
            .unwrap_or_default();
        if !ctx.trim().is_empty() {
            if native_hook_style(agent) {
                emit!("{}", native_context("SessionStart", &ctx));
            } else {
                emit!(
                    "{}",
                    json!({
                        "context": ctx,
                        "additionalContext": ctx,
                        "prompt": ctx,
                    })
                );
            }
        } else {
            emit!("{{}}");
        }
        return Ok(());
    }

    // ── Pending-work nudge: only speaks when something is assigned ─────────
    if canonical == Some(CanonicalEvent::UserPromptSubmit) {
        spawn_background_sync(&db);
        let parts: Vec<String> = [
            resume_once(&db, effective_project_id, &session_id, &dir_of(&project_ref.root_path)),
            pending_work_nudge(&db, effective_project_id, &session_id),
            others_leases_note(&db, effective_project_id, &session_id),
        ]
        .into_iter()
        .flatten()
        .collect();
        if parts.is_empty() {
            emit!("{{}}");
        } else if native_hook_style(agent) {
            emit!("{}", native_context("UserPromptSubmit", &parts.join(" ")));
        } else {
            emit!("{}", json!({ "nudge": parts.join(" ") }));
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
        // A shell command is leased on the files it will write, before it runs.
        let shell_targets = command_text
            .as_deref()
            .map(automemory_core::locking::shell_write_targets)
            .unwrap_or_default();
        for t in &shell_targets {
            if !paths.contains(t) {
                paths.push(t.clone());
            }
        }
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
            is_mutation: gate_event == CanonicalEvent::PreCommit
                || is_mutation_tool(tool_name)
                || !shell_targets.is_empty(),
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
            emit!(
                "{}",
                native_gate_output(gate_event, merged.decision, &merged.reason)
            );
        } else {
            // Antigravity accepts allow / ask / deny (not "warn"); a reason rides along.
            let decision = match merged.decision {
                GateDecision::Warn => GateDecision::Allow,
                d => d,
            };
            if merged.reason.is_empty() {
                emit!("{}", json!({"decision": decision.as_str()}));
            } else {
                emit!(
                    "{}",
                    json!({"decision": decision.as_str(), "reason": merged.reason})
                );
            }
        }
        if merged.decision == GateDecision::Deny {
            return Ok(());
        }
    }

    // ── Record Event ──────────────────────────────────────────────────────────
    let project_root = project_ref.root_path.clone();
    let root_dir = || {
        if project_root.is_empty() {
            std::env::current_dir().unwrap_or_default()
        } else {
            PathBuf::from(&project_root)
        }
    };
    // Before a shell command: remember what git sees, to find what the command changes.
    if canonical == Some(CanonicalEvent::PreToolUse)
        && command_text.is_some()
        && known_session
        && lock_cfg.reconcile
    {
        if let Some(snap) = automemory_core::locking::snapshot(&root_dir()) {
            let _ = db.begin_snapshot(&session_id, effective_project_id, &snap);
        }
    }
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

    // After a shell command: lease the files it changed that nothing covered, and tell the
    // agent when one of them is held by someone else.
    let mut post_note: Option<String> = None;
    if canonical == Some(CanonicalEvent::PostToolUse)
        && command_text.is_some()
        && known_session
        && lock_cfg.reconcile
    {
        if let (Ok(Some(before)), Some(after)) = (
            db.finish_snapshot(&session_id),
            automemory_core::locking::snapshot(&root_dir()),
        ) {
            let r = automemory_core::locking::reconcile(
                &db,
                &lock_cfg,
                effective_project_id,
                &session_id,
                agent,
                &before,
                &after,
            );
            post_note = lock_conflict_note(&r.conflicts);
        }
    }

    // The session is over: its files are free for other agents at once.
    if canonical == Some(CanonicalEvent::SessionEnd) {
        maybe_handoff(&db, effective_project_id, &session_id, agent, &root_dir());
        spawn_background_sync(&db);
        let _ = db.release_leases(&session_id, None);
        db.drop_snapshot(&session_id);
        emit!("{{}}");
        return Ok(());
    }

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
        if canonical == Some(CanonicalEvent::Stop) {
            save_session_checkpoint(&db, &session_id);
        }
    }

    // Auto-rescan and update document intelligence if any plan/spec/doc was touched
    auto_rescan_doc_if_applicable(
        &db,
        &project_root,
        effective_project_id,
        normalized_event.tool.as_ref(),
        &raw_payload,
    );

    // Stop is blocked when work is owed, in which case the stop is blocked (PRD §7.2) and the human is
    // told why through the Notifier port.
    if canonical == Some(CanonicalEvent::Stop) {
        if let Some(owed) = pending_work_nudge(&db, effective_project_id, &session_id) {
            active_notifier().notify("stop blocked", &owed);
            if agy_style(agent) {
                // Antigravity: "continue" keeps the agent running, anything else lets it stop.
                emit!("{}", json!({"decision": "continue", "reason": owed}));
            } else {
                emit!("{}", json!({"decision": "block", "reason": owed}));
            }
            return Ok(());
        }
        // Stop fires after every agent turn, so leases stay (renewed by activity, expiring
        // when idle); SessionEnd releases them. Only the in-flight marker is cleared.
        let _ = db.finish_snapshot(&session_id);
        // Antigravity has no SessionEnd. A Stop with nothing left running means the agent has
        // finished, so its files are free; the next edit takes them again.
        if agy_style(agent) && raw_payload.get("fullyIdle").and_then(|v| v.as_bool()) == Some(true) {
            maybe_handoff(&db, effective_project_id, &session_id, agent, &root_dir());
            spawn_background_sync(&db);
            let _ = db.release_leases(&session_id, None);
        }
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
        match post_note {
            Some(n) if native_hook_style(agent) => emit!("{}", native_context("PostToolUse", &n)),
            // Antigravity's PostToolUse output must be {}: keep the note for its next PreInvocation.
            Some(n) => {
                db.push_note(&session_id, &n);
                emit!("{{}}");
            }
            None if agy_style(agent) && canonical == Some(CanonicalEvent::Stop) => {
                emit!("{}", json!({"decision": "allow"}))
            }
            None => emit!("{{}}"),
        }
    }
    Ok(())
}

fn dir_of(root: &str) -> PathBuf {
    if root.is_empty() {
        std::env::current_dir().unwrap_or_default()
    } else {
        PathBuf::from(root)
    }
}

/// Start `automemory sync auto` detached (a hook never waits for the network). Throttled, and
/// only when a git remote is configured.
fn spawn_background_sync(db: &Database) {
    if !automemory_core::sync::auto::should_spawn(db) {
        return;
    }
    if let Ok(exe) = std::env::current_exe() {
        let _ = Command::new(exe)
            .args(["sync", "auto", "--pull", "--push"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}

/// The resume pack, once per session.
fn resume_once(db: &Database, project_id: Option<&str>, session_id: &str, cwd: &Path) -> Option<String> {
    if session_id == "unknown-session" {
        return None;
    }
    let key = format!("resume_shown:{session_id}");
    if db.sync_kv_get(&key).is_some() {
        return None;
    }
    db.sync_kv_set(&key, "1");
    // Keep instruction files that already carry our block current (never creates new ones).
    super::rules_export::refresh_existing(db, project_id, cwd);
    automemory_core::resume::build_resume(db, project_id, session_id, cwd)
}

/// A session that changed files leaves a hand-off for whoever continues (on any machine).
fn maybe_handoff(db: &Database, project_id: Option<&str>, session_id: &str, agent: &str, cwd: &Path) {
    if session_id == "unknown-session" {
        return;
    }
    let key = format!("handoff_at:{session_id}");
    let now = chrono::Utc::now().timestamp();
    let recent = db
        .sync_kv_get(&key)
        .and_then(|v| v.parse::<i64>().ok())
        .is_some_and(|t| now - t < 600);
    if recent {
        return;
    }
    let edited = automemory_core::extraction::TimeSpanDigestEngine::new(db)
        .generate_session_digest(session_id)
        .is_ok_and(|d| d.metrics.edits_count > 0);
    if edited && automemory_core::resume::build_handoff(db, project_id, session_id, agent, cwd, None).is_ok() {
        db.sync_kv_set(&key, &now.to_string());
    }
}

/// What the agent is told when a command changed files that another session holds.
fn lock_conflict_note(conflicts: &[automemory_core::models::LeaseConflict]) -> Option<String> {
    if conflicts.is_empty() {
        return None;
    }
    let items: Vec<String> = conflicts
        .iter()
        .take(5)
        .map(|c| {
            format!(
                "'{}' is leased by agent '{}' (goal: '{}', {}s left)",
                c.scope, c.holder_agent, c.goal, c.remaining_seconds
            )
        })
        .collect();
    Some(format!(
        "Lock warning: that command changed files another agent holds: {}. Coordinate before changing them again.",
        items.join("; ")
    ))
}

/// Files other sessions hold right now, so the agent can avoid them before it starts.
fn others_leases_note(db: &Database, project_id: Option<&str>, session_id: &str) -> Option<String> {
    let leases = db.list_active_leases(project_id).ok()?;
    let others: Vec<String> = leases
        .iter()
        .filter(|l| l.session_id != session_id)
        .take(8)
        .map(|l| format!("{} ({}: {})", l.scope, l.agent, l.goal))
        .collect();
    if others.is_empty() {
        None
    } else {
        Some(format!(
            "Files other agents are editing right now: {}. Avoid them or coordinate first.",
            others.join("; ")
        ))
    }
}

/// Claude Code and Codex share one hook wire format (`hookSpecificOutput`);
/// Antigravity and the rest use the flat `{"decision": ...}` shape.
fn native_hook_style(agent: &str) -> bool {
    matches!(
        agent.to_ascii_lowercase().as_str(),
        "claude" | "claude-code" | "claudecode" | "codex" | "openai"
    )
}

/// Antigravity (`agy`) has its own hook schema, documented at antigravity.google/docs/hooks.
fn agy_style(agent: &str) -> bool {
    matches!(agent.to_ascii_lowercase().as_str(), "agy" | "antigravity")
}

/// PreInvocation output that adds a message to the model's next step.
fn agy_injection(parts: &[String]) -> Value {
    if parts.is_empty() {
        return json!({});
    }
    json!({ "injectSteps": [{ "ephemeralMessage": format!("[AutoMemory] {}", parts.join(" ")) }] })
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
    // Only a baton addressed to this very session is owed work. Broadcast hand-offs (what a
    // session end writes automatically) are for the resume pack, not for blocking a Stop.
    if db
        .latest_baton(project_id, Some(session_id))
        .ok()
        .flatten()
        .is_some_and(|b| b.to_session.as_deref() == Some(session_id))
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

/// Leaves a progress memory when a session that edited files stops, so the next
/// agent finds what happened without anyone calling `memory.digest` first. Facts come
/// from the ledger; read-only sessions leave nothing.
fn save_session_checkpoint(db: &Database, session_id: &str) {
    let engine = automemory_core::extraction::TimeSpanDigestEngine::new(db);
    let Ok(digest) = engine.generate_session_digest(session_id) else {
        return;
    };
    if digest.metrics.edits_count == 0 {
        return;
    }
    // Stop fires after every agent turn. A summary that is stable per session lets
    // save_memory update one row in place instead of adding one per turn.
    if let Ok(mut mem) = engine.synthesize_milestone_memory(&digest, Some("Session checkpoint")) {
        let short: String = session_id.chars().take(8).collect();
        mem.summary = format!("Session checkpoint {short}");
        // Where it happened, so another machine's resume pack can say so.
        mem.metadata = json!({
            "session": session_id,
            "device": automemory_core::resume::device_label(db),
        });
        let _ = db.save_memory(&mem);
    }
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
fn resolve_project(db: &Database, project_arg: &str, payload: &Value, attach: bool) -> ProjectRef {
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

    // 5. Same repository as a project synced from another machine: adopt it and remember this path.
    if attach {
        let dir = std::path::Path::new(&current_dir);
        if let Some(key) = automemory_core::sync::identity::project_key_for_dir(dir) {
            if let Some(project) = db
                .project_id_for_key(&key)
                .and_then(|pid| db.get_project_by_id(&pid).ok().flatten())
            {
                let root = canonical_root
                    .as_ref()
                    .map(|r| r.to_string_lossy().to_string())
                    .unwrap_or_else(|| current_dir.clone());
                let _ = db.add_project_path(&project.id, &root, false);
                return ProjectRef {
                    id: Some(project.id),
                    slug: Some(project.slug),
                    root_path: current_dir,
                    source_git_remote: None,
                };
            }
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

#[cfg(test)]
mod native_hook_tests {
    use super::*;

    #[test]
    fn claude_and_codex_get_hook_specific_output_others_keep_flat_shape() {
        assert!(native_hook_style("claude") && native_hook_style("Codex"));
        assert!(!native_hook_style("agy") && !native_hook_style("cursor"));

        let deny = native_gate_output(CanonicalEvent::PreToolUse, GateDecision::Deny, "nope");
        assert_eq!(deny["hookSpecificOutput"]["permissionDecision"], "deny");
        assert_eq!(
            deny["hookSpecificOutput"]["permissionDecisionReason"],
            "nope"
        );

        assert_eq!(
            native_gate_output(CanonicalEvent::PreToolUse, GateDecision::Allow, ""),
            json!({})
        );
        let warn = native_gate_output(CanonicalEvent::PreToolUse, GateDecision::Warn, "careful");
        assert!(warn["hookSpecificOutput"]
            .get("permissionDecision")
            .is_none());
        assert!(warn["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains("careful"));
        // PreCommit is reported to the agent as the PreToolUse it really is.
        let commit = native_gate_output(CanonicalEvent::PreCommit, GateDecision::Deny, "x");
        assert_eq!(commit["hookSpecificOutput"]["hookEventName"], "PreToolUse");
    }

    #[test]
    fn claude_and_codex_tool_payloads_normalize_to_canonical_tools() {
        let claude =
            json!({"session_id": "s", "tool_name": "Bash", "tool_input": {"command": "ls"}});
        let (tool, session, _) = normalize_payload("pre-tool", &claude);
        let tool = tool.unwrap();
        assert_eq!(
            (tool.name.as_str(), tool.category.as_str()),
            ("command", "command")
        );
        assert_eq!(session, "s");

        let edit =
            json!({"session_id": "s", "tool_name": "Edit", "tool_input": {"file_path": "a.rs"}});
        assert_eq!(normalize_payload("pre-tool", &edit).0.unwrap().name, "edit");

        // Codex passes argv arrays and `cmd`; guardrails need one string.
        let codex = json!({"session_id": "s", "tool_name": "shell", "tool_input": {"command": ["bash", "-lc", "ls -la"]}});
        let tool = normalize_payload("pre-tool", &codex).0.unwrap();
        assert_eq!(tool.input.unwrap()["command"], "bash -lc ls -la");
        let exec =
            json!({"session_id": "s", "tool_name": "exec_command", "tool_input": {"cmd": "pwd"}});
        assert_eq!(
            normalize_payload("pre-tool", &exec)
                .0
                .unwrap()
                .input
                .unwrap()["command"],
            "pwd"
        );
    }

    #[test]
    fn apply_patch_paths_are_extracted_for_lease_checks() {
        let patch = "*** Begin Patch\n*** Add File: src/new.rs\n+x\n*** Update File: src/old.rs\n*** Delete File: gone.rs\n*** End Patch";
        assert_eq!(patch_paths(patch), ["src/new.rs", "src/old.rs", "gone.rs"]);
        let (paths, _) = extract_target_paths_and_symbols("edit", &json!({ "input": patch }));
        assert!(paths.contains(&"src/old.rs".to_string()));
    }

    #[test]
    fn tool_response_is_recorded_as_output() {
        let raw = json!({"session_id": "s", "tool_name": "Bash", "tool_input": {"command": "ls"}, "tool_response": {"stdout": "ok"}});
        let tool = normalize_payload("post-tool", &raw).0.unwrap();
        assert_eq!(tool.output.unwrap()["stdout"], "ok");
    }

    #[test]
    fn antigravity_gets_only_fields_its_schema_accepts() {
        assert!(agy_style("agy") && !agy_style("claude"));
        assert_eq!(agy_injection(&[]), json!({}));
        let out = agy_injection(&["Files other agents are editing: a.rs".into()]);
        let msg = out["injectSteps"][0]["ephemeralMessage"].as_str().unwrap();
        assert!(msg.starts_with("[AutoMemory]") && msg.contains("a.rs"));
        // The only top-level key is injectSteps; Antigravity rejects unknown fields.
        assert_eq!(out.as_object().unwrap().keys().collect::<Vec<_>>(), ["injectSteps"]);
    }

    #[test]
    fn a_failed_hook_still_answers_in_the_agents_schema() {
        // Antigravity needs a decision on PreToolUse and Stop; everything else takes {}.
        assert_eq!(fail_open_output("agy", "pre-tool"), json!({"decision": "allow"}));
        assert_eq!(fail_open_output("agy", "stop"), json!({"decision": "allow"}));
        assert_eq!(fail_open_output("agy", "post-tool"), json!({}));
        assert_eq!(fail_open_output("agy", "boot"), json!({}));
        // Claude Code and Codex treat {} as "no opinion".
        assert_eq!(fail_open_output("claude", "pre-tool"), json!({}));
        assert_eq!(fail_open_output("codex", "stop"), json!({}));
    }

    #[test]
    fn only_payloads_with_a_step_id_are_deduplicated() {
        assert!(has_unique_step_id(r#"{"conversationId":"c","stepIdx":3}"#));
        assert!(has_unique_step_id(r#"{"session_id":"s","tool_use_id":"toolu_1"}"#));
        assert!(!has_unique_step_id(r#"{"session_id":"s","tool_name":"Edit"}"#));
        assert!(!has_unique_step_id("not json"));
    }
}
