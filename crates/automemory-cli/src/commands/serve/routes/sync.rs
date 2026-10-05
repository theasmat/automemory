use crate::commands::serve::state::AppState;
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;

#[derive(Debug, Deserialize)]
pub struct ProjectQuery {
    pub project_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MockCloudPayload {
    pub project_id: String,
    pub timestamp: String,
    pub documents: Vec<serde_json::Value>,
    pub memories: Vec<serde_json::Value>,
    pub skills: Vec<serde_json::Value>,
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
pub async fn api_mock_cloud_sync(
    State(_state): State<AppState>,
    Json(payload): Json<MockCloudPayload>,
) -> impl IntoResponse {
    let project_id = payload.project_id.clone();
    let config = automemory_core::config::Config::default_config();
    let mock_dir = config
        .home_dir
        .join("mock_cloud")
        .join("projects")
        .join(&project_id);
    let _ = fs::create_dir_all(&mock_dir);
    let manifest_path = mock_dir.join("manifest.json");
    let _ = fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&payload).unwrap_or_default(),
    );

    Json(json!({
        "status": "success",
        "message": format!("Local mock cloud received delta for project '{}'", project_id),
        "docs_received": payload.documents.len(),
        "memories_received": payload.memories.len(),
        "skills_received": payload.skills.len(),
        "timestamp": payload.timestamp,
    }))
}

pub async fn api_mock_cloud_manifest(
    State(_state): State<AppState>,
    Path(project_id): Path<String>,
) -> impl IntoResponse {
    let config = automemory_core::config::Config::default_config();
    let manifest_path = config
        .home_dir
        .join("mock_cloud")
        .join("projects")
        .join(&project_id)
        .join("manifest.json");
    if let Ok(content) = fs::read_to_string(&manifest_path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
            return Json(val);
        }
    }

    Json(json!({
        "project_id": project_id,
        "documents": [],
        "memories": [],
        "skills": [],
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}
