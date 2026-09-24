use crate::commands::serve::state::{AppState, SearchParams};
use automemory_core::extraction::TrajectoryDistiller;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde_json::json;

pub async fn api_events(State(state): State<AppState>) -> impl IntoResponse {
    let events = state.db.list_recent_events(25).unwrap_or_default();
    Json(events)
}

pub async fn api_sessions(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let sessions = state.db.list_sessions(params.project_id.as_deref(), 50).unwrap_or_default();
    Json(sessions)
}

pub async fn api_session_events(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let events = state.db.get_session_events(&session_id).unwrap_or_default();
    Json(events)
}

pub async fn api_session_distill(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let distiller = TrajectoryDistiller::new(&state.db);
    match distiller.distill_session(&session_id) {
        Ok(report) => {
            let _ = state.broadcast_tx.send(json!({
                "type": "session_distilled",
                "session_id": session_id,
                "report": report,
                "timestamp": Utc::now().to_rfc3339(),
            }).to_string());
            (StatusCode::OK, Json(json!({ "success": true, "report": report })))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        ),
    }
}

pub async fn api_traces(State(state): State<AppState>) -> impl IntoResponse {
    let traces = state.db.list_traces(None, 50).unwrap_or_default();
    Json(traces)
}

pub async fn api_failures(State(state): State<AppState>) -> impl IntoResponse {
    let failures = state.db.list_failures(None, None, 50).unwrap_or_default();
    Json(failures)
}
