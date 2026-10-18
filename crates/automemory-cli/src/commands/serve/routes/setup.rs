use crate::commands::serve::routes::guardrails::require_human_header;
use crate::commands::serve::state::AppState;
use automemory_core::onboarding;
use automemory_core::setup;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

/// GET /api/setup: Returns the getting started checklist.
pub async fn api_setup(State(state): State<AppState>) -> impl IntoResponse {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let steps = setup::checklist(&state.db, &home, &cwd);
    Json(steps)
}

/// GET /api/onboarding: Returns the current onboarding progress state.
pub async fn api_get_onboarding(State(state): State<AppState>) -> impl IntoResponse {
    let ob = onboarding::load(&state.db);
    Json(ob)
}

#[derive(Deserialize, Default)]
pub struct OnboardingPayload {
    pub action: Option<String>,
    pub step_id: Option<String>,
    pub flag: Option<String>,
    pub demo_loaded: Option<bool>,
    pub tour_step: Option<usize>,
    pub completed_steps: Option<Vec<String>>,
}

/// POST /api/onboarding: Updates onboarding progress (human-gated).
pub async fn api_post_onboarding(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<OnboardingPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<Value>)> {
    require_human_header(&headers, "update onboarding")?;

    if let Some(action) = payload.action.as_deref() {
        match action {
            "mark_done" => {
                if let Some(ref step) = payload.step_id {
                    onboarding::mark_done(&state.db, step)
                        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
                }
            }
            "dismiss" => {
                onboarding::dismiss(&state.db, payload.flag.as_deref())
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
            }
            "reset" => {
                onboarding::reset(&state.db)
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
            }
            other => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": format!("Unknown action: {other}")})),
                ));
            }
        }
    } else {
        if let Some(ref step) = payload.step_id {
            let _ = onboarding::mark_done(&state.db, step);
        }
    }

    if let Some(loaded) = payload.demo_loaded {
        let _ = onboarding::set_demo_loaded(&state.db, loaded);
    }
    if let Some(ts) = payload.tour_step {
        let _ = onboarding::set_tour_step(&state.db, ts);
    }
    if let Some(steps) = payload.completed_steps {
        let mut cur = onboarding::load(&state.db);
        for s in steps {
            if !cur.is_done(&s) {
                cur.completed_steps.push(s);
            }
        }
        let _ = onboarding::save(&state.db, &cur);
    }

    let updated = onboarding::load(&state.db);
    let _ = state.broadcast_tx.send(
        json!({
            "type": "setup_changed",
            "timestamp": chrono::Utc::now().to_rfc3339()
        })
        .to_string(),
    );
    Ok(Json(updated))
}

/// GET /api/learn: Returns guided learning topics.
pub async fn api_learn() -> impl IntoResponse {
    Json(setup::LEARN_TOPICS)
}

/// POST /api/demo: Seeds a sample project (human-gated).
pub async fn api_post_demo(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<Value>)> {
    require_human_header(&headers, "seed sample demo project")?;
    onboarding::seed_demo(&state.db)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let _ = state.broadcast_tx.send(
        json!({
            "type": "setup_changed",
            "timestamp": chrono::Utc::now().to_rfc3339()
        })
        .to_string(),
    );

    Ok(Json(json!({
        "ok": true,
        "message": "Demo project seeded with sample decisions, notes, and suggestions"
    })))
}

/// DELETE /api/demo: Removes the sample project (human-gated).
pub async fn api_delete_demo(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<Value>)> {
    require_human_header(&headers, "remove sample demo project")?;
    onboarding::remove_demo(&state.db)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let _ = state.broadcast_tx.send(
        json!({
            "type": "setup_changed",
            "timestamp": chrono::Utc::now().to_rfc3339()
        })
        .to_string(),
    );

    Ok(Json(json!({
        "ok": true,
        "message": "Demo project and all sample data removed"
    })))
}

