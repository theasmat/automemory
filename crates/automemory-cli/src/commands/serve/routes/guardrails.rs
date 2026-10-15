use crate::commands::serve::state::{AppState, SearchParams};
use automemory_core::guardrails::CustomGuardrailRule;
use automemory_core::models::*;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use std::str::FromStr;

/// Human gate for dashboard safety writes. The browser dashboard prompts for
/// the password and sends it as `X-AutoMemory-Password`; scripts use the same
/// header. Unset password warns once and allows (fresh installs stay usable).
fn require_human_header(
    headers: &HeaderMap,
    action: &str,
) -> Result<(), (StatusCode, Json<Value>)> {
    let deny = |msg: String| (StatusCode::FORBIDDEN, Json(json!({ "error": msg })));
    let Some(hash) =
        automemory_core::auth::stored_password_hash().filter(|h| !h.trim().is_empty())
    else {
        eprintln!(
            "⚠ Dashboard safety write '{action}' allowed with no human password set. Run `automemory rule passwd` to lock it."
        );
        return Ok(());
    };
    match headers
        .get("x-automemory-password")
        .and_then(|v| v.to_str().ok())
    {
        Some(pw) if automemory_core::auth::verify_password(pw, &hash) => Ok(()),
        _ => Err(deny(format!(
            "Safety change '{action}' needs the human password in the X-AutoMemory-Password header."
        ))),
    }
}

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
    headers: HeaderMap,
    Json(payload): Json<ModePayload>,
) -> impl IntoResponse {
    if let Err(e) = require_human_header(&headers, "mode set") {
        return e;
    };
    let mode = match AutopilotMode::from_str(&payload.mode) {
        Ok(m) => m,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": e.to_string()})),
            )
        }
    };

    let slug = payload
        .project
        .clone()
        .unwrap_or_else(|| "default".to_string());

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

    let _ = state.broadcast_tx.send(
        json!({
            "type": "mode_updated",
            "mode": payload.mode,
            "project": slug,
            "timestamp": Utc::now().to_rfc3339(),
        })
        .to_string(),
    );

    (
        StatusCode::OK,
        Json(json!({ "success": true, "mode": payload.mode, "project": slug })),
    )
}

pub async fn api_guardrails_rules(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let defaults = automemory_core::guardrails::GuardrailRule::default_rules();
    let mut result = Vec::new();

    for (idx, r) in defaults.into_iter().enumerate() {
        result.push(json!({
            "id": format!("builtin_{}", idx),
            "name": r.name,
            "category": r.category,
            "pattern": r.pattern,
            "is_critical": r.is_critical,
            "action": if r.is_critical { "block" } else { "warn" },
            "description": r.description,
            "mitigation_hint": r.mitigation_hint,
            "is_custom": false,
            "enabled": true,
        }));
    }

    if let Ok(customs) = state
        .db
        .list_custom_guardrail_rules(params.project_id.as_deref())
    {
        for c in customs {
            result.push(json!({
                "id": c.id,
                "project_id": c.project_id,
                "name": c.name,
                "category": c.category,
                "pattern": c.pattern,
                "is_critical": c.is_critical,
                "action": c.action,
                "description": c.description,
                "mitigation_hint": c.mitigation_hint,
                "is_custom": true,
                "enabled": c.enabled,
                "created_at": c.created_at.to_rfc3339(),
            }));
        }
    }

    Json(result)
}

#[derive(Deserialize)]
pub struct CreateRulePayload {
    pub name: String,
    pub category: Option<String>,
    pub pattern: String,
    pub is_critical: Option<bool>,
    pub action: Option<String>,
    pub description: Option<String>,
    pub mitigation_hint: Option<String>,
    pub project_id: Option<String>,
}

pub async fn api_guardrail_rule_add(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateRulePayload>,
) -> impl IntoResponse {
    if let Err(e) = require_human_header(&headers, "rule add") {
        return e;
    }
    if let Err(e) = regex::Regex::new(&payload.pattern) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Invalid regex pattern: {}", e) })),
        );
    }

    let rule = CustomGuardrailRule {
        id: format!("custom_{}", uuid::Uuid::now_v7()),
        project_id: payload.project_id,
        name: payload.name,
        category: payload.category.unwrap_or_else(|| "custom".to_string()),
        pattern: payload.pattern,
        is_critical: payload.is_critical.unwrap_or(false),
        action: payload.action.unwrap_or_else(|| "block".to_string()),
        description: payload.description.unwrap_or_default(),
        mitigation_hint: payload.mitigation_hint.unwrap_or_default(),
        enabled: true,
        created_at: Utc::now(),
    };

    match state.db.save_custom_guardrail_rule(&rule) {
        Ok(()) => {
            let _ = state.broadcast_tx.send(
                json!({
                    "type": "guardrail_rule_added",
                    "id": rule.id,
                    "name": rule.name,
                    "timestamp": Utc::now().to_rfc3339(),
                })
                .to_string(),
            );
            (
                StatusCode::CREATED,
                Json(json!({ "success": true, "rule": rule })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

pub async fn api_guardrail_rule_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if let Err(e) = require_human_header(&headers, "rule delete") {
        return e;
    }
    match state.db.delete_custom_guardrail_rule(&id) {
        Ok(true) => {
            let _ = state.broadcast_tx.send(
                json!({
                    "type": "guardrail_rule_deleted",
                    "id": id,
                    "timestamp": Utc::now().to_rfc3339(),
                })
                .to_string(),
            );
            (StatusCode::OK, Json(json!({ "success": true, "id": id })))
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Rule not found" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

#[derive(Deserialize)]
pub struct ToggleRulePayload {
    pub enabled: bool,
}

pub async fn api_guardrail_rule_toggle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<ToggleRulePayload>,
) -> impl IntoResponse {
    if let Err(e) = require_human_header(&headers, "rule toggle") {
        return e;
    }
    match state.db.toggle_custom_guardrail_rule(&id, payload.enabled) {
        Ok(true) => {
            let _ = state.broadcast_tx.send(
                json!({
                    "type": "guardrail_rule_toggled",
                    "id": id,
                    "enabled": payload.enabled,
                    "timestamp": Utc::now().to_rfc3339(),
                })
                .to_string(),
            );
            (
                StatusCode::OK,
                Json(json!({ "success": true, "id": id, "enabled": payload.enabled })),
            )
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Rule not found" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
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
