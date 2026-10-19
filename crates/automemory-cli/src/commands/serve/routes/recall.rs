//! Recall settings: retrieval mode, embedding profiles, API keys (write-only), test and indexing.
//! Every write needs the human password; no response ever contains a key.

use crate::commands::serve::routes::guardrails::require_human_header;
use crate::commands::serve::jobs;
use crate::commands::serve::state::AppState;
use automemory_core::config::{EmbeddingProfile, GlobalConfig};
use automemory_core::embeddings::{self, PRESETS};
use automemory_core::secrets;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

pub(super) fn gate(headers: &HeaderMap, action: &str) -> Option<Response> {
    require_human_header(headers, action).err().map(IntoResponse::into_response)
}

pub(super) fn bad(msg: impl Into<String>) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({ "error": msg.into() }))).into_response()
}

fn key_label(name: &str, env: &str) -> Value {
    json!({ "set": secrets::status(&format!("embeddings.{name}"), Some(env)) != secrets::Source::None,
            "where": secrets::status(&format!("embeddings.{name}"), Some(env)).label() })
}

fn profile_json(name: &str, p: &EmbeddingProfile) -> Value {
    json!({
        "name": name, "provider": p.provider, "model": p.model, "endpoint": p.endpoint,
        "dimensions": p.dimensions, "allow_network": p.allow_network,
        "needs_key": !matches!(p.provider.as_str(), "ngram" | "ollama" | "exec"),
        "key": key_label(name, &p.api_key_env),
    })
}

