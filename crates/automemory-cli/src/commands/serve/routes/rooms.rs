//! Planning rooms: watch the discussion, see the plan it produced, and give the human verdict.
//! Reading is open; approving, rejecting or sending back for another round is Protected.
use super::control::{bad, protected};
use crate::commands::serve::state::AppState;
use automemory_core::db::queries::rooms::PlanStep;
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize, Default)]
pub struct RoomsQuery {
    pub project_id: Option<String>,
}

fn notify_room(state: &AppState, room_id: &str, event: &str) {
    let _ = state.broadcast_tx.send(
        json!({
            "type": "room_updated",
            "room_id": room_id,
            "event": event,
            "timestamp": Utc::now().to_rfc3339(),
        })
        .to_string(),
    );
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum StepInput {
    Title(String),
    Full(PlanStep),
}

impl StepInput {
    fn into_step(self) -> PlanStep {
        match self {
            Self::Title(title) => PlanStep {
                title,
                ..Default::default()
            },
            Self::Full(step) => step,
        }
    }
}

/// GET /api/rooms: every room with its post count and the human outcome, if any.
pub async fn api_rooms(State(state): State<AppState>, Query(q): Query<RoomsQuery>) -> Response {
    let rooms = match state.db.list_rooms(q.project_id.as_deref(), 50) {
        Ok(r) => r,
        Err(e) => return bad(e.to_string()),
    };
    let out: Vec<_> = rooms
        .into_iter()
        .map(|(r, posts)| {
            let outcome = state.db.room_outcome(&r.id);
            let needs_you = r.state == automemory_core::models::RoomState::Decided && outcome.is_none();
            let turn_stalled = state.db.is_room_turn_stalled(&r);
            let superseded_by = state.db.superseded_by_room(&r.id);
            let tasks = state.db.list_room_tasks(&r.id).unwrap_or_default();
            let labels: serde_json::Map<String, serde_json::Value> = r
                .members
                .iter()
                .map(|m| (m.clone(), json!(state.db.session_label(m))))
                .collect();
            json!({
                "room": r,
                "posts": posts,
                "outcome": outcome,
                "needs_you": needs_you,
                "turn_stalled": turn_stalled,
                "superseded_by": superseded_by,
                "task_count": tasks.len(),
                "tasks_done": tasks.iter().filter(|t| !t.cancelled && t.status == automemory_core::models::TaskState::Submitted).count(),
                "tasks_claimed": tasks.iter().filter(|t| !t.cancelled && t.status == automemory_core::models::TaskState::Claimed).count(),
                "labels": labels
            })
        })
        .collect();
    Json(out).into_response()
}

/// GET /api/rooms/:id: the discussion, and for a decided room the plan approving it would store.
pub async fn api_room(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let Ok(Some(room)) = state.db.get_room(&id) else {
        return bad("room not found");
    };
    let plan = state.db.room_plan(&id).ok();
    let posts = state.db.room_posts(&id).unwrap_or_default();
    let tasks = state.db.list_room_tasks(&id).unwrap_or_default();
    let active_leases = state
        .db
        .list_active_leases(room.project_id.as_deref())
        .unwrap_or_default();
    let locked_files: Vec<String> = active_leases.iter().map(|l| l.scope.clone()).collect();
    let labels: serde_json::Map<String, serde_json::Value> = room
        .members
        .iter()
        .chain(posts.iter().map(|p| &p.author_session))
        .chain(tasks.iter().filter_map(|t| t.owner_session.as_ref()))
        .map(|m| (m.clone(), json!(state.db.session_label(m))))
        .collect();
    Json(json!({
        "room": room,
        "posts": posts,
        "outcome": state.db.room_outcome(&id),
        "plan": plan,
        "draft": state.db.room_draft(&id),
        "tasks": tasks,
        "locked_files": locked_files,
        "turn_stalled": state.db.is_room_turn_stalled(&room),
        "superseded_by": state.db.superseded_by_room(&id),
        "labels": labels,
        "brief": state.db.room_meta(&id),
    }))
    .into_response()
}

pub async fn api_room_approve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Some(r) = protected(&state, &headers, "approve a planning room's plan") {
        return r;
    }
    match state.db.approve_room(&id) {
        Ok(o) => {
            notify_room(&state, &id, "approved");
            Json(json!({ "success": true, "outcome": o })).into_response()
        }
        Err(e) => bad(e.to_string()),
    }
}

pub async fn api_room_reject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Some(r) = protected(&state, &headers, "reject a planning room's plan") {
        return r;
    }
    match state.db.reject_room(&id) {
        Ok(o) => {
            notify_room(&state, &id, "rejected");
            Json(json!({ "success": true, "outcome": o })).into_response()
        }
        Err(e) => bad(e.to_string()),
    }
}

