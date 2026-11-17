//! `/api/backup/git`: the git backup on the Sync page (docs/specs/git-backup.md). Status is read-only; setting it up and turning it off ask for the
//! password; "back up now" and "check the restore" only run what is already configured.

use super::control::{bad, protected};
use crate::commands::backup_git::{setup_core, turn_off, SetupIssue};
use crate::commands::serve::state::AppState;
use automemory_core::sync::backup_git as bg;
use axum::{extract::State, http::{HeaderMap, StatusCode}, response::{IntoResponse, Response}, Json};
use serde::Deserialize;
use serde_json::json;

pub async fn api_status(State(state): State<AppState>) -> impl IntoResponse {
    let st = bg::status(&state.db);
    let providers: Vec<_> = [bg::Provider::GitHub, bg::Provider::GitLab, bg::Provider::Bitbucket, bg::Provider::Other]
        .iter()
        .map(|p| json!({ "id": p, "name": p.name(), "hint": bg::create_hint(*p) }))
        .collect();
    Json(json!({ "status": st, "providers": providers }))
}

#[derive(Deserialize)]
pub struct SetupBody {
    pub url: String,
    #[serde(default)]
    pub allow_public: bool,
    #[serde(default)]
    pub allow_non_empty: bool,
    #[serde(default)]
    pub allow_secret: bool,
}

pub async fn api_setup(State(state): State<AppState>, headers: HeaderMap, Json(b): Json<SetupBody>) -> Response {
    if let Some(r) = protected(&state, &headers, "set up the git backup (sends encrypted data to a remote)") {
        return r;
    }
    let out = tokio::task::spawn_blocking(move || setup_core(&b.url, b.allow_public, b.allow_non_empty, b.allow_secret)).await;
    match out {
        Ok(Ok(ok)) => Json(json!({
            "ok": true,
            "provider": ok.provider,
            "remote": ok.remote,
            "recovery_phrase": ok.phrase,
            "verified": ok.verified,
            "note": ok.note,
        }))
        .into_response(),
        Ok(Err(issue)) => {
            let (code, msg) = match issue {
                SetupIssue::Invalid(m) => ("invalid", m),
                SetupIssue::Unreachable(m) => ("unreachable", m),
                SetupIssue::Public => ("public", "Anyone can read this repository. Your data is encrypted, but a backup repository should be private.".to_string()),
                SetupIssue::NotEmpty => ("not_empty", "This repository is not empty. If it holds an AutoMemory vault from another computer, restore from it with `automemory backup git restore`.".to_string()),
                SetupIssue::Secret => ("secret", "The address contains a password or token, which would be stored in plain text. An ssh address or a signed-in git credential helper is safer.".to_string()),
                SetupIssue::Failed(m) => ("failed", m),
            };
            (StatusCode::CONFLICT, Json(json!({ "ok": false, "code": code, "error": msg }))).into_response()
        }
        Err(_) => bad("the setup was interrupted"),
    }
}

pub async fn api_now(State(state): State<AppState>) -> impl IntoResponse {
    state.sync_daemon.trigger();
    Json(json!({ "status": "triggered" }))
}

pub async fn api_verify(State(state): State<AppState>) -> Response {
    let db = state.db.clone();
    let out = tokio::task::spawn_blocking(move || {
        let (_, remote) = automemory_core::sync::auto::vault_settings();
        let url = remote.ok_or_else(|| "no git backup is set up".to_string())?;
        let key = automemory_core::sync::auto::existing_key().ok_or_else(|| "this computer has no vault key".to_string())?;
        bg::verify_remote(&db, &url, &key).map_err(|e| e.to_string())
    })
    .await;
    match out {
        Ok(Ok(v)) => Json(json!({ "ok": true, "verification": v })).into_response(),
        Ok(Err(e)) => (StatusCode::CONFLICT, Json(json!({ "ok": false, "error": e }))).into_response(),
        Err(_) => bad("the check was interrupted"),
    }
}

pub async fn api_off(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(r) = protected(&state, &headers, "turn the git backup off") {
        return r;
    }
    match turn_off() {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => bad(e.to_string()),
    }
}
