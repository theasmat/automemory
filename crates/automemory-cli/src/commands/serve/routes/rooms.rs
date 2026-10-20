//! Planning rooms: watch the discussion, see the plan it produced, and give the human verdict.
//! Reading is open; approving, rejecting or sending back for another round is Protected.
use super::control::{bad, protected};
use crate::commands::serve::state::AppState;
use axum::{extract::{Path, Query, State}, http::HeaderMap, response::{IntoResponse, Response}, Json};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize, Default)]
pub struct RoomsQuery { pub project_id: Option<String> }

/// GET /api/rooms: every room with its post count and the human outcome, if any.
pub async fn api_rooms(State(state): State<AppState>, Query(q): Query<RoomsQuery>) -> Response {
    let rooms = match state.db.list_rooms(q.project_id.as_deref(), 50) { Ok(r) => r, Err(e) => return bad(e.to_string()) };
    let out: Vec<_> = rooms
        .into_iter()
        .map(|(r, posts)| {
            let outcome = state.db.room_outcome(&r.id);
            let needs_you = r.state == automemory_core::models::RoomState::Decided && outcome.is_none();
            let labels: serde_json::Map<String, serde_json::Value> = r.members.iter().map(|m| (m.clone(), json!(state.db.session_label(m)))).collect();
            json!({ "room": r, "posts": posts, "outcome": outcome, "needs_you": needs_you, "labels": labels })
        })
        .collect();
    Json(out).into_response()
}

/// GET /api/rooms/:id: the discussion, and for a decided room the plan approving it would store.
pub async fn api_room(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let Ok(Some(room)) = state.db.get_room(&id) else { return bad("room not found") };
    let plan = state.db.room_plan(&id).ok();
    let posts = state.db.room_posts(&id).unwrap_or_default();
    let labels: serde_json::Map<String, serde_json::Value> = room.members.iter().chain(posts.iter().map(|p| &p.author_session))
        .map(|m| (m.clone(), json!(state.db.session_label(m)))).collect();
    Json(json!({ "room": room, "posts": posts, "outcome": state.db.room_outcome(&id), "plan": plan, "labels": labels })).into_response()
}

pub async fn api_room_approve(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    if let Some(r) = protected(&state, &headers, "approve a planning room's plan") { return r; }
    match state.db.approve_room(&id) { Ok(o) => Json(json!({ "success": true, "outcome": o })).into_response(), Err(e) => bad(e.to_string()) }
}

pub async fn api_room_reject(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    if let Some(r) = protected(&state, &headers, "reject a planning room's plan") { return r; }
    match state.db.reject_room(&id) { Ok(o) => Json(json!({ "success": true, "outcome": o })).into_response(), Err(e) => bad(e.to_string()) }
}

pub async fn api_room_again(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    if let Some(r) = protected(&state, &headers, "send a planning room back for another round") { return r; }
    match state.db.another_round(&id) { Ok(room) => Json(json!({ "success": true, "room": room })).into_response(), Err(e) => bad(e.to_string()) }
}

#[derive(Deserialize)]
pub struct NotePayload { pub text: String }

/// POST /api/rooms/:id/note: your remark in the room; no turn, no state change.
pub async fn api_room_note(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(p): Json<NotePayload>) -> Response {
    if let Some(r) = protected(&state, &headers, "add a note to a planning room") { return r; }
    match state.db.add_room_note(&id, &p.text) { Ok(post) => Json(json!({ "success": true, "post": post })).into_response(), Err(e) => bad(e.to_string()) }
}

#[derive(Deserialize)]
pub struct DecidePayload { pub decision: String, #[serde(default)] pub steps: Vec<String> }

/// POST /api/rooms/:id/decide: your decision, at any stage; stores the decision and one task per step.
pub async fn api_room_decide(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Json(p): Json<DecidePayload>) -> Response {
    if let Some(r) = protected(&state, &headers, "decide a planning room") { return r; }
    match state.db.decide_room(&id, &p.decision, &p.steps) { Ok(o) => Json(json!({ "success": true, "outcome": o })).into_response(), Err(e) => bad(e.to_string()) }
}
