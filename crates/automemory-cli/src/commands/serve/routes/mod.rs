pub mod guardrails;
pub mod memories;
pub mod stats;
pub mod sync;
pub mod traces;

use super::state::AppState;
use axum::{
    routing::{delete, get, post},
    Router,
};

pub fn build_api_router() -> Router<AppState> {
    Router::new()
        .route("/api/stats", get(stats::api_stats))
        .route("/api/coordination", get(stats::api_coordination))
        .route(
            "/api/mode",
            get(guardrails::api_get_mode).post(guardrails::api_set_mode),
        )
        .route(
            "/api/guardrails/rules",
            get(guardrails::api_guardrails_rules).post(guardrails::api_guardrail_rule_add),
        )
        .route(
            "/api/guardrails/rules/:id",
            delete(guardrails::api_guardrail_rule_delete),
        )
        .route(
            "/api/guardrails/rules/:id/toggle",
            post(guardrails::api_guardrail_rule_toggle),
        )
        .route(
            "/api/guardrails/evaluate",
            post(guardrails::api_guardrails_evaluate),
        )
        .route("/api/events", get(traces::api_events))
        .route("/api/sessions", get(traces::api_sessions))
        .route("/api/sessions/:id/events", get(traces::api_session_events))
        .route(
            "/api/sessions/:id/distill",
            post(traces::api_session_distill),
        )
        .route("/api/memories", get(memories::api_memories))
        .route(
            "/api/memories/:id/archive",
            post(memories::api_memory_archive),
        )
        .route(
            "/api/memories/:id/update",
            post(memories::api_memory_update),
        )
        .route("/api/skills", get(memories::api_skills))
        .route("/api/constitution", get(memories::api_constitution))
        .route("/api/traces", get(traces::api_traces))
        .route("/api/failures", get(traces::api_failures))
        .route(
            "/api/failures/:id/resolve",
            post(traces::api_failure_resolve),
        )
        .route("/api/projects", get(sync::api_projects))
        .route("/api/preferences", get(memories::api_preferences))
        .route("/api/context", get(stats::api_context))
        .route("/api/docs", get(stats::api_docs))
        .route("/api/proposals", get(memories::api_proposals))
        .route(
            "/api/proposals/summary",
            get(memories::api_proposals_summary),
        )
        .route(
            "/api/proposals/decide",
            post(memories::api_proposals_decide),
        )
        .route(
            "/api/proposals/decide_all",
            post(memories::api_proposals_decide_all),
        )
        .route("/api/graph", get(stats::api_graph))
        .route("/api/stream", get(stats::api_stream))
        .route("/api/sync/status", get(sync::api_sync_status))
        .route("/api/sync/trigger", post(sync::api_sync_trigger))
        .route("/api/cloud/sync", post(sync::api_mock_cloud_sync))
        .route(
            "/api/cloud/projects/:id/manifest",
            get(sync::api_mock_cloud_manifest),
        )
}
