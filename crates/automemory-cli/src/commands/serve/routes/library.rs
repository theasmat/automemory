//! Library page: documents for agents kept outside git. Reading and writing the copies in the project folder are Safe; adding, editing,
//! removing, reverting and choosing which files are kept in place are Protected (they decide what agents are shown as the person's words).
use super::control::{bad, project_dir_pub, protected};
use crate::commands::serve::state::AppState;
use automemory_core::config::GlobalConfig;
use automemory_core::library::{fs as libfs, NewFile, Origin, PutOutcome};
use axum::{extract::{Query, State}, http::HeaderMap, response::{IntoResponse, Response}, Json};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize, Default)]
pub struct ProjectQuery { pub project_id: Option<String> }

fn here(state: &AppState, project: Option<&str>) -> Option<(String, String, std::path::PathBuf)> {
    let (pid, root) = project_dir_pub(state, project)?;
    let pid = pid?;
    let slug = state.db.get_project_by_id(&pid).ok().flatten().map(|p| p.slug).unwrap_or_default();
    Some((pid, slug, root))
}

/// GET /api/library: the files, the folder's state (is it out of git status?), problems and the kept-in-place patterns.
pub async fn api_overview(State(state): State<AppState>, Query(q): Query<ProjectQuery>) -> Response {
    let Some((pid, slug, root)) = here(&state, q.project_id.as_deref()) else { return Json(json!({ "project": null })).into_response() };
    let cfg = GlobalConfig::load().unwrap_or_default();
    let files = state.db.library_list(Some(&pid), None).unwrap_or_default();
    let folder = libfs::folder(&root);
    let ignored = folder.exists() && std::process::Command::new("git").arg("-C").arg(&root).args(["check-ignore", "-q", ".automemory/library/INDEX.md"]).status().map(|s| s.success()).unwrap_or(false);
    Json(json!({
        "project": pid, "slug": slug, "root": root.to_string_lossy(),
        "files": files.iter().map(|f| json!({ "path": f.path, "summary": f.l0, "size": f.size, "linked": f.linked, "personal": f.scope == "personal", "origin": f.origin, "author": f.author, "updated_at": f.updated_at, "revisions": f.revisions })).collect::<Vec<_>>(),
        "folder": { "path": folder.to_string_lossy(), "exists": folder.exists(), "out_of_git_status": ignored, "exclude_from_git": cfg.library.exclude_from_git },
        "project_docs": automemory_core::library::repo_docs::list(&state.db, &pid).unwrap_or_default(),
        "problems": libfs::check(&state.db, &pid, &slug, &root, &cfg.library),
        "patterns": { "global": cfg.library.files, "project": cfg.library.projects.get(&slug).map(|p| p.files.clone()).unwrap_or_default() },
    })).into_response()
}

#[derive(Deserialize)]
pub struct FileQuery { pub path: String, pub project_id: Option<String> }

