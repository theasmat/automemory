use crate::commands::serve::state::{AppState, SearchParams};
use automemory_core::extraction::SkillDistiller;
use automemory_core::models::*;
use automemory_core::retrieval::RetrievalEngine;
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
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
        let results = state
            .db
            .search_memories_fts(q, params.project_id.as_deref(), limit)
            .unwrap_or_default();
        results.into_iter().map(|(m, _)| m).collect()
    } else {
        state
            .db
            .list_memories(
                params.project_id.as_deref(),
                Some(MemoryStatus::Active),
                None,
                limit,
            )
            .unwrap_or_default()
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
    let mut skills = state
        .db
        .list_skills(params.project_id.as_deref(), 50)
        .unwrap_or_default();
    if skills.is_empty() {
        let distiller = SkillDistiller::new(&state.db);
        skills = distiller
            .distill_skills(params.project_id.as_deref())
            .unwrap_or_default();
    }
    Json(skills)
}

pub async fn api_constitution(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let engine = RetrievalEngine::new(&state.db);
    let constitution = engine
        .generate_constitution(params.project_id.as_deref())
        .unwrap_or_default();
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
    headers: HeaderMap,
    Json(payload): Json<DecideProposalPayload>,
) -> impl IntoResponse {
    if let Err(denied) = super::guardrails::require_human_header(&headers, "decide proposal") {
        return denied;
    }
    match state.db.decide_proposal(&payload.id, payload.approved) {
        Ok(true) => {
            let _ = state.broadcast_tx.send(
                json!({
                    "type": "proposal_decided",
                    "id": payload.id,
                    "approved": payload.approved,
                    "timestamp": Utc::now().to_rfc3339(),
                })
                .to_string(),
            );
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

pub async fn api_proposals_summary(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    match state.db.get_proposals_summary(params.project_id.as_deref()) {
        Ok(summary) => (StatusCode::OK, Json(json!(summary))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

#[derive(Deserialize)]
pub struct DecideAllProposalsPayload {
    pub project_id: Option<String>,
    pub proposal_type: Option<String>,
    pub approved: bool,
}

pub async fn api_proposals_decide_all(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<DecideAllProposalsPayload>,
) -> impl IntoResponse {
    if let Err(denied) = super::guardrails::require_human_header(&headers, "decide all proposals") {
        return denied;
    }
    match state.db.decide_all_proposals(
        payload.project_id.as_deref(),
        payload.proposal_type.as_deref(),
        payload.approved,
    ) {
        Ok(result) => {
            let _ = state.broadcast_tx.send(
                json!({
                    "type": "proposals_decided_all",
                    "approved": payload.approved,
                    "decided_count": result.decided_count,
                    "archived_memories_count": result.archived_memories_count,
                    "timestamp": Utc::now().to_rfc3339(),
                })
                .to_string(),
            );
            (
                StatusCode::OK,
                Json(json!({ "success": true, "result": result })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

pub async fn api_memory_archive(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match state.db.archive_memory(&id) {
        Ok(true) => {
            let _ = state.broadcast_tx.send(
                json!({
                    "type": "memory_archived",
                    "id": id,
                    "timestamp": Utc::now().to_rfc3339(),
                })
                .to_string(),
            );
            (StatusCode::OK, Json(json!({ "success": true, "id": id })))
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Memory not found" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

#[derive(Deserialize)]
pub struct UpdateMemoryPayload {
    pub summary: Option<String>,
    pub content: Option<String>,
    pub reason: Option<String>,
}

pub async fn api_memory_update(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(payload): Json<UpdateMemoryPayload>,
) -> impl IntoResponse {
    match state.db.get_memory_by_id(&id) {
        Ok(Some(mut mem)) => {
            if let Some(s) = payload.summary {
                mem.summary = s;
            }
            if let Some(c) = payload.content {
                mem.content = c;
            }
            mem.updated_at = Utc::now();
            let reason = payload
                .reason
                .unwrap_or_else(|| "Manual edit via dashboard".to_string());

            match state.db.update_memory(&mem, Some(&reason), &[]) {
                Ok(()) => {
                    let _ = state.broadcast_tx.send(
                        json!({
                            "type": "memory_updated",
                            "id": id,
                            "timestamp": Utc::now().to_rfc3339(),
                        })
                        .to_string(),
                    );
                    (
                        StatusCode::OK,
                        Json(json!({ "success": true, "memory": mem })),
                    )
                }
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": e.to_string() })),
                ),
            }
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Memory not found" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}
