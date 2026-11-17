//! `GET /api/changes`: the file changes Tracewise recorded, for the dashboard (Agents & Locks, session replay, a note's files).
//! Read-only. Each call starts `tracewise`, so answers are kept for a few seconds per query.

use crate::commands::serve::state::AppState;
use automemory_core::companion::{self, ChangeFilter};
use axum::{extract::{Query, State}, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const CACHE_TTL: Duration = Duration::from_secs(5);

#[derive(Deserialize)]
pub struct ChangesQuery {
    pub project_id: Option<String>,
    /// A session id (at least its first 8 characters).
    pub session: Option<String>,
    /// Comma-separated files; at most five are used.
    pub paths: Option<String>,
    pub limit: Option<usize>,
}

fn cache() -> &'static Mutex<HashMap<String, (Instant, Value)>> {
    static C: OnceLock<Mutex<HashMap<String, (Instant, Value)>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

pub async fn api_changes(State(state): State<AppState>, Query(q): Query<ChangesQuery>) -> impl IntoResponse {
    let key = format!("{:?}|{:?}|{:?}|{:?}", q.project_id, q.session, q.paths, q.limit);
    if let Some((at, v)) = cache().lock().ok().and_then(|c| c.get(&key).cloned()) {
        if at.elapsed() < CACHE_TTL {
            return Json(v);
        }
    }
    let db = state.db.clone();
    let result = tokio::task::spawn_blocking(move || {
        let settings = automemory_core::config::GlobalConfig::load().unwrap_or_default().companion;
        let filter = ChangeFilter {
            session: q.session.filter(|s| !s.trim().is_empty()),
            paths: q.paths.as_deref().unwrap_or("").split(',').map(str::trim).filter(|p| !p.is_empty()).map(String::from).collect(),
            limit: q.limit.unwrap_or(30),
        };
        companion::file_changes(&settings, &db, q.project_id.as_deref(), &filter)
    })
    .await;
    let body = match result {
        Ok(Ok(changes)) => json!({ "source": "tracewise", "changes": changes }),
        Ok(Err(note)) => json!({ "source": "none", "changes": [], "note": note }),
        Err(_) => json!({ "source": "none", "changes": [], "note": "the change list could not be read" }),
    };
    if let Ok(mut c) = cache().lock() {
        if c.len() > 64 {
            c.clear();
        }
        c.insert(key, (Instant::now(), body.clone()));
    }
    Json(body)
}
