use crate::commands::serve::state::{AppState, SearchParams};
use automemory_core::models::*;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use std::str::FromStr;

#[derive(Deserialize)]
pub struct ModePayload {
    pub mode: String,
    pub project: Option<String>,
}

pub async fn api_get_mode(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let mode = if let Some(ref pid) = params.project_id {
        if let Ok(Some(p)) = state.db.get_project_by_slug(pid) {
            p.capture_policy.mode
        } else if let Ok(Some(p)) = state.db.get_project_by_id(pid) {
            p.capture_policy.mode
        } else {
            AutopilotMode::Vibe
        }
    } else {
        AutopilotMode::Vibe
    };

    Json(json!({
        "mode": format!("{:?}", mode).to_lowercase(),
        "project": params.project_id,
    }))
}

pub async fn api_set_mode(
    State(state): State<AppState>,
    Json(payload): Json<ModePayload>,
) -> impl IntoResponse {
    let mode = match AutopilotMode::from_str(&payload.mode) {
        Ok(m) => m,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()}))),
    };

    let slug = payload.project.clone().unwrap_or_else(|| "default".to_string());

    if let Ok(Some(mut project)) = state.db.get_project_by_slug(&slug) {
        project.capture_policy.mode = mode;
        project.updated_at = Utc::now();
        let _ = state.db.save_project(&project);
    } else {
        let new_project = Project {
            id: uuid::Uuid::new_v4().to_string(),
            slug: slug.clone(),
            display_name: slug.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            capture_policy: CapturePolicy {
                mode,
                ..Default::default()
            },
            context_budget: Default::default(),
            document_config: Default::default(),
            sync_config: Default::default(),
        };
        let _ = state.db.save_project(&new_project);
    }

    let _ = state.broadcast_tx.send(json!({
        "type": "mode_updated",
        "mode": payload.mode,
        "project": slug,
        "timestamp": Utc::now().to_rfc3339(),
    }).to_string());

    (StatusCode::OK, Json(json!({ "success": true, "mode": payload.mode, "project": slug })))
}

pub async fn api_guardrails_rules() -> impl IntoResponse {
    let rules = automemory_core::guardrails::GuardrailRule::default_rules();
    Json(rules)
}

#[derive(Deserialize)]
pub struct GuardrailEvalPayload {
    pub tool_name: String,
    pub args: Value,
    pub project_id: Option<String>,
    pub mode: Option<String>,
}

pub async fn api_guardrails_evaluate(
    State(state): State<AppState>,
    Json(payload): Json<GuardrailEvalPayload>,
) -> impl IntoResponse {
    let mode = payload
        .mode
        .as_deref()
        .and_then(|m| AutopilotMode::from_str(m).ok())
        .unwrap_or(AutopilotMode::Vibe);

    let evaluator = automemory_core::guardrails::GuardrailEvaluator::new(Some(&state.db));
    let decision = evaluator.evaluate(
        &payload.tool_name,
        &payload.args,
        payload.project_id.as_deref(),
        mode,
    );

    Json(decision)
}
