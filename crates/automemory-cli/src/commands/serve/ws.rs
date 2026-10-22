use super::state::AppState;
use automemory_core::extraction::TrajectoryDistiller;
use automemory_core::models::*;
use automemory_core::retrieval::RetrievalEngine;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_socket(socket, state))
}

pub async fn handle_ws_socket(mut socket: WebSocket, state: AppState) {
    let mut broadcast_rx = state.broadcast_tx.subscribe();

    loop {
        tokio::select! {
            // Outgoing broadcast message to this client
            recv_res = broadcast_rx.recv() => {
                match recv_res {
                    Ok(msg) => {
                        if socket.send(Message::Text(msg)).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // Client fell behind, ignore lagged frame
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
            // Incoming message from this client
            client_msg = socket.recv() => {
                match client_msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Some(resp) = handle_ws_json_rpc(&text, &state).await {
                            if socket.send(Message::Text(resp)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        if socket.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
}

#[derive(Deserialize)]
pub struct WsRequest {
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

pub async fn handle_ws_json_rpc(raw: &str, state: &AppState) -> Option<String> {
    let req: WsRequest = match serde_json::from_str(raw) {
        Ok(r) => r,
        Err(_) => return None,
    };

    let req_id = req.id.unwrap_or(Value::Null);

    let result = match req.method.as_str() {
        "ping" => json!({ "status": "pong", "timestamp": Utc::now().to_rfc3339() }),
        "stats" => rpc_stats(state),
        "events" => rpc_events(&req.params, state),
        "memories" => rpc_memories(&req.params, state),
        "sessions" => rpc_sessions(&req.params, state),
        "session_events" => rpc_session_events(&req.params, state),
        "distill_session" => rpc_distill_session(&req.params, state).await,
        "traces" => rpc_traces(&req.params, state),
        "failures" => rpc_failures(&req.params, state),
        "projects" => json!(state.db.list_projects().unwrap_or_default()),
        "preferences" => rpc_preferences(&req.params, state),
        "docs" => rpc_docs(&req.params, state),
        "proposals" => rpc_proposals(&req.params, state),
        "decide_proposal" => rpc_decide_proposal(&req.params, state),
        "skills" => rpc_skills(&req.params, state),
        "constitution" => rpc_constitution(&req.params, state),
        "assemble_context" => rpc_assemble_context(&req.params, state),
        "trigger_sync" => {
            state.sync_daemon.trigger();
            json!({ "status": "triggered", "message": "Automated git vault sync triggered" })
        }
        unknown => json!({ "error": format!("Unknown method: {}", unknown) }),
    };

    Some(
        json!({
            "id": req_id,
            "result": result,
        })
        .to_string(),
    )
}

fn rpc_stats(state: &AppState) -> Value {
    let (active_mem, archived_mem) = state.db.count_memories(None).unwrap_or((0, 0));
    let (mem_tokens, event_tokens) = state.db.get_token_stats(None).unwrap_or((0, 0));
    let traces = state.db.list_traces(None, 1000).unwrap_or_default();
    let failures = state
        .db
        .list_failures(None, Some("unresolved"), 1000)
        .unwrap_or_default();
    let projects = state.db.list_projects().unwrap_or_default();
    let proposals = state.db.list_pending_proposals(None).unwrap_or_default();
    json!({
        "active_memories": active_mem,
        "archived_memories": archived_mem,
        "total_memory_tokens": mem_tokens,
        "total_event_tokens": event_tokens,
        "tokens_saved_estimate": event_tokens.saturating_sub(mem_tokens),
        "active_traces": traces.len(),
        "unresolved_failures": failures.len(),
        "total_projects": projects.len(),
        "pending_proposals": proposals.len(),
    })
}

fn rpc_events(params: &Value, state: &AppState) -> Value {
    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
    let events = state.db.list_recent_events(limit).unwrap_or_default();
    json!(events)
}

fn rpc_memories(params: &Value, state: &AppState) -> Value {
    let project_id = params.get("project_id").and_then(|v| v.as_str());
    let query = params.get("q").and_then(|v| v.as_str());
    let scope_filter = params.get("scope").and_then(|v| v.as_str());
    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

    let mut memories = if let Some(q) = query {
        let results = state
            .db
            .search_memories_fts(q, project_id, limit)
            .unwrap_or_default();
        results.into_iter().map(|(m, _)| m).collect::<Vec<_>>()
    } else {
        state
            .db
            .list_memories(project_id, Some(MemoryStatus::Active), None, limit)
            .unwrap_or_default()
    };

    if let Some(scope) = scope_filter {
        if scope == "local" {
            memories.retain(|m| m.scope == MemoryScope::Project);
        } else if scope == "cloud" || scope == "global" {
            memories.retain(|m| m.scope == MemoryScope::Global);
        }
    }

    json!(memories)
}

fn rpc_sessions(params: &Value, state: &AppState) -> Value {
    let project_id = params.get("project_id").and_then(|v| v.as_str());
    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
    let sessions = state
        .db
        .list_sessions(project_id, limit)
        .unwrap_or_default();
    json!(sessions)
}

fn rpc_session_events(params: &Value, state: &AppState) -> Value {
    let session_id = params
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let events = state.db.get_session_events(session_id).unwrap_or_default();
    json!(events)
}

async fn rpc_distill_session(params: &Value, state: &AppState) -> Value {
    let session_id = params
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let distiller = TrajectoryDistiller::new(&state.db);
    match distiller.distill_session(session_id) {
        Ok(report) => {
            let _ = state.broadcast_tx.send(
                json!({
                    "type": "session_distilled",
                    "session_id": session_id,
                    "report": report,
                    "timestamp": Utc::now().to_rfc3339(),
                })
                .to_string(),
            );
            json!({ "success": true, "report": report })
        }
        Err(e) => json!({ "success": false, "error": e.to_string() }),
    }
}

fn rpc_traces(params: &Value, state: &AppState) -> Value {
    let project_id = params.get("project_id").and_then(|v| v.as_str());
    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
    let traces = state.db.list_traces(project_id, limit).unwrap_or_default();
    json!(traces)
}

fn rpc_failures(params: &Value, state: &AppState) -> Value {
    let project_id = params.get("project_id").and_then(|v| v.as_str());
    let status = params.get("status").and_then(|v| v.as_str());
    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
    let failures = state
        .db
        .list_failures(project_id, status, limit)
        .unwrap_or_default();
    json!(failures)
}

fn rpc_preferences(params: &Value, state: &AppState) -> Value {
    let project_id = params.get("project_id").and_then(|v| v.as_str());
    let prefs = state.db.list_preferences(project_id).unwrap_or_default();
    json!(prefs)
}

fn rpc_docs(params: &Value, state: &AppState) -> Value {
    let project_id = params.get("project_id").and_then(|v| v.as_str());
    if let Some(pid) = project_id {
        let docs = state.db.list_documents(pid).unwrap_or_default();
        json!(docs)
    } else {
        let mut all_docs = Vec::new();
        if let Ok(projects) = state.db.list_projects() {
            for p in projects {
                if let Ok(docs) = state.db.list_documents(&p.id) {
                    all_docs.extend(docs);
                }
            }
        }
        json!(all_docs)
    }
}

fn rpc_proposals(params: &Value, state: &AppState) -> Value {
    let project_id = params.get("project_id").and_then(|v| v.as_str());
    let proposals = state
        .db
        .list_pending_proposals(project_id)
        .unwrap_or_default();
    json!(proposals)
}

fn rpc_decide_proposal(params: &Value, state: &AppState) -> Value {
    let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let approved = params
        .get("approved")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    match state.db.decide_proposal(id, approved) {
        Ok(true) => {
            let _ = state.broadcast_tx.send(
                json!({
                    "type": "proposal_decided",
                    "id": id,
                    "approved": approved,
                    "timestamp": Utc::now().to_rfc3339(),
                })
                .to_string(),
            );
            json!({ "success": true, "id": id, "approved": approved })
        }
        Ok(false) => json!({ "error": "Proposal not found" }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

fn rpc_skills(params: &Value, state: &AppState) -> Value {
    let project_id = params.get("project_id").and_then(|v| v.as_str());
    let distiller = automemory_core::extraction::SkillDistiller::new(&state.db);
    let skills = distiller.distill_skills(project_id).unwrap_or_default();
    json!(skills)
}

fn rpc_constitution(params: &Value, state: &AppState) -> Value {
    let project_id = params.get("project_id").and_then(|v| v.as_str());
    let engine = RetrievalEngine::new(&state.db);
    let constitution = engine.generate_constitution(project_id).unwrap_or_default();
    json!({ "constitution": constitution })
}

fn rpc_assemble_context(params: &Value, state: &AppState) -> Value {
    let project_id = params.get("project_id").and_then(|v| v.as_str());
    let query = params.get("q").and_then(|v| v.as_str());
    let budget = params
        .get("token_budget")
        .and_then(|v| v.as_u64())
        .unwrap_or(3000) as usize;

    let engine = RetrievalEngine::new(&state.db);
    let ctx = engine
        .assemble_context(project_id, query, budget)
        .unwrap_or_default();
    json!({ "context": ctx })
}
