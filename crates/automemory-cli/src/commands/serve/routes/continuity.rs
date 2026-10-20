//! Continuity page: what the next agent will be handed, which agents' chats are read, imported history and search.
//! Reading, importing from an agent a person already turned on, and writing a hand-off are Safe; turning an agent on or off,
//! editing the brief and deleting history are Protected.
use super::control::{bad, project_dir_pub, protected};
use crate::commands::serve::jobs;
use crate::commands::serve::state::AppState;
use automemory_core::config::GlobalConfig;
use automemory_core::continuity::{deliver, import, readers, ItemKind, ItemStatus, NewItem, Origin};
use axum::{extract::{Path, Query, State}, http::HeaderMap, response::{IntoResponse, Response}, Json};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize, Default)]
pub struct ProjectQuery { pub project_id: Option<String> }

fn root_of(state: &AppState, project: Option<&str>) -> Option<(Option<String>, std::path::PathBuf)> {
    project_dir_pub(state, project)
}

fn tokens_budget() -> usize {
    GlobalConfig::load().map(|g| g.continuity.inject_tokens).unwrap_or(deliver::DEFAULT_INJECT_TOKENS)
}

/// GET /api/continuity: the brief, what a new agent would receive, and the chat sources.
pub async fn api_overview(State(state): State<AppState>, Query(q): Query<ProjectQuery>) -> Response {
    let Some((pid, root)) = root_of(&state, q.project_id.as_deref()) else { return Json(json!({ "project": null })).into_response() };
    let cfg = GlobalConfig::load().unwrap_or_default();
    let home = dirs::home_dir().unwrap_or_default();
    let items = state.db.brief_list(pid.as_deref(), false).unwrap_or_default();
    let label = |i: &automemory_core::continuity::BriefItem| deliver::label(&state.db, i);
    let items: Vec<Value> = items.iter().map(|i| json!({
        "id": i.id, "kind": i.kind, "heading": i.kind.heading(), "text": i.text, "reason": i.reason, "origin": i.origin, "author": i.author,
        "state": i.state, "label": label(i), "updated_at": i.updated_at,
    })).collect();
    let preview = deliver::render_core(&state.db, pid.as_deref(), tokens_budget());
    let sources: Vec<Value> = readers::registry().iter().map(|r| {
        let stored = state.db.chat_sessions(pid.as_deref(), Some(r.id())).unwrap_or_default();
        let last = stored.iter().filter_map(|s| s.last_at.clone()).max();
        json!({
            "id": r.id(), "label": r.label(), "enabled": cfg.continuity.read_chats.iter().any(|a| a == r.id()),
            "locations": r.locations(&home).iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
            "found": r.sessions(&home, &root).len(), "sessions": stored.len(), "messages": stored.iter().map(|s| s.messages).sum::<i64>(), "last_message_at": last,
        })
    }).collect();
    Json(json!({
        "project": pid, "root": root.to_string_lossy(), "items": items, "kinds": ItemKind::ALL.iter().map(|k| json!({ "id": k.as_str(), "heading": k.heading() })).collect::<Vec<_>>(),
        "preview": preview, "preview_tokens": preview.as_ref().map(|p| p.len() / 4).unwrap_or(0), "inject_tokens": tokens_budget(), "sources": sources,
    })).into_response()
}

#[derive(Deserialize)]
pub struct SearchQuery { pub q: String, pub agent: Option<String>, pub limit: Option<usize>, pub project_id: Option<String> }

