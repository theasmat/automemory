use crate::commands::serve::state::AppState;
use axum::{extract::State, response::IntoResponse, Json};
use serde_json::json;

pub async fn api_sync_trigger(State(state): State<AppState>) -> impl IntoResponse {
    state.sync_daemon.trigger();
    Json(json!({"status": "triggered", "message": "Automated sync scheduled"}))
}

pub async fn api_projects(State(state): State<AppState>) -> impl IntoResponse {
    let projects = state.db.list_projects().unwrap_or_default();
    Json(projects)
}
