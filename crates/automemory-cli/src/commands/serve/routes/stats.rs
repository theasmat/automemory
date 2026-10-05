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