pub async fn api_room_again(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Some(r) = protected(&state, &headers, "send a planning room back for another round") {
        return r;
    }
    match state.db.another_round(&id) {
        Ok(room) => {
            notify_room(&state, &id, "another_round");
            Json(json!({ "success": true, "room": room })).into_response()
        }
        Err(e) => bad(e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct NotePayload {
    pub text: String,
}

/// POST /api/rooms/:id/note: your remark in the room; no turn, no state change.
pub async fn api_room_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(p): Json<NotePayload>,
) -> Response {
    if let Some(r) = protected(&state, &headers, "add a note to a planning room") {
        return r;
    }
    match state.db.add_room_note(&id, &p.text) {
        Ok(post) => {
            notify_room(&state, &id, "note");
            Json(json!({ "success": true, "post": post })).into_response()
        }
        Err(e) => bad(e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct DecidePayload {
    pub decision: String,
    #[serde(default)]
    pub steps: Vec<StepInput>,
}

/// POST /api/rooms/:id/decide: your decision, at any stage; stores the decision and one task per step.
pub async fn api_room_decide(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(p): Json<DecidePayload>,
) -> Response {
    if let Some(r) = protected(&state, &headers, "decide a planning room") {
        return r;
    }
    let steps: Vec<PlanStep> = p.steps.into_iter().map(StepInput::into_step).collect();
    match state.db.decide_room_steps(&id, &p.decision, &steps) {
        Ok(o) => {
            notify_room(&state, &id, "decided");
            Json(json!({ "success": true, "outcome": o })).into_response()
        }
        Err(e) => bad(e.to_string()),
    }
}

/// PUT /api/rooms/:id/draft: saves your edited decision and structured steps as a draft before approving.
pub async fn api_room_draft(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(p): Json<DecidePayload>,
) -> Response {
    if let Some(r) = protected(&state, &headers, "save a draft plan for a planning room") {
        return r;
    }
    let steps: Vec<PlanStep> = p.steps.into_iter().map(StepInput::into_step).collect();
    match state.db.save_room_draft(&id, &p.decision, &steps) {
        Ok(draft) => {
            notify_room(&state, &id, "draft");
            Json(json!({ "success": true, "draft": draft })).into_response()
        }
        Err(e) => bad(e.to_string()),
    }
}

/// POST /api/rooms/:id/revise: opens a follow-up room seeded with the approved room's decision.
pub async fn api_room_revise(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Some(r) = protected(&state, &headers, "revise an approved planning room") {
        return r;
    }
    match state.db.revise_room(&id) {
        Ok(room) => {
            notify_room(&state, &room.id, "revised");
            Json(json!({ "success": true, "room": room })).into_response()
        }
        Err(e) => bad(e.to_string()),
    }
}

/// POST /api/rooms/:id/skip: skips a stalled turn and passes the turn to the next member.
pub async fn api_room_skip(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Some(r) = protected(&state, &headers, "skip a stalled turn in a planning room") {
        return r;
    }
    match state.db.skip_room_turn(&id) {
        Ok(room) => {
            notify_room(&state, &id, "skipped");
            Json(json!({ "success": true, "room": room })).into_response()
        }
        Err(e) => bad(e.to_string()),
    }
}

/// POST /api/rooms/:id/end: closes a room without storing a decision or tasks.
pub async fn api_room_end(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Some(r) = protected(&state, &headers, "end a planning room without a plan") {
        return r;
    }
    match state.db.end_room(&id) {
        Ok(o) => {
            notify_room(&state, &id, "ended");
            Json(json!({ "success": true, "outcome": o })).into_response()
        }
        Err(e) => bad(e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct TaskUpdatePayload {
    pub title: Option<String>,
    pub assigned_agent: Option<String>,
}

/// PATCH /api/tasks/:id: rename a task or change its assigned agent.
pub async fn api_task_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(p): Json<TaskUpdatePayload>,
) -> Response {
    if let Some(r) = protected(&state, &headers, "update a room task") {
        return r;
    }
    match state
        .db
        .update_task(&id, p.title.as_deref(), p.assigned_agent.as_deref())
    {
        Ok(Some(task)) => {
            notify_room(&state, task.room_id.as_deref().unwrap_or(""), "task_updated");
            Json(json!({ "success": true, "task": task })).into_response()
        }
        Ok(None) => bad("task not found"),
        Err(e) => bad(e.to_string()),
    }
}

/// DELETE /api/tasks/:id: cancel an open or claimed task.
pub async fn api_task_cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Some(r) = protected(&state, &headers, "cancel a room task") {
        return r;
    }
    let room_id = state
        .db
        .get_task(&id)
        .ok()
        .flatten()
        .and_then(|t| t.room_id)
        .unwrap_or_default();
    match state.db.cancel_task(&id) {
        Ok(true) => {
            notify_room(&state, &room_id, "task_cancelled");
            Json(json!({ "success": true })).into_response()
        }
        Ok(false) => bad("task not found or already cancelled"),
        Err(e) => bad(e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct CreatePayload {
    pub topic: String,
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub agents: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub scope: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub rounds: Option<i64>,
    pub project_id: Option<String>,
}

/// POST /api/rooms: you start a room; the invited agents are told on their next prompt.
pub async fn api_room_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(p): Json<CreatePayload>,
) -> Response {
    if let Some(r) = protected(
        &state,
        &headers,
        &format!("start a planning room: {}", p.topic),
    ) {
        return r;
    }
    let project =
        super::control::project_dir_pub(&state, p.project_id.as_deref()).and_then(|(pid, _)| pid);
    let clean = |v: Vec<String>| {
        v.into_iter()
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .take(20)
            .collect::<Vec<_>>()
    };
    let meta = automemory_core::db::queries::rooms::RoomMeta {
        question: p.question,
        scope: clean(p.scope),
        constraints: clean(p.constraints),
        invited: clean(p.agents),
        evidence: clean(p.evidence),
        created_by: String::new(),
        rounds: p.rounds,
        parent_room: None,
    };
    match state
        .db
        .create_room_by_human(project.as_deref(), &p.topic, meta)
    {
        Ok(room) => {
            notify_room(&state, &room.id, "created");
            Json(json!({ "success": true, "room": room })).into_response()
        }
        Err(e) => bad(e.to_string()),
    }
}
