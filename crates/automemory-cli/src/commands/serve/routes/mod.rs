pub mod control;
pub mod guardrails;
pub mod memories;
pub mod llm;
pub mod recall;
pub mod rooms;
pub mod setup;
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
        .route("/api/setup", get(setup::api_setup))
        .route(
            "/api/onboarding",
            get(setup::api_get_onboarding).post(setup::api_post_onboarding),
        )
        .route("/api/learn", get(setup::api_learn))
        .route(
            "/api/demo",
            post(setup::api_post_demo).delete(setup::api_delete_demo),
        )
        .route("/api/rooms", get(rooms::api_rooms).post(rooms::api_room_create))
        .route("/api/rooms/:id", get(rooms::api_room))
        .route("/api/rooms/:id/note", post(rooms::api_room_note))
        .route("/api/rooms/:id/decide", post(rooms::api_room_decide))
        .route("/api/rooms/:id/approve", post(rooms::api_room_approve))
        .route("/api/rooms/:id/reject", post(rooms::api_room_reject))
        .route("/api/rooms/:id/another-round", post(rooms::api_room_again))
        .route("/api/value", get(control::api_value))
        .route("/api/memories/hygiene", get(control::api_memory_hygiene))
        .route("/api/memories/archive_many", post(control::api_memory_archive_many))
        .route("/api/memories/:id/pin", post(control::api_memory_pin))
        .route("/api/memories/:id/veto", post(control::api_memory_veto))
        .route("/api/memories/:id/restore", post(control::api_memory_restore))
        .route("/api/memories/:id/revoke", post(control::api_memory_revoke))
        .route("/api/decisions", get(control::api_decisions).post(control::api_decision_record))
        .route("/api/tools/doctor", post(control::api_tool_doctor))
        .route("/api/tools/eval", post(control::api_tool_eval))
        .route("/api/tools/docs-rescan", post(control::api_tool_docs_rescan))
        .route("/api/tools/graph-index", post(control::api_tool_graph_index))
        .route("/api/tools/backup", post(control::api_tool_backup))
        .route("/api/tools/handoff", post(control::api_tool_handoff))
        .route("/api/tools/export-rules", get(control::api_export_preview).post(control::api_export_apply))
        .route("/api/sync/pull", post(control::api_sync_pull))
        .route("/api/agents/setup", get(control::api_agents_setup))
        .route("/api/agents/connect", post(control::api_agent_connect))
        .route("/api/agents/disconnect", post(control::api_agent_disconnect))
        .route("/api/locks/release", post(control::api_lock_release))
        .route("/api/locks/stop", post(control::api_lock_stop))
        .route("/api/settings/locks", get(control::api_settings_locks_get).put(control::api_settings_locks_put))
        .route("/api/activity", get(control::api_activity))
        .route("/api/policy", get(crate::commands::serve::policy::api_policy))
        .route("/api/jobs", get(crate::commands::serve::jobs::api_list))
        .route("/api/jobs/:id/cancel", post(crate::commands::serve::jobs::api_cancel))
        .route("/api/llm", get(llm::api_get).put(llm::api_put))
        .route("/api/llm/profile/:name", delete(llm::api_profile_delete))
        .route("/api/llm/key", axum::routing::put(llm::api_key_set).delete(llm::api_key_delete))
        .route("/api/llm/run", post(llm::api_run))
        .route("/api/llm/run/cancel", post(llm::api_run_cancel))
        .route("/api/recall", get(recall::api_get).put(recall::api_put))
        .route("/api/recall/profile", post(recall::api_profile_save))
        .route("/api/recall/profile/:name", delete(recall::api_profile_delete))
        .route("/api/recall/key", axum::routing::put(recall::api_key_set).delete(recall::api_key_delete))
        .route("/api/recall/test", post(recall::api_test))
        .route("/api/recall/index", post(recall::api_index))
        .route("/api/recall/index/cancel", post(recall::api_index_cancel))
        .route("/api/recall/clean", post(recall::api_clean))
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
        .route("/api/memories", get(memories::api_memories).post(memories::api_memory_add))
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
