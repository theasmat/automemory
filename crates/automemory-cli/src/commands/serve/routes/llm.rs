//! `/api/llm*`: the optional suggesting model. Settings and keys are Protected; running a job is Effect
//! (a local model is Safe, a cloud one sends note text out, so it needs the password). Output is only Inbox proposals.
use super::recall::{bad, gate};
use crate::commands::serve::jobs;
use crate::commands::serve::state::AppState;
use axum::{extract::State, http::HeaderMap, response::{IntoResponse, Response}, Json};
use automemory_core::config::{GlobalConfig, LlmSettings};
use automemory_core::llm::{self, LlmClient};
use automemory_core::secrets;
use serde::Deserialize;
use serde_json::json;

fn key_info(secret: &str, env: &str) -> serde_json::Value {
    json!({ "set": secrets::status(secret, Some(env)) != secrets::Source::None, "where": secrets::status(secret, Some(env)).label() })
}

pub async fn api_get(State(state): State<AppState>) -> Response {
    let cfg = GlobalConfig::load().unwrap_or_default();
    let s = cfg.llm.resolved();
    let local_of = |settings: &automemory_core::config::LlmSettings, name: &str| {
        let mut one = settings.clone();
        one.active = if name == "default" { String::new() } else { name.to_string() };
        LlmClient::from_settings(&one).map(|c| c.is_local()).unwrap_or(false)
    };
    let mut profiles = vec![json!({ "name": "default", "model": cfg.llm.model, "endpoint": cfg.llm.endpoint, "api_key_env": cfg.llm.api_key_env,
        "allow_network": cfg.llm.allow_network, "local": local_of(&cfg.llm, "default"), "key": key_info(&LlmSettings::secret_for("default"), &cfg.llm.api_key_env) })];
    for (name, p) in &cfg.llm.profiles {
        profiles.push(json!({ "name": name, "model": p.model, "endpoint": p.endpoint, "api_key_env": p.api_key_env, "allow_network": p.allow_network,
            "local": local_of(&cfg.llm, name), "key": key_info(&LlmSettings::secret_for(name), &p.api_key_env) }));
    }
    let client = LlmClient::from_settings(&cfg.llm);
    Json(json!({
        "on": !s.model.is_empty(), "active": if cfg.llm.active.is_empty() { "default" } else { &cfg.llm.active },
        "model": s.model, "endpoint": s.endpoint, "api_key_env": s.api_key_env, "allow_network": s.allow_network,
        "max_tokens_per_day": cfg.llm.max_tokens_per_day, "tokens_today": llm::tokens_today(&state.db),
        "local": client.as_ref().map(|c| c.is_local()).unwrap_or(false),
        "problem": client.as_ref().err().filter(|_| !s.model.is_empty()).map(|e| e.to_string()),
        "key": key_info(&s.secret_name(), &s.api_key_env),
        "profiles": profiles,
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct LlmPayload {
    /// Which profile to edit (`default` or absent = the original one).
    pub name: Option<String>,
    pub model: Option<String>,
    pub endpoint: Option<String>,
    pub api_key_env: Option<String>,
    pub allow_network: Option<bool>,
    pub max_tokens_per_day: Option<u64>,
    /// Make this profile the active one.
    pub activate: Option<bool>,
    /// Turn the helper off entirely.
    pub off: Option<bool>,
}

fn valid_name(n: &str) -> bool {
    !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub async fn api_put(headers: HeaderMap, Json(p): Json<LlmPayload>) -> Response {
    if let Some(r) = gate(&headers, "change the LLM helper settings") {
        return r;
    }
    let mut cfg = GlobalConfig::load().unwrap_or_default();
    if p.off == Some(true) {
        cfg.llm.model.clear();
        cfg.llm.active.clear();
    } else {
        let name = p.name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| "default".into());
        if name == "default" {
            if let Some(v) = p.model { cfg.llm.model = v.trim().to_string() }
            if let Some(v) = p.endpoint { cfg.llm.endpoint = v.trim().to_string() }
            if let Some(v) = p.api_key_env { cfg.llm.api_key_env = v.trim().to_string() }
            if let Some(v) = p.allow_network { cfg.llm.allow_network = v }
        } else {
            if !valid_name(&name) { return bad("profile names use letters, digits, - and _"); }
            let timeout = cfg.llm.timeout_secs;
            let e = cfg.llm.profiles.entry(name.clone()).or_insert_with(|| automemory_core::config::LlmProfile { timeout_secs: timeout, ..Default::default() });
            if let Some(v) = p.model { e.model = v.trim().to_string() }
            if let Some(v) = p.endpoint { e.endpoint = v.trim().to_string() }
            if let Some(v) = p.api_key_env { e.api_key_env = v.trim().to_string() }
            if let Some(v) = p.allow_network { e.allow_network = v }
        }
        if p.activate == Some(true) {
            cfg.llm.active = if name == "default" { String::new() } else { name };
        }
    }
    if let Some(v) = p.max_tokens_per_day { cfg.llm.max_tokens_per_day = v }
    match cfg.save() { Ok(_) => Json(json!({ "success": true })).into_response(), Err(e) => bad(e.to_string()) }
}

/// DELETE /api/llm/profile/:name: removes a saved setup and its key.
pub async fn api_profile_delete(headers: HeaderMap, axum::extract::Path(name): axum::extract::Path<String>) -> Response {
    if let Some(r) = gate(&headers, "remove an LLM profile") {
        return r;
    }
    let mut cfg = GlobalConfig::load().unwrap_or_default();
    if cfg.llm.profiles.remove(&name).is_none() { return bad("no such profile"); }
    if cfg.llm.active == name { cfg.llm.active.clear(); }
    let _ = secrets::delete(&LlmSettings::secret_for(&name));
    match cfg.save() { Ok(_) => Json(json!({ "success": true })).into_response(), Err(e) => bad(e.to_string()) }
}

#[derive(Deserialize)]
pub struct KeyPayload { pub key: String, pub profile: Option<String> }

pub async fn api_key_set(headers: HeaderMap, Json(p): Json<KeyPayload>) -> Response {
    if let Some(r) = gate(&headers, "store the LLM API key") {
        return r;
    }
    if p.key.trim().is_empty() {
        return bad("the key is empty");
    }
    let secret = LlmSettings::secret_for(&p.profile.unwrap_or_else(|| GlobalConfig::load().unwrap_or_default().llm.active));
    match secrets::set(&secret, p.key.trim()) {
        Ok(src) => Json(json!({ "success": true, "where": src.label() })).into_response(),
        Err(e) => bad(e.to_string()),
    }
}

#[derive(Deserialize, Default)]
pub struct ProfileQuery { pub profile: Option<String> }

pub async fn api_key_delete(headers: HeaderMap, axum::extract::Query(q): axum::extract::Query<ProfileQuery>) -> Response {
    if let Some(r) = gate(&headers, "remove the LLM API key") {
        return r;
    }
    let secret = LlmSettings::secret_for(&q.profile.unwrap_or_else(|| GlobalConfig::load().unwrap_or_default().llm.active));
    Json(json!({ "success": true, "removed": secrets::delete(&secret).unwrap_or(false) })).into_response()
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
