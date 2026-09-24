use crate::commands::serve::state::{AppState, SearchParams};
use automemory_core::extraction::SkillDistiller;
use automemory_core::models::*;
use automemory_core::retrieval::RetrievalEngine;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

pub async fn api_memories(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let limit = 50;
    let mut memories: Vec<Memory> = if let Some(ref q) = params.q {
        let results = state.db.search_memories_fts(q, params.project_id.as_deref(), limit).unwrap_or_default();
        results.into_iter().map(|(m, _)| m).collect()
    } else {
        state.db.list_memories(params.project_id.as_deref(), Some(MemoryStatus::Active), None, limit).unwrap_or_default()
    };

    if let Some(ref scope) = params.scope {
        if scope == "local" {
            memories.retain(|m| m.scope == MemoryScope::Project);
        } else if scope == "cloud" || scope == "global" {
            memories.retain(|m| m.scope == MemoryScope::Global);
        }
    }

    Json(memories)
}

pub async fn api_skills(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let distiller = SkillDistiller::new(&state.db);
    let skills = distiller.distill_skills(params.project_id.as_deref()).unwrap_or_default();
    Json(skills)
}

pub async fn api_constitution(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let engine = RetrievalEngine::new(&state.db);
    let constitution = engine.generate_constitution(params.project_id.as_deref()).unwrap_or_default();
    Json(json!({ "constitution": constitution }))
}

pub async fn api_preferences(State(state): State<AppState>) -> impl IntoResponse {
    let prefs = state.db.list_preferences(None).unwrap_or_default();
    Json(prefs)
}

pub async fn api_proposals(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let proposals = state
        .db
        .list_pending_proposals(params.project_id.as_deref())
        .unwrap_or_default();
    Json(proposals)
}

#[derive(Deserialize)]
pub struct DecideProposalPayload {
    pub id: String,
    pub approved: bool,
}

pub async fn api_proposals_decide(
    State(state): State<AppState>,
    Json(payload): Json<DecideProposalPayload>,
) -> impl IntoResponse {
    match state.db.decide_proposal(&payload.id, payload.approved) {
        Ok(true) => {
            let _ = state.broadcast_tx.send(json!({
                "type": "proposal_decided",
                "id": payload.id,
                "approved": payload.approved,
                "timestamp": Utc::now().to_rfc3339(),
            }).to_string());
            (
                StatusCode::OK,
                Json(json!({ "success": true, "id": payload.id, "approved": payload.approved })),
            )
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Proposal not found" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}