/// GET /api/recall
pub async fn api_get(State(state): State<AppState>) -> Response {
    let cfg = GlobalConfig::load().unwrap_or_default();
    let e = &cfg.embeddings;
    let flat = EmbeddingProfile {
        provider: e.provider.clone(), model: e.model.clone(), endpoint: e.endpoint.clone(),
        api_key_env: e.api_key_env.clone(), allow_network: e.allow_network, dimensions: e.dimensions, timeout_secs: e.timeout_secs,
    };
    let mut profiles = vec![profile_json("default", &flat)];
    profiles.extend(e.profiles.iter().map(|(n, p)| profile_json(n, p)));

    let resolved = e.resolved();
    let (coverage, problem) = match embeddings::provider_from_settings(e) {
        Ok(p) => {
            let (embedded, active, others) = state.db.vector_coverage(&p.model_id()).unwrap_or((0, 0, vec![]));
            let (pending, tokens) = embeddings::pending_estimate(&state.db, p.as_ref()).unwrap_or((0, 0));
            let cost = embeddings::price_per_million_tokens(&resolved.model).map(|per| tokens as f64 / 1e6 * per);
            (json!({ "model": p.model_id(), "local": p.is_local(), "embedded": embedded, "active": active, "pending": pending,
                     "estimated_tokens": tokens, "estimated_cost_usd": cost,
                     "unused": others.iter().map(|(m, n)| json!({ "model": m, "vectors": n })).collect::<Vec<_>>() }), None)
        }
        Err(err) => (Value::Null, Some(err.to_string())),
    };
    let idx = jobs::latest("index");
    let (running, done, total, error) = match &idx {
        Some(j) => (j.state == "running", j.done, j.total, if j.state == "failed" { j.message.clone() } else { None }),
        None => (false, 0, 0, None),
    };
    Json(json!({
        "mode": cfg.retrieval.mode, "active": resolved.active, "profiles": profiles,
        "presets": PRESETS.iter().map(|p| json!({ "id": p.id, "label": p.label, "provider": p.provider, "model": p.model,
            "dimensions": p.dimensions, "endpoint": p.endpoint, "local": p.local, "note": p.note })).collect::<Vec<_>>(),
        "coverage": coverage, "problem": problem, "max_tokens_per_day": e.max_tokens_per_day, "tokens_today": embeddings::tokens_today(&state.db),
        "indexing": { "running": running, "done": done, "total": total, "error": error },
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct SetPayload {
    pub mode: Option<String>,
    /// Profile to use ("default" for the built-in one).
    pub active: Option<String>,
}

/// PUT /api/recall
pub async fn api_put(headers: HeaderMap, Json(p): Json<SetPayload>) -> Response {
    if let Some(r) = gate(&headers, "change recall settings") {
        return r;
    }
    let mut cfg = GlobalConfig::load().unwrap_or_default();
    if let Some(m) = p.mode {
        if m != "keyword" && m != "hybrid" {
            return bad("mode must be keyword or hybrid");
        }
        cfg.retrieval.mode = m;
    }
    if let Some(a) = p.active {
        if a != "default" && !cfg.embeddings.profiles.contains_key(&a) {
            return bad(format!("no profile '{a}'"));
        }
        cfg.embeddings.active = if a == "default" { String::new() } else { a };
    }
    match cfg.save() {
        Ok(()) => Json(json!({ "success": true })).into_response(),
        Err(e) => bad(e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct ProfilePayload {
    pub name: String,
    pub preset: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub endpoint: Option<String>,
    pub dimensions: Option<usize>,
    pub allow_network: Option<bool>,
}

/// POST /api/recall/profile: add or replace a profile.
pub async fn api_profile_save(headers: HeaderMap, Json(p): Json<ProfilePayload>) -> Response {
    if let Some(r) = gate(&headers, "save an embedding profile") {
        return r;
    }
    if p.name == "default" || p.name.is_empty() || !p.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return bad("profile names use letters, digits, - and _ (and `default` is the built-in one)");
    }
    let mut prof = EmbeddingProfile { provider: "ngram".into(), model: String::new(), endpoint: String::new(), api_key_env: String::new(),
        allow_network: p.allow_network.unwrap_or(false), dimensions: p.dimensions, timeout_secs: 8 };
    if let Some(id) = &p.preset {
        let Some(pr) = PRESETS.iter().find(|x| x.id == id) else { return bad("unknown preset") };
        prof.provider = pr.provider.into();
        prof.model = pr.model.into();
        prof.endpoint = pr.endpoint.into();
        prof.dimensions = p.dimensions.or(pr.dimensions);
    }
    if let Some(v) = p.provider { prof.provider = v; }
    if let Some(v) = p.model { prof.model = v; }
    if let Some(v) = p.endpoint { prof.endpoint = v; }
    let mut cfg = GlobalConfig::load().unwrap_or_default();
    cfg.embeddings.profiles.insert(p.name.clone(), prof);
    let mut probe = cfg.embeddings.clone();
    probe.active = p.name.clone();
    if let Err(e) = embeddings::provider_from_settings(&probe) {
        let msg = e.to_string();
        if !msg.contains("embeddings key set") {
            return bad(msg);
        }
    }
    match cfg.save() {
        Ok(()) => Json(json!({ "success": true, "name": p.name })).into_response(),
        Err(e) => bad(e.to_string()),
    }
}

/// DELETE /api/recall/profile/:name
pub async fn api_profile_delete(headers: HeaderMap, Path(name): Path<String>) -> Response {
    if let Some(r) = gate(&headers, "remove an embedding profile") {
        return r;
    }
    let mut cfg = GlobalConfig::load().unwrap_or_default();
    if cfg.embeddings.profiles.remove(&name).is_none() {
        return bad("no such profile");
    }
    if cfg.embeddings.active == name {
        cfg.embeddings.active.clear();
    }
    let _ = secrets::delete(&format!("embeddings.{name}"));
    match cfg.save() {
        Ok(()) => Json(json!({ "success": true })).into_response(),
        Err(e) => bad(e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct KeyPayload {
    pub profile: String,
    pub key: String,
}

/// PUT /api/recall/key: write-only.
pub async fn api_key_set(headers: HeaderMap, Json(p): Json<KeyPayload>) -> Response {
    if let Some(r) = gate(&headers, "store an API key") {
        return r;
    }
    if p.key.trim().is_empty() {
        return bad("the key is empty");
    }
    match secrets::set(&format!("embeddings.{}", p.profile), p.key.trim()) {
        Ok(src) => Json(json!({ "success": true, "where": src.label() })).into_response(),
        Err(e) => bad(e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct ProfileQuery {
    pub profile: String,
}

/// DELETE /api/recall/key?profile=
pub async fn api_key_delete(headers: HeaderMap, Query(q): Query<ProfileQuery>) -> Response {
    if let Some(r) = gate(&headers, "remove an API key") {
        return r;
    }
    let removed = secrets::delete(&format!("embeddings.{}", q.profile)).unwrap_or(false);
    Json(json!({ "success": true, "removed": removed })).into_response()
}

/// POST /api/recall/test: embeds one sentence with the active profile (this may send text to a cloud service).
pub async fn api_test(headers: HeaderMap) -> Response {
    let cfg = GlobalConfig::load().unwrap_or_default();
    // A local model is Safe; a cloud model sends text out, so it needs the password.
    if !active_is_local(&cfg) {
        if let Some(r) = gate(&headers, "test the embedding provider") {
            return r;
        }
    }
    let out = tokio::task::spawn_blocking(move || {
        let provider = embeddings::provider_from_settings(&cfg.embeddings)?;
        let start = std::time::Instant::now();
        let v = provider.embed("automemory embedding self-test")?;
        Ok::<_, anyhow::Error>(json!({ "ok": true, "model": provider.model_id(), "dimensions": v.len(),
            "ms": start.elapsed().as_millis() as u64, "local": provider.is_local() }))
    })
    .await;
    match out {
        Ok(Ok(v)) => Json(v).into_response(),
        Ok(Err(e)) => Json(json!({ "ok": false, "error": e.to_string() })).into_response(),
        Err(e) => Json(json!({ "ok": false, "error": e.to_string() })).into_response(),
    }
}

#[derive(Deserialize, Default)]
pub struct IndexPayload {
    /// Recompute every vector of the active model.
    pub all: Option<bool>,
}

/// Whether the active profile keeps text on this computer (an unusable profile counts as not local).
fn active_is_local(cfg: &GlobalConfig) -> bool {
    embeddings::provider_from_settings(&cfg.embeddings).map(|p| p.is_local()).unwrap_or(false)
}

/// POST /api/recall/index: starts the background indexing job. A local model needs no password; a cloud model does.
pub async fn api_index(State(state): State<AppState>, headers: HeaderMap, body: Option<Json<IndexPayload>>) -> Response {
    let cfg = GlobalConfig::load().unwrap_or_default();
    if !active_is_local(&cfg) {
        if let Some(r) = gate(&headers, "index memories with a cloud model") {
            return r;
        }
    }
    let all = body.and_then(|b| b.0.all).unwrap_or(false);
    let db = Arc::clone(&state.db);
    let started = jobs::start(&state, "index", "Indexing memories", move |ctl| {
        let provider = embeddings::provider_from_settings(&cfg.embeddings)?;
        if all {
            embeddings::drop_model_vectors(&db, &provider.model_id())?;
        }
        let n = embeddings::index_with_progress(&db, provider.as_ref(), cfg.embeddings.max_tokens_per_day, ctl.flag(), &mut |done, total| {
            ctl.progress(done, total, Some(format!("{done} of {total} memories")));
        })?;
        Ok(format!("Indexed {n} memories with {}", provider.model_id()))
    });
    match started {
        Ok(id) => Json(json!({ "started": true, "job": id })).into_response(),
        Err(e) => bad(e),
    }
}

/// POST /api/recall/index/cancel
pub async fn api_index_cancel() -> Response {
    Json(json!({ "success": jobs::cancel_kind("index") })).into_response()
}

/// POST /api/recall/clean: remove vectors of models no longer in use.
pub async fn api_clean(State(state): State<AppState>) -> Response {
    let cfg = GlobalConfig::load().unwrap_or_default();
    match embeddings::provider_from_settings(&cfg.embeddings).and_then(|p| embeddings::remove_unused_vectors(&state.db, &p.model_id())) {
        Ok(n) => Json(json!({ "removed": n })).into_response(),
        Err(e) => bad(e.to_string()),
    }
}
