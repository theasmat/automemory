use crate::commands::serve::state::AppState;
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::fs;

#[derive(Debug, Deserialize)]
pub struct ProjectQuery {
    pub project_id: Option<String>,
}

pub async fn api_sync_trigger(State(state): State<AppState>) -> impl IntoResponse {
    state.sync_daemon.trigger();
    Json(json!({"status": "triggered", "message": "Automated sync scheduled"}))
}

pub async fn api_projects(State(state): State<AppState>) -> impl IntoResponse {
    let projects = state.db.list_projects().unwrap_or_default();
    Json(projects)
}

pub async fn api_sync_status(
    State(state): State<AppState>,
    Query(params): Query<ProjectQuery>,
) -> impl IntoResponse {
    let project_id = params
        .project_id
        .unwrap_or_else(|| "default-project".to_string());
    let docs = state.db.list_documents(&project_id).unwrap_or_default();
    let (active_mem, _) = state.db.count_memories(Some(&project_id)).unwrap_or((0, 0));
    let skills = state
        .db
        .list_skills(Some(&project_id), 100)
        .unwrap_or_default();

    let mut pending = 0;
    let mut synced = 0;
    for d in &docs {
        if let Some(ref remote) = d.remote_synced_hash {
            if remote == &d.content_hash {
                synced += 1;
                continue;
            }
        }
        pending += 1;
    }

    // Check canonical snapshots
    let mut snapshots = Vec::new();
    let config = automemory_core::config::Config::default_config();
    let deltas_dir = config
        .home_dir
        .join("context")
        .join("projects")
        .join(&project_id)
        .join("deltas");
    if let Ok(entries) = fs::read_dir(deltas_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".json") {
                    snapshots.push(name.to_string());
                }
            }
        }
    }
    snapshots.sort();
    snapshots.reverse();

    let (device_id, device_name) =
        crate::commands::sync::get_or_create_device_identity(&config.home_dir);

    Json(json!({
        "project_id": project_id,
        "total_documents": docs.len(),
        "synced_documents": synced,
        "pending_deltas": pending,
        "active_memories": active_mem,
        "skills_count": skills.len(),
        "snapshots": snapshots,
        "device_id": device_id,
        "device_name": device_name,
    }))
}

// Local mock cloud endpoint for offline testing