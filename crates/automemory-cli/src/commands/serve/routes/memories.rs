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

    // Notes that came from a teammate say so (`metadata.shared_by`), added to the response only.
    let out: Vec<serde_json::Value> = memories
        .iter()
        .map(|m| {
            let mut v = serde_json::to_value(m).unwrap_or_default();
            if let (Some(by), Some(meta)) = (automemory_core::sync::deltas::shared_by(&state.db, &m.id), v.get_mut("metadata").and_then(|x| x.as_object_mut())) {
                meta.insert("shared_by".into(), by.into());
            }
            v
        })
        .collect();
    Json(out)
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
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if let Err(denied) = super::guardrails::require_human_header(&headers, "archive a memory") {
        return denied;
    }
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
pub struct AddMemoryPayload {
    pub summary: String,
    pub content: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub project_id: Option<String>,
}

/// A person adds knowledge from the dashboard: trusted (user source) and active at once.
pub async fn api_memory_add(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(p): Json<AddMemoryPayload>,
) -> impl IntoResponse {
    use automemory_core::models::*;
    if let Err(denied) = super::guardrails::require_human_header(&headers, "add memory") {
        return denied.into_response();
    }
    let summary = p.summary.trim();
    if summary.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "summary is required" }))).into_response();
    }
    let Ok(kind) = p.kind.as_deref().unwrap_or("fact").parse::<MemoryType>() else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "unknown type" }))).into_response();
    };
    let now = Utc::now();
    let mem = Memory {
        id: uuid::Uuid::now_v7().to_string(),
        scope: if p.project_id.is_some() { MemoryScope::Project } else { MemoryScope::Global },
        project_id: p.project_id,
        memory_type: kind,
        content: p.content.filter(|c| !c.trim().is_empty()).unwrap_or_else(|| summary.to_string()),
        summary: summary.to_string(), confidence: 1.0, importance: 0.8, freshness: 1.0,
        status: MemoryStatus::Active, source_kind: MemorySourceKind::User, version: Some(1),
        created_at: now, updated_at: now, last_recalled_at: None, tags: vec![], entities: vec![],
        metadata: json!({ "via": "dashboard" }),
    };
    match state.db.save_memory(&mem) {
        Ok(()) => {
            let _ = state.broadcast_tx.send(json!({ "type": "memory_added", "id": mem.id, "timestamp": now.to_rfc3339() }).to_string());
            (StatusCode::OK, Json(json!({ "success": true, "memory": mem }))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
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
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(payload): Json<UpdateMemoryPayload>,
) -> impl IntoResponse {
    if let Err(denied) = super::guardrails::require_human_header(&headers, "edit a memory") {
        return denied;
    }
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
