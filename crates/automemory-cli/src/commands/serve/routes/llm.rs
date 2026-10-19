//! `/api/llm*`: the optional suggesting model. Settings and keys are Protected; running a job is Effect
//! (a local model is Safe, a cloud one sends note text out, so it needs the password). Output is only Inbox proposals.
use super::recall::{bad, gate};
use crate::commands::serve::jobs;
use crate::commands::serve::state::AppState;
use axum::{extract::State, http::HeaderMap, response::{IntoResponse, Response}, Json};
use automemory_core::config::GlobalConfig;
use automemory_core::llm::{self, LlmClient};
use automemory_core::secrets;
use serde::Deserialize;
use serde_json::json;

pub async fn api_get(State(state): State<AppState>) -> Response {
    let cfg = GlobalConfig::load().unwrap_or_default();
    let s = &cfg.llm;
    let client = LlmClient::from_settings(s);
    Json(json!({
        "on": !s.model.is_empty(), "model": s.model, "endpoint": s.endpoint, "api_key_env": s.api_key_env,
        "allow_network": s.allow_network, "max_tokens_per_day": s.max_tokens_per_day,
        "tokens_today": llm::tokens_today(&state.db),
        "local": client.as_ref().map(|c| c.is_local()).unwrap_or(false),
        "problem": client.as_ref().err().filter(|_| !s.model.is_empty()).map(|e| e.to_string()),
        "key": { "set": secrets::status(llm::SECRET_NAME, Some(&s.api_key_env)) != secrets::Source::None,
                 "where": secrets::status(llm::SECRET_NAME, Some(&s.api_key_env)).label() },
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct LlmPayload {
    pub model: Option<String>,
    pub endpoint: Option<String>,
    pub api_key_env: Option<String>,
    pub allow_network: Option<bool>,
    pub max_tokens_per_day: Option<u64>,
}

pub async fn api_put(headers: HeaderMap, Json(p): Json<LlmPayload>) -> Response {
    if let Some(r) = gate(&headers, "change the LLM helper settings") {
        return r;
    }
    let mut cfg = GlobalConfig::load().unwrap_or_default();
    if let Some(v) = p.model { cfg.llm.model = v.trim().to_string() }
    if let Some(v) = p.endpoint { cfg.llm.endpoint = v.trim().to_string() }
    if let Some(v) = p.api_key_env { cfg.llm.api_key_env = v.trim().to_string() }
    if let Some(v) = p.allow_network { cfg.llm.allow_network = v }
    if let Some(v) = p.max_tokens_per_day { cfg.llm.max_tokens_per_day = v }
    match cfg.save() { Ok(_) => Json(json!({ "success": true })).into_response(), Err(e) => bad(e.to_string()) }
}

#[derive(Deserialize)]
pub struct KeyPayload { pub key: String }

pub async fn api_key_set(headers: HeaderMap, Json(p): Json<KeyPayload>) -> Response {
    if let Some(r) = gate(&headers, "store the LLM API key") {
        return r;
    }
    if p.key.trim().is_empty() {
        return bad("the key is empty");
    }
    match secrets::set(llm::SECRET_NAME, p.key.trim()) {
        Ok(src) => Json(json!({ "success": true, "where": src.label() })).into_response(),
        Err(e) => bad(e.to_string()),
    }
}

pub async fn api_key_delete(headers: HeaderMap) -> Response {
    if let Some(r) = gate(&headers, "remove the LLM API key") {
        return r;
    }
    Json(json!({ "success": true, "removed": secrets::delete(llm::SECRET_NAME).unwrap_or(false) })).into_response()
}

#[derive(Deserialize)]
pub struct RunPayload { pub what: String }

pub async fn api_run(State(state): State<AppState>, headers: HeaderMap, Json(p): Json<RunPayload>) -> Response {
    if !matches!(p.what.as_str(), "pairs" | "distill") {
        return bad("unknown job: use pairs or distill");
    }
    let cfg = GlobalConfig::load().unwrap_or_default();
    let client = match LlmClient::from_settings(&cfg.llm) { Ok(c) => c, Err(e) => return bad(e.to_string()) };
    if !client.is_local() {
        if let Some(r) = gate(&headers, "send note text to the LLM") {
            return r;
        }
    }
    let (what, title) = (p.what.clone(), if p.what == "pairs" { "Looking for conflicts and duplicates" } else { "Suggesting notes from sessions" });
    // Runs in this process so progress and Stop work; the project is the one for the daemon's folder.
    match jobs::start(&state, "llm", title, move |ctl| {
        crate::commands::llm::run_job(&what, ctl.flag(), &mut |done, total| ctl.progress(done, total, Some(format!("{done} of {total}"))))
    }) {
        Ok(id) => Json(json!({ "started": true, "job": id })).into_response(),
        Err(e) => bad(e),
    }
}

pub async fn api_run_cancel() -> Response {
    Json(json!({ "success": jobs::cancel_kind("llm") })).into_response()
}