/// GET /api/library/file?path=: the file with its summary, outline and text, and its revisions.
pub async fn api_file(State(state): State<AppState>, Query(q): Query<FileQuery>) -> Response {
    let Some((pid, _, _)) = here(&state, q.project_id.as_deref()) else { return bad("no project yet") };
    match state.db.library_get(Some(&pid), &q.path) {
        Ok(Some(f)) => Json(json!({
            "path": f.path, "summary": f.l0, "outline": f.l1, "content": f.content, "linked": f.linked, "personal": f.scope == "personal", "origin": f.origin, "author": f.author, "updated_at": f.updated_at,
            "revisions": state.db.library_revisions(Some(&pid), &f.path).unwrap_or_default().iter().map(|(rev, author, origin, at)| json!({ "rev": rev, "author": author, "origin": origin, "at": at })).collect::<Vec<_>>(),
        })).into_response(),
        Ok(None) => {
            // One of the project's own documents in the repository: read-only here, git is its home.
            let docs = automemory_core::library::repo_docs::list(&state.db, &pid).unwrap_or_default();
            let Some(d) = docs.iter().find(|d| d.path == q.path) else { return bad("no such file") };
            let get = |tier| automemory_core::library::repo_docs::get(&state.db, &pid, &q.path, tier).ok().flatten().unwrap_or_default();
            Json(json!({
                "path": d.path, "summary": get(0), "outline": get(1), "content": get(2), "linked": false, "personal": false, "origin": "repo", "author": d.category,
                "source": "repo", "updated_at": null, "revisions": [],
            })).into_response()
        }
        Err(e) => bad(e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct RevisionQuery { pub path: String, pub rev: i64, pub project_id: Option<String> }

/// GET /api/library/revision?path=&rev=: the text of an older revision.
pub async fn api_revision(State(state): State<AppState>, Query(q): Query<RevisionQuery>) -> Response {
    let Some((pid, _, _)) = here(&state, q.project_id.as_deref()) else { return bad("no project yet") };
    match state.db.library_revision_text(Some(&pid), &q.path, q.rev) {
        Ok(Some(t)) => Json(json!({ "content": t })).into_response(),
        Ok(None) => bad("no such revision"),
        Err(e) => bad(e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct PutPayload { pub path: String, pub content: String, #[serde(default)] pub personal: bool, #[serde(default)] pub allow_secrets: bool, pub project_id: Option<String> }

fn put_result(r: anyhow::Result<(automemory_core::library::LibraryFile, PutOutcome)>) -> Response {
    match r {
        Ok((f, o)) => Json(json!({ "success": true, "path": f.path, "outcome": format!("{o:?}").to_lowercase(), "summary": f.l0 })).into_response(),
        Err(e) => bad(e.to_string()),
    }
}

/// PUT /api/library/file (Protected): add or replace a file (your words). Kept-in-place files are edited on disk, not here.
pub async fn api_put(State(state): State<AppState>, headers: HeaderMap, Json(p): Json<PutPayload>) -> Response {
    if let Some(r) = protected(&state, &headers, "write to the project library") { return r; }
    let Some((pid, _, _)) = here(&state, p.project_id.as_deref()) else { return bad("no project yet") };
    if state.db.library_get(Some(&pid), &p.path).ok().flatten().is_some_and(|f| f.linked) {
        return bad("this file is kept in place in the project folder: edit it there");
    }
    put_result(state.db.library_put(NewFile { project_id: Some(pid), path: p.path, content: p.content, origin: Origin::Human, author: "human".into(), personal: p.personal, allow_secrets: p.allow_secrets, linked: false }))
}

/// DELETE /api/library/file?path= (Protected): remove it from the library (the removal reaches your other computers).
pub async fn api_remove(State(state): State<AppState>, headers: HeaderMap, Query(q): Query<FileQuery>) -> Response {
    if let Some(r) = protected(&state, &headers, "remove a file from the project library") { return r; }
    let Some((pid, _, _)) = here(&state, q.project_id.as_deref()) else { return bad("no project yet") };
    match state.db.library_remove(Some(&pid), &q.path) {
        Ok(true) => Json(json!({ "success": true })).into_response(),
        Ok(false) => bad("no such file"),
        Err(e) => bad(e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct RevertPayload { pub path: String, pub rev: i64, pub project_id: Option<String> }

/// POST /api/library/revert (Protected): make an older revision the current text (as a new revision).
pub async fn api_revert(State(state): State<AppState>, headers: HeaderMap, Json(p): Json<RevertPayload>) -> Response {
    if let Some(r) = protected(&state, &headers, "restore an older version of a library file") { return r; }
    let Some((pid, _, _)) = here(&state, p.project_id.as_deref()) else { return bad("no project yet") };
    let Some(f) = state.db.library_get(Some(&pid), &p.path).ok().flatten() else { return bad("no such file") };
    if f.linked {
        return bad("this file is kept in place in the project folder: restore it there");
    }
    match state.db.library_revision_text(Some(&pid), &p.path, p.rev) {
        Ok(Some(text)) => put_result(state.db.library_put(NewFile { project_id: Some(pid), path: p.path, content: text, origin: Origin::Human, author: "human".into(), personal: f.scope == "personal", allow_secrets: true, linked: false })),
        Ok(None) => bad("no such revision"),
        Err(e) => bad(e.to_string()),
    }
}

/// POST /api/library/sync: write the copies and INDEX.md, take edits back, read kept-in-place files.
pub async fn api_sync(State(state): State<AppState>, Query(q): Query<ProjectQuery>) -> Response {
    let Some((pid, slug, root)) = here(&state, q.project_id.as_deref()) else { return bad("no project yet") };
    let cfg = GlobalConfig::load().unwrap_or_default();
    match libfs::sync_project(&state.db, &pid, &slug, &root, &cfg.library) {
        Ok(r) => Json(json!({ "success": true, "report": r })).into_response(),
        Err(e) => bad(e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct LinkPayload { pub pattern: String, #[serde(default)] pub project_only: bool, pub project_id: Option<String> }

fn ok_pattern(p: &str) -> bool {
    !p.trim().is_empty() && !p.starts_with('/') && !p.contains("..") && !p.contains(':')
}

/// POST /api/library/link (Protected): keep files matching a pattern in sync where they are (git tracked, ignored or not).
pub async fn api_link(State(state): State<AppState>, headers: HeaderMap, Json(p): Json<LinkPayload>) -> Response {
    if !ok_pattern(&p.pattern) {
        return bad("use a path or pattern relative to the project folder, such as PRD-private.md or plans/**/*.md");
    }
    if let Some(r) = protected(&state, &headers, "change which files the library keeps in sync") { return r; }
    let Some((_, slug, _)) = here(&state, p.project_id.as_deref()) else { return bad("no project yet") };
    let mut cfg = GlobalConfig::load().unwrap_or_default();
    let list = if p.project_only { &mut cfg.library.projects.entry(slug).or_default().files } else { &mut cfg.library.files };
    if !list.contains(&p.pattern) {
        list.push(p.pattern);
    }
    match cfg.save() { Ok(_) => Json(json!({ "success": true })).into_response(), Err(e) => bad(e.to_string()) }
}

#[derive(Deserialize)]
pub struct UnlinkQuery { pub pattern: String }

/// DELETE /api/library/link?pattern= (Protected): stop keeping a pattern in sync (files already in the library stay).
pub async fn api_unlink(State(state): State<AppState>, headers: HeaderMap, Query(q): Query<UnlinkQuery>) -> Response {
    if let Some(r) = protected(&state, &headers, "change which files the library keeps in sync") { return r; }
    let mut cfg = GlobalConfig::load().unwrap_or_default();
    cfg.library.files.retain(|x| *x != q.pattern);
    for p in cfg.library.projects.values_mut() {
        p.files.retain(|x| *x != q.pattern);
    }
    match cfg.save() { Ok(_) => Json(json!({ "success": true })).into_response(), Err(e) => bad(e.to_string()) }
}

