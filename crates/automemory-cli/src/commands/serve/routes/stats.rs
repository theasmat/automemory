use crate::commands::serve::state::{AppState, SearchParams};
use automemory_core::retrieval::RetrievalEngine;
use axum::{
    extract::{Query, State},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    Json,
};
use chrono::Utc;
use serde_json::json;
use std::time::Duration;

pub async fn api_stats(State(state): State<AppState>) -> impl IntoResponse {
    let (active_mem, archived_mem) = state.db.count_memories(None).unwrap_or((0, 0));
    let (mem_tokens, event_tokens) = state.db.get_token_stats(None).unwrap_or((0, 0));
    let traces = state.db.list_traces(None, 1000).unwrap_or_default();
    let failures = state
        .db
        .list_failures(None, Some("unresolved"), 1000)
        .unwrap_or_default();
    let projects = state.db.list_projects().unwrap_or_default();

    Json(json!({
        "active_memories": active_mem,
        "archived_memories": archived_mem,
        "total_memory_tokens": mem_tokens,
        "total_event_tokens": event_tokens,
        "tokens_saved_estimate": event_tokens.saturating_sub(mem_tokens),
        "active_traces": traces.len(),
        "unresolved_failures": failures.len(),
        "total_projects": projects.len(),
    }))
}

pub async fn api_context(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let engine = RetrievalEngine::new(&state.db);
    let ctx = engine
        .assemble_context(params.project_id.as_deref(), params.q.as_deref(), 3000)
        .unwrap_or_else(|e| format!("Error assembling context: {}", e));
    ctx
}

pub async fn api_docs(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    if let Some(pid) = params.project_id {
        let docs = state.db.list_documents(&pid).unwrap_or_default();
        Json(docs)
    } else {
        let mut all_docs = Vec::new();
        if let Ok(projects) = state.db.list_projects() {
            for p in projects {
                if let Ok(docs) = state.db.list_documents(&p.id) {
                    all_docs.extend(docs);
                }
            }
        }
        Json(all_docs)
    }
}

pub async fn api_graph(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let entities = state
        .db
        .list_all_graph_entities(params.project_id.as_deref(), 300)
        .unwrap_or_default();
    let edges = state.db.list_all_graph_edges(500).unwrap_or_default();

    Json(json!({
        "entities": entities,
        "edges": edges,
    }))
}

pub async fn api_stream(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let stream = async_stream::stream! {
        let mut interval = tokio::time::interval(Duration::from_millis(2500));
        loop {
            interval.tick().await;
            let (active_mem, _) = state.db.count_memories(None).unwrap_or((0, 0));
            let events = state.db.list_recent_events(10).unwrap_or_default();
            let proposals = state.db.list_pending_proposals(None).unwrap_or_default();

            let payload = json!({
                "type": "pulse",
                "active_memories": active_mem,
                "recent_events": events,
                "pending_proposals": proposals.len(),
                "timestamp": Utc::now().to_rfc3339(),
            });

            yield Ok(Event::default().data(payload.to_string()));
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// Live multi-agent coordination state for the dashboard: held leases, planning
/// rooms and recent handoff batons. Read-only.
pub async fn api_coordination(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let now = Utc::now();
    let leases: Vec<_> = state
        .db
        .list_active_leases(params.project_id.as_deref())
        .unwrap_or_default()
        .into_iter()
        .map(|l| {
            json!({
                "agent": l.agent, "session_id": l.session_id, "scope": l.scope,
                "scope_type": l.scope_type.to_string(), "goal": l.goal,
                "remaining_seconds": l.remaining_seconds(now),
            })
        })
        .collect();

    let (rooms, batons) = {
        let conn = state.db.conn();
        let rows = |sql: &str, cols: &[&str]| -> Vec<serde_json::Value> {
            let Ok(mut stmt) = conn.prepare(sql) else { return Vec::new() };
            let Ok(iter) = stmt.query_map([], |r| {
                let mut o = serde_json::Map::new();
                for (i, c) in cols.iter().enumerate() {
                    o.insert((*c).to_string(), json!(r.get::<_, Option<String>>(i)?));
                }
                Ok(serde_json::Value::Object(o))
            }) else {
                return Vec::new();
            };
            iter.flatten().collect()
        };
        (
            rows(
                "SELECT id, topic, state, turn_holder, CAST(round AS TEXT), updated_at \
                 FROM rooms ORDER BY updated_at DESC LIMIT 20",
                &["id", "topic", "state", "turn_holder", "round", "updated_at"],
            ),
            rows(
                "SELECT id, agent, from_session, to_session, packet, created_at \
                 FROM batons ORDER BY created_at DESC LIMIT 10",
                &["id", "agent", "from_session", "to_session", "packet", "created_at"],
            ),
        )
    };
    let alerts: Vec<_> = state
        .db
        .recent_alerts(10)
        .into_iter()
        .map(|(ts, path, message)| json!({ "ts": ts, "path": path, "message": message }))
        .collect();
    Json(json!({ "leases": leases, "rooms": rooms, "batons": batons, "alerts": alerts }))
}
