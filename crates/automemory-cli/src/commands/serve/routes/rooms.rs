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
            json!({ "room": r, "posts": posts, "outcome": outcome, "needs_you": needs_you })
        })
        .collect();
    Json(out).into_response()
}

/// GET /api/rooms/:id: the discussion, and for a decided room the plan approving it would store.
pub async fn api_room(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let Ok(Some(room)) = state.db.get_room(&id) else { return bad("room not found") };
    let plan = state.db.room_plan(&id).ok();
    Json(json!({ "room": room, "posts": state.db.room_posts(&id).unwrap_or_default(), "outcome": state.db.room_outcome(&id), "plan": plan })).into_response()
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
