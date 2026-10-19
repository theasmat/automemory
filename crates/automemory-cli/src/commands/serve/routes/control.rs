//! Dashboard controls for things the CLI already does: tools, agents, decisions, memory care, locks, settings, activity.
//! Tiers are listed in `serve/policy.rs`; Protected routes ask for the human password every time.

use crate::commands::serve::jobs;
use crate::commands::serve::routes::guardrails::require_human_header;
use crate::commands::serve::state::{AppState, SearchParams};
use automemory_core::config::GlobalConfig;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

fn bad(msg: impl Into<String>) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({ "error": msg.into() }))).into_response()
}

/// Protected action: asks for the password, then notes the action in the audit log.
fn protected(state: &AppState, headers: &HeaderMap, text: &str) -> Option<Response> {
    if let Err(e) = require_human_header(headers, text) {
        return Some(e.into_response());
    }
    let _ = state.db.log_audit("ui.action", "dashboard", text, &json!({}));
    None
}

/// The project a request is about and its folder: the id given, else the project of the daemon's folder,
/// else the newest real project.
fn project_dir(state: &AppState, project_id: Option<&str>) -> Option<(Option<String>, PathBuf)> {
    let by_id = project_id.filter(|p| !p.is_empty()).and_then(|id| {
        let pid = state.db.get_project_by_id(id).ok().flatten().or_else(|| state.db.get_project_by_slug(id).ok().flatten())?.id;
        let path: String = state.db.conn().query_row("SELECT local_path FROM project_paths WHERE project_id = ?1 ORDER BY is_primary DESC LIMIT 1", [&pid], |r| r.get(0)).ok()?;
        Some((Some(pid), PathBuf::from(path)))
    });
    by_id.or_else(|| {
        let daemon = std::env::current_dir().ok()?;
        if let Ok(Some(p)) = state.db.find_project_by_path(&daemon.to_string_lossy()) {
            return Some((Some(p.id), daemon));
        }
        let (pid, path): (String, String) = state
            .db
            .conn()
            .query_row("SELECT project_id, local_path FROM project_paths WHERE project_id != 'demo' ORDER BY created_at DESC LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .ok()?;
        Some((Some(pid), PathBuf::from(path)))
    })
}

/// Runs this same binary (so the dashboard does exactly what the CLI does) and returns its output.
fn run_self(args: &[&str], cwd: &std::path::Path) -> anyhow::Result<String> {
    let out = std::process::Command::new(std::env::current_exe()?).args(args).current_dir(cwd).output()?;
    let mut text = String::from_utf8_lossy(&out.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    let text = strip_ansi(&text);
    if out.status.success() { Ok(text.trim().to_string()) } else { Err(anyhow::anyhow!(text.trim().to_string())) }
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            for n in chars.by_ref() {
                if n.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn started(r: Result<String, String>) -> Response {
    match r {
        Ok(id) => Json(json!({ "started": true, "job": id })).into_response(),
        Err(e) => bad(e),
    }
}

// ── Memory care (Protected) ─────────────────────────────────────────────────────────
pub async fn api_memory_pin(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    if let Some(r) = protected(&state, &headers, "pin a memory") { return r; }
    match state.db.pin_memory(&id) { Ok(true) => Json(json!({ "success": true })).into_response(), Ok(false) => bad("memory not found"), Err(e) => bad(e.to_string()) }
}

#[derive(Deserialize, Default)]
pub struct ReasonPayload { pub reason: Option<String> }

pub async fn api_memory_veto(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, body: Option<Json<ReasonPayload>>) -> Response {
    if let Some(r) = protected(&state, &headers, "veto a memory") { return r; }
    let reason = body.and_then(|b| b.0.reason);
    match state.db.veto_memory(&id, reason.as_deref()) { Ok(true) => Json(json!({ "success": true })).into_response(), Ok(false) => bad("memory not found"), Err(e) => bad(e.to_string()) }
}

pub async fn api_memory_restore(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    if let Some(r) = protected(&state, &headers, "restore a memory") { return r; }
    match state.db.restore_memory(&id) { Ok(true) => Json(json!({ "success": true })).into_response(), Ok(false) => bad("memory not found"), Err(e) => bad(e.to_string()) }
}

pub async fn api_memory_revoke(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    if let Some(r) = protected(&state, &headers, "revoke a memory") { return r; }
    match state.db.revoke_memories(Some(&id), None, None) { Ok(n) if n > 0 => Json(json!({ "revoked": n })).into_response(), Ok(_) => bad("memory not found"), Err(e) => bad(e.to_string()) }
}

// ── Decisions ───────────────────────────────────────────────────────────────────────
pub async fn api_decisions(State(state): State<AppState>, Query(q): Query<SearchParams>) -> Response {
    Json(state.db.list_decisions(q.project_id.as_deref(), None, 200).unwrap_or_default()).into_response()
}

#[derive(Deserialize)]
pub struct DecisionPayload {
    pub topic: String,
    pub decision: String,
    pub reason: Option<String>,
    pub project_id: Option<String>,
    #[serde(default)]
    pub alternatives: Vec<String>,
}

/// Recording a decision on a topic supersedes the active one (the history is kept).
pub async fn api_decision_record(State(state): State<AppState>, headers: HeaderMap, Json(p): Json<DecisionPayload>) -> Response {
    if let Some(r) = protected(&state, &headers, "record a decision") { return r; }
    if p.topic.trim().is_empty() || p.decision.trim().is_empty() {
        return bad("topic and decision are required");
    }
    let project = project_dir(&state, p.project_id.as_deref()).and_then(|(id, _)| id);
    let reason = p.reason.unwrap_or_default();
    match state.db.record_decision(automemory_core::models::DecisionRecordArgs {
        project_id: project.as_deref(), topic: p.topic.trim(), decision: p.decision.trim(), reason: &reason,
        alternatives: &p.alternatives, dissent: None, agent: "human", symbols: &[], evidence: &[],
    }) {
        Ok(d) => Json(json!({ "success": true, "decision": d })).into_response(),
        Err(e) => bad(e.to_string()),
    }
}

// ── Tools (Safe) ────────────────────────────────────────────────────────────────────
pub async fn api_tool_doctor(State(state): State<AppState>) -> Response {
    let cwd = project_dir(&state, None).map(|(_, d)| d).unwrap_or_else(|| PathBuf::from("."));
    started(jobs::start(&state, "doctor", "Health check", move |_| run_self(&["doctor"], &cwd)))
}

pub async fn api_tool_eval(State(state): State<AppState>) -> Response {
    let cwd = project_dir(&state, None).map(|(_, d)| d).unwrap_or_else(|| PathBuf::from("."));
    started(jobs::start(&state, "eval", "Eval suites", move |_| run_self(&["eval"], &cwd)))
}

pub async fn api_tool_docs_rescan(State(state): State<AppState>) -> Response {
    let Some((_, cwd)) = project_dir(&state, None) else { return bad("no project yet") };
    started(jobs::start(&state, "docs", "Rescan documents", move |_| run_self(&["doc", "scan"], &cwd)))
}

pub async fn api_tool_graph_index(State(state): State<AppState>) -> Response {
    let Some((Some(pid), dir)) = project_dir(&state, None) else { return bad("no project yet") };
    let db = Arc::clone(&state.db);
    started(jobs::start(&state, "graph", "Index the code graph", move |_| {
        let (entities, edges) = automemory_core::graph::GraphBuilder::new(&db).scan_and_index_project(&pid, &dir.to_string_lossy())?;
        Ok(format!("{entities} entities and {edges} relations indexed"))
    }))
}

pub async fn api_tool_backup(State(state): State<AppState>) -> Response {
    let db = Arc::clone(&state.db);
    started(jobs::start(&state, "backup", "Back up the database", move |_| {
        let dir = automemory_core::Config::default_config().home_dir.join("backups");
        std::fs::create_dir_all(&dir)?;
        let dest = dir.join(format!("automemory-{}.db", chrono::Utc::now().format("%Y%m%d-%H%M%S")));
        db.backup_to(&dest)?;
        // Safe to repeat, so cap the disk it can use: keep the newest 10.
        let mut old: Vec<_> = std::fs::read_dir(&dir)?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "db")).collect();
        old.sort();
        for p in old.iter().rev().skip(10) { let _ = std::fs::remove_file(p); }
        Ok(format!("Saved {}", dest.display()))
    }))
}

pub async fn api_sync_pull(State(state): State<AppState>) -> Response {
    let cwd = project_dir(&state, None).map(|(_, d)| d).unwrap_or_else(|| PathBuf::from("."));
    started(jobs::start(&state, "sync-pull", "Pull from the remote", move |_| run_self(&["sync", "auto", "--pull"], &cwd)))
}

#[derive(Deserialize, Default)]
pub struct HandoffPayload { pub note: Option<String>, pub project_id: Option<String> }

pub async fn api_tool_handoff(State(state): State<AppState>, body: Option<Json<HandoffPayload>>) -> Response {
    let p = body.map(|b| b.0).unwrap_or_default();
    let Some((pid, dir)) = project_dir(&state, p.project_id.as_deref()) else { return bad("no project yet") };
    let session = format!("dashboard-{}", chrono::Utc::now().timestamp());
    match automemory_core::resume::build_handoff(&state.db, pid.as_deref(), &session, "dashboard", &dir, p.note.as_deref()) {
        Ok(b) => Json(json!({ "success": true, "packet": b.packet })).into_response(),
        Err(e) => bad(e.to_string()),
    }
}

pub async fn api_export_preview(State(state): State<AppState>, Query(q): Query<SearchParams>) -> Response {
    let Some((pid, dir)) = project_dir(&state, q.project_id.as_deref()) else { return bad("no project yet") };
    let root = crate::commands::rules_export::repo_root(&dir);
    Json(crate::commands::rules_export::plan(&state.db, &root, pid.as_deref(), false)).into_response()
}

pub async fn api_export_apply(State(state): State<AppState>, Query(q): Query<SearchParams>) -> Response {
    let Some((pid, dir)) = project_dir(&state, q.project_id.as_deref()) else { return bad("no project yet") };
    let root = crate::commands::rules_export::repo_root(&dir);
    Json(crate::commands::rules_export::plan(&state.db, &root, pid.as_deref(), true)).into_response()
}

// ── Agents (Protected to change) ────────────────────────────────────────────────────
pub async fn api_agents_setup(State(state): State<AppState>) -> Response {
    let Some((_, dir)) = project_dir(&state, None) else { return Json(json!({ "project": null, "agents": [] })).into_response() };
    let home = dirs::home_dir().unwrap_or_default();
    let found = tokio::task::spawn_blocking({
        let dir = dir.clone();
        move || crate::commands::disconnect::inventory(&home, &dir)
    })
    .await
    .unwrap_or_default();
    let agents: Vec<Value> = crate::commands::adapter::ADAPTERS
        .iter()
        .map(|a| {
            let here = found.iter().filter(|l| l.starts_with(&format!("{} [workspace]", a.id))).count();
            json!({ "id": a.id, "native_hooks": !a.events.is_empty(), "registered_here": here > 0, "items": here })
        })
        .collect();
    Json(json!({ "project": dir.to_string_lossy(), "agents": agents })).into_response()
}

#[derive(Deserialize)]
pub struct AgentPayload { pub agent: String, pub dry_run: Option<bool> }

fn agent_ok(agent: &str) -> bool {
    crate::commands::adapter::find_adapter(agent).is_some()
}

pub async fn api_agent_connect(State(state): State<AppState>, headers: HeaderMap, Json(p): Json<AgentPayload>) -> Response {
    if let Some(r) = protected(&state, &headers, &format!("connect {}", p.agent)) { return r; }
    if !agent_ok(&p.agent) { return bad("unknown agent"); }
    let Some((_, dir)) = project_dir(&state, None) else { return bad("no project yet") };
    match tokio::task::spawn_blocking(move || run_self(&["connect", &p.agent], &dir)).await {
        Ok(Ok(out)) => Json(json!({ "success": true, "output": out })).into_response(),
        Ok(Err(e)) => bad(e.to_string()),
        Err(e) => bad(e.to_string()),
    }
}

pub async fn api_agent_disconnect(State(state): State<AppState>, headers: HeaderMap, Json(p): Json<AgentPayload>) -> Response {
    let dry = p.dry_run.unwrap_or(false);
    if !dry {
        if let Some(r) = protected(&state, &headers, &format!("disconnect {}", p.agent)) { return r; }
    }
    if !agent_ok(&p.agent) { return bad("unknown agent"); }
    let Some((_, dir)) = project_dir(&state, None) else { return bad("no project yet") };
    let mut args = vec!["disconnect", p.agent.as_str()];
    if dry { args.push("--dry-run"); }
    let args: Vec<String> = args.into_iter().map(String::from).collect();
    match tokio::task::spawn_blocking(move || run_self(&args.iter().map(String::as_str).collect::<Vec<_>>(), &dir)).await {
        Ok(Ok(out)) => Json(json!({ "success": true, "output": out })).into_response(),
        Ok(Err(e)) => bad(e.to_string()),
        Err(e) => bad(e.to_string()),
    }
}

// ── Locks (Protected) ───────────────────────────────────────────────────────────────
#[derive(Deserialize)]
pub struct ScopePayload { pub scope: String, pub project_id: Option<String> }

pub async fn api_lock_release(State(state): State<AppState>, headers: HeaderMap, Json(p): Json<ScopePayload>) -> Response {
    if let Some(r) = protected(&state, &headers, "release a lock") { return r; }
    match state.db.unlock_scope(p.project_id.as_deref(), &p.scope) { Ok(n) => Json(json!({ "released": n })).into_response(), Err(e) => bad(e.to_string()) }
}

pub async fn api_lock_stop(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(r) = protected(&state, &headers, "release every lock") { return r; }
    match state.db.release_all_leases(None) { Ok(n) => Json(json!({ "released": n })).into_response(), Err(e) => bad(e.to_string()) }
}

// ── Settings and activity ───────────────────────────────────────────────────────────
pub async fn api_settings_locks_get() -> Response {
    Json(GlobalConfig::load().unwrap_or_default().locks).into_response()
}

#[derive(Deserialize)]
pub struct LockSettingsPayload {
    pub ignore: Option<Vec<String>>,
    pub idle_ttl_seconds: Option<i64>,
    pub reconcile: Option<bool>,
}

pub async fn api_settings_locks_put(State(state): State<AppState>, headers: HeaderMap, Json(p): Json<LockSettingsPayload>) -> Response {
    if let Some(r) = protected(&state, &headers, "change lock settings") { return r; }
    let mut cfg = GlobalConfig::load().unwrap_or_default();
    if let Some(v) = p.ignore { cfg.locks.ignore = v.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(); }
    if let Some(t) = p.idle_ttl_seconds {
        if !(30..=86_400).contains(&t) { return bad("idle time must be between 30 seconds and 24 hours"); }
        cfg.locks.idle_ttl_seconds = t;
    }
    if let Some(r) = p.reconcile { cfg.locks.reconcile = r; }
    match cfg.save() { Ok(()) => Json(json!({ "success": true })).into_response(), Err(e) => bad(e.to_string()) }
}

/// Recent blocks and dashboard actions, newest first.
pub async fn api_activity(State(state): State<AppState>) -> Response {
    let conn = state.db.conn();
    let Ok(mut stmt) = conn.prepare("SELECT action, entity_type, entity_id, details, timestamp FROM audit_log ORDER BY timestamp DESC LIMIT 100") else {
        return Json(json!([])).into_response();
    };
    let rows: Vec<Value> = stmt
        .query_map([], |r| {
            let details: String = r.get(3)?;
            Ok(json!({ "action": r.get::<_, String>(0)?, "kind": r.get::<_, String>(1)?, "target": r.get::<_, String>(2)?,
                       "details": serde_json::from_str::<Value>(&details).unwrap_or(Value::Null), "at": r.get::<_, String>(4)? }))
        })
        .map(|it| it.flatten().collect())
        .unwrap_or_default();
    Json(rows).into_response()
}

// ── Memory hygiene ──────────────────────────────────────────────────────────────────
#[derive(Deserialize, Default)]
pub struct HygieneQuery { pub project_id: Option<String> }

/// GET /api/memories/hygiene: every active memory with a reason to look at it (never recalled, unused for 30 days, low confidence).
pub async fn api_memory_hygiene(State(state): State<AppState>, axum::extract::Query(q): axum::extract::Query<HygieneQuery>) -> Response {
    let now = chrono::Utc::now();
    let days = |s: &str| chrono::DateTime::parse_from_rfc3339(s).map(|t| (now - t.with_timezone(&chrono::Utc)).num_days()).unwrap_or(0);
    let rows = match state.db.hygiene_rows(q.project_id.as_deref()) { Ok(r) => r, Err(e) => return bad(e.to_string()) };
    let items: Vec<_> = rows.into_iter().filter_map(|(id, summary, kind, confidence, created, recalled)| {
        let age = days(&created);
        let mut flags = Vec::new();
        match recalled.as_deref() {
            None if age >= 7 => flags.push("never_recalled"),
            Some(r) if days(r) >= 30 => flags.push("unused_30d"),
            _ => {}
        }
        if confidence < 0.4 { flags.push("low_confidence"); }
        (!flags.is_empty()).then(|| json!({ "id": id, "summary": summary, "type": kind, "confidence": confidence, "age_days": age, "last_recalled_at": recalled, "flags": flags }))
    }).collect();
    Json(json!({ "count": items.len(), "items": items })).into_response()
}

#[derive(Deserialize)]
pub struct IdsPayload { pub ids: Vec<String> }

/// POST /api/memories/archive_many: archives several memories at once (each can be restored).
pub async fn api_memory_archive_many(State(state): State<AppState>, headers: HeaderMap, Json(p): Json<IdsPayload>) -> Response {
    if let Some(r) = protected(&state, &headers, &format!("archive {} memories", p.ids.len())) { return r; }
    if p.ids.len() > 500 { return bad("at most 500 at a time"); }
    let mut done = Vec::new();
    for id in &p.ids {
        if matches!(state.db.archive_memory(id), Ok(true)) { done.push(id.clone()); }
    }
    Json(json!({ "archived": done })).into_response()
}