pub async fn api_search(State(state): State<AppState>, Query(q): Query<SearchQuery>) -> Response {
    let pid = root_of(&state, q.project_id.as_deref()).and_then(|(p, _)| p);
    match state.db.chat_search(&q.q, pid.as_deref(), q.agent.as_deref(), q.limit.unwrap_or(10).min(30)) {
        Ok(hits) => Json(json!({ "hits": hits })).into_response(),
        Err(e) => bad(e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct ChatQuery { pub session: String, pub from: Option<i64>, pub limit: Option<usize> }

pub async fn api_chat(State(state): State<AppState>, Query(q): Query<ChatQuery>) -> Response {
    match (state.db.chat_session(&q.session), state.db.chat_get(&q.session, q.from.unwrap_or(0).max(0), q.limit.unwrap_or(20).min(60))) {
        (Ok(Some(s)), Ok(messages)) => Json(json!({ "session": s, "messages": messages })).into_response(),
        (Ok(None), _) => bad("no such session"),
        (Err(e), _) | (_, Err(e)) => bad(e.to_string()),
    }
}

#[derive(Deserialize, Default)]
pub struct ImportPayload { pub agent: Option<String>, pub project_id: Option<String> }

/// Imports new messages for every enabled agent (or one) into the archive. Only agents a person turned on are read.
fn import_enabled(state: &AppState, pid: Option<&str>, root: &std::path::Path, only: Option<&str>) -> Result<Vec<Value>, String> {
    let cfg = GlobalConfig::load().map_err(|e| e.to_string())?;
    let home = dirs::home_dir().unwrap_or_default();
    let agents: Vec<String> = match only {
        Some(a) if !cfg.continuity.read_chats.iter().any(|x| x == a) => return Err(format!("{a} is off: turn it on first")),
        Some(a) => vec![a.to_string()],
        None => cfg.continuity.read_chats.clone(),
    };
    let mut out = Vec::new();
    for a in agents {
        let Some(r) = readers::reader(&a) else { continue };
        let rep = import::import_project(&state.db, r.as_ref(), &home, pid, root).map_err(|e| e.to_string())?;
        out.push(json!({ "agent": a, "label": r.label(), "sessions": rep.sessions, "new_messages": rep.new_messages }));
    }
    Ok(out)
}

/// POST /api/continuity/import: a background job, so the Jobs tray shows it.
pub async fn api_import(State(state): State<AppState>, body: Option<Json<ImportPayload>>) -> Response {
    let p = body.map(|b| b.0).unwrap_or_default();
    let Some((pid, root)) = root_of(&state, p.project_id.as_deref()) else { return bad("no project yet") };
    let cfg = GlobalConfig::load().unwrap_or_default();
    if cfg.continuity.read_chats.is_empty() {
        return bad("no agent is turned on");
    }
    if let Some(a) = p.agent.as_deref() {
        if !cfg.continuity.read_chats.iter().any(|x| x == a) {
            return bad(format!("{a} is off: turn it on first"));
        }
    }
    let st = state.clone();
    match jobs::start(&state, "chat-import", "Importing chat history", move |_| {
        let rep = import_enabled(&st, pid.as_deref(), &root, p.agent.as_deref()).map_err(anyhow::Error::msg)?;
        Ok(rep.iter().map(|r| format!("{}: {} new message(s)", r["label"].as_str().unwrap_or(""), r["new_messages"])).collect::<Vec<_>>().join("; "))
    }) {
        Ok(id) => Json(json!({ "started": true, "job": id })).into_response(),
        Err(e) => bad(e),
    }
}

/// POST /api/continuity/handoff: imports what is new from the agents that are on, writes a hand-off, returns what the next agent gets.
pub async fn api_handoff(State(state): State<AppState>, body: Option<Json<ImportPayload>>) -> Response {
    let p = body.map(|b| b.0).unwrap_or_default();
    let Some((pid, root)) = root_of(&state, p.project_id.as_deref()) else { return bad("no project yet") };
    let imported = match import_enabled(&state, pid.as_deref(), &root, None) { Ok(v) => v, Err(e) => return bad(e) };
    let session = format!("dashboard-{}", chrono::Utc::now().timestamp());
    let packet = match automemory_core::resume::build_handoff(&state.db, pid.as_deref(), &session, "dashboard", &root, None) {
        Ok(b) => b.packet,
        Err(e) => return bad(e.to_string()),
    };
    Json(json!({ "success": true, "imported": imported, "packet": packet, "preview": deliver::render_core(&state.db, pid.as_deref(), tokens_budget()) })).into_response()
}

#[derive(Deserialize)]
pub struct SourcePayload { pub enabled: bool }

/// PUT /api/continuity/source/:agent (Protected): allow or stop reading one agent's chats.
pub async fn api_source(State(state): State<AppState>, headers: HeaderMap, Path(agent): Path<String>, Json(p): Json<SourcePayload>) -> Response {
    if readers::reader(&agent).is_none() {
        return bad("no reader for that agent");
    }
    if let Some(r) = protected(&state, &headers, &format!("{} reading {agent} chats", if p.enabled { "allow" } else { "stop" })) { return r; }
    let mut cfg = GlobalConfig::load().unwrap_or_default();
    cfg.continuity.read_chats.retain(|a| a != &agent);
    if p.enabled {
        cfg.continuity.read_chats.push(agent);
    }
    match cfg.save() { Ok(_) => Json(json!({ "success": true })).into_response(), Err(e) => bad(e.to_string()) }
}

#[derive(Deserialize, Default)]
pub struct ForgetQuery { pub agent: Option<String>, pub project_id: Option<String> }

/// DELETE /api/continuity/history (Protected): delete imported chat of this project (or one agent's).
pub async fn api_forget(State(state): State<AppState>, headers: HeaderMap, Query(q): Query<ForgetQuery>) -> Response {
    if let Some(r) = protected(&state, &headers, "delete imported chat history") { return r; }
    let pid = root_of(&state, q.project_id.as_deref()).and_then(|(p, _)| p);
    match state.db.chat_forget(pid.as_deref(), q.agent.as_deref()) { Ok(n) => Json(json!({ "success": true, "removed": n })).into_response(), Err(e) => bad(e.to_string()) }
}

#[derive(Deserialize)]
pub struct NewItemPayload { pub kind: String, pub text: String, pub reason: Option<String>, pub project_id: Option<String> }

/// POST /api/continuity/items (Protected): you add something to the brief.
pub async fn api_item_add(State(state): State<AppState>, headers: HeaderMap, Json(p): Json<NewItemPayload>) -> Response {
    if let Some(r) = protected(&state, &headers, "add to the brief") { return r; }
    let kind: ItemKind = match p.kind.parse() { Ok(k) => k, Err(e) => return bad(e) };
    let pid = root_of(&state, p.project_id.as_deref()).and_then(|(p, _)| p);
    match state.db.brief_add(NewItem { project_id: pid, kind, text: p.text, reason: p.reason, ref_id: None, origin: Origin::Human, author: "human".into(), session: None }) {
        Ok((item, created)) => Json(json!({ "success": true, "id": item.id, "created": created })).into_response(),
        Err(e) => bad(e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct EditPayload { pub text: String, pub reason: Option<String> }

/// PUT /api/continuity/items/:id (Protected): your edit; agents can no longer change the item.
pub async fn api_item_edit(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(p): Json<EditPayload>) -> Response {
    if let Some(r) = protected(&state, &headers, "edit the brief") { return r; }
    match state.db.brief_correct(&id, &p.text, p.reason.as_deref(), true, "human") { Ok(_) => Json(json!({ "success": true })).into_response(), Err(e) => bad(e.to_string()) }
}

/// POST /api/continuity/items/:id/:action (Protected): confirm, resolve or remove.
pub async fn api_item_action(State(state): State<AppState>, headers: HeaderMap, Path((id, action)): Path<(String, String)>) -> Response {
    if !matches!(action.as_str(), "confirm" | "resolve" | "remove") {
        return bad("unknown action");
    }
    if let Some(r) = protected(&state, &headers, &format!("{action} a brief item")) { return r; }
    let done = match action.as_str() {
        "confirm" => state.db.brief_confirm(&id).map(|_| true),
        "resolve" => state.db.brief_set_status(&id, ItemStatus::Resolved),
        _ => state.db.brief_set_status(&id, ItemStatus::Removed),
    };
    match done { Ok(true) => Json(json!({ "success": true })).into_response(), Ok(false) => bad("no such item"), Err(e) => bad(e.to_string()) }
}
