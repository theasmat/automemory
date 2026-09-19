use anyhow::{Context, Result};
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_core::docs::DocumentWatcher;
use automemory_core::extraction::{ConsolidationWorker, ExtractionWorker};
use automemory_core::models::*;
use automemory_core::retrieval::RetrievalEngine;
use automemory_core::sync::AutoSyncDaemon;
use automemory_mcp::{mcp_router, McpServer};
use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::CorsLayer;

#[derive(RustEmbed)]
#[folder = "src/dashboard/assets/"]
struct DashboardAssets;

#[derive(Clone)]
struct AppState {
    db: Arc<Database>,
    sync_daemon: Arc<AutoSyncDaemon>,
}

#[derive(Deserialize)]
struct SearchParams {
    q: Option<String>,
    project_id: Option<String>,
}

pub async fn execute(port: u16, db_path: Option<String>) -> Result<()> {
    let config = Config::default_config();
    let path = db_path.map(std::path::PathBuf::from).unwrap_or(config.db_path);

    let db = Arc::new(
        Database::open(&path)
            .with_context(|| format!("Failed to open database at {:?}", path))?,
    );

    // 1. Start background extraction worker
    let worker = Arc::new(ExtractionWorker::new(db.clone()));
    tokio::spawn(async move {
        worker.run_loop(Duration::from_secs(4)).await;
    });

    // 2. Start automated Git vault sync daemon
    let sync_daemon = Arc::new(AutoSyncDaemon::new(
        db.clone(),
        config.vault_dir.clone(),
        Duration::from_secs(30),
    ));
    let sync_clone = sync_daemon.clone();
    tokio::spawn(async move {
        sync_clone.run_loop().await;
    });

    // 3. Start Document Watcher & initial scan for registered projects
    let doc_watcher = DocumentWatcher::new(db.clone());
    if let Ok(projects) = db.list_projects() {
        let mut projects_with_paths = Vec::new();
        for p in projects {
            if let Ok(paths) = db.get_project_paths(&p.id) {
                for pp in paths {
                    let p_buf = PathBuf::from(&pp.local_path);
                    if p_buf.exists() && p_buf.is_dir() {
                        let _ = doc_watcher.scanner().scan_project(&p, &p_buf);
                        projects_with_paths.push((p.clone(), p_buf));
                    }
                }
            }
        }
        if !projects_with_paths.is_empty() {
            let _ = doc_watcher.start_watching(projects_with_paths);
        }
    }

    // 4. Start periodic consolidation worker (every 60 minutes)
    let consolidator = Arc::new(ConsolidationWorker::new(db.clone()));
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(3600));
        loop {
            ticker.tick().await;
            let _ = consolidator.consolidate(None);
        }
    });

    let state = AppState {
        db: db.clone(),
        sync_daemon: sync_daemon.clone(),
    };

    // 5. Setup MCP server router
    let mcp_server = Arc::new(McpServer::new(db.clone()));
    let mcp_app = mcp_router(mcp_server);

    // 6. Setup REST API & Dashboard router
    let api_app = Router::new()
        .route("/api/stats", get(api_stats))
        .route("/api/events", get(api_events))
        .route("/api/memories", get(api_memories))
        .route("/api/traces", get(api_traces))
        .route("/api/failures", get(api_failures))
        .route("/api/projects", get(api_projects))
        .route("/api/preferences", get(api_preferences))
        .route("/api/context", get(api_context))
        .route("/api/docs", get(api_docs))
        .route("/api/proposals", get(api_proposals))
        .route("/api/sync/trigger", post(api_sync_trigger))
        .with_state(state);

    // Combine with static assets and CORS
    let app = Router::new()
        .merge(mcp_app)
        .merge(api_app)
        .fallback(static_asset_handler)
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    println!("┌────────────────────────────────────────────────────────┐");
    println!("│              AutoMemory Pro Daemon Active              │");
    println!("├────────────────────────────────────────────────────────┤");
    println!("│  Dashboard Web UI: http://127.0.0.1:{:<5}              │", port);
    println!("│  MCP HTTP Gateway: http://127.0.0.1:{:<5}/mcp          │", port);
    println!("│  SQLite Database:  {:<35} │", path.to_string_lossy());
    println!("└────────────────────────────────────────────────────────┘");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn get_mime_type(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html"
    } else if path.ends_with(".css") {
        "text/css"
    } else if path.ends_with(".js") {
        "application/javascript"
    } else if path.ends_with(".json") {
        "application/json"
    } else {
        "application/octet-stream"
    }
}

async fn static_asset_handler(uri: axum::http::Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }

    match DashboardAssets::get(&path) {
        Some(content) => {
            let mime = get_mime_type(&path);
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, mime.parse().unwrap());
            (StatusCode::OK, headers, content.data).into_response()
        }
        None => match DashboardAssets::get("index.html") {
            Some(content) => {
                let mut headers = HeaderMap::new();
                headers.insert(header::CONTENT_TYPE, "text/html".parse().unwrap());
                (StatusCode::OK, headers, content.data).into_response()
            }
            None => (StatusCode::NOT_FOUND, "Not Found").into_response(),
        },
    }
}

async fn api_stats(State(state): State<AppState>) -> impl IntoResponse {
    let (active_mem, archived_mem) = state.db.count_memories(None).unwrap_or((0, 0));
    let traces = state.db.list_traces(None, 1000).unwrap_or_default();
    let failures = state.db.list_failures(None, Some("unresolved"), 1000).unwrap_or_default();
    let projects = state.db.list_projects().unwrap_or_default();

    Json(json!({
        "active_memories": active_mem,
        "archived_memories": archived_mem,
        "active_traces": traces.len(),
        "unresolved_failures": failures.len(),
        "total_projects": projects.len(),
    }))
}

async fn api_events(State(state): State<AppState>) -> impl IntoResponse {
    let events = state.db.list_recent_events(25).unwrap_or_default();
    Json(events)
}

async fn api_memories(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let limit = 50;
    if let Some(ref q) = params.q {
        let results = state.db.search_memories_fts(q, params.project_id.as_deref(), limit).unwrap_or_default();
        let memories: Vec<_> = results.into_iter().map(|(m, _)| m).collect();
        Json(memories)
    } else {
        let memories = state.db.list_memories(params.project_id.as_deref(), Some(MemoryStatus::Active), None, limit).unwrap_or_default();
        Json(memories)
    }
}

async fn api_traces(State(state): State<AppState>) -> impl IntoResponse {
    let traces = state.db.list_traces(None, 50).unwrap_or_default();
    Json(traces)
}

async fn api_failures(State(state): State<AppState>) -> impl IntoResponse {
    let failures = state.db.list_failures(None, None, 50).unwrap_or_default();
    Json(failures)
}

async fn api_projects(State(state): State<AppState>) -> impl IntoResponse {
    let projects = state.db.list_projects().unwrap_or_default();
    Json(projects)
}

async fn api_preferences(State(state): State<AppState>) -> impl IntoResponse {
    let prefs = state.db.list_preferences(None).unwrap_or_default();
    Json(prefs)
}

async fn api_context(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let engine = RetrievalEngine::new(&state.db);
    let ctx = engine
        .assemble_context(params.project_id.as_deref(), params.q.as_deref(), 3000)
        .unwrap_or_else(|e| format!("Error assembling context: {}", e));
    ctx
}

async fn api_docs(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    if let Some(pid) = params.project_id {
        let docs = state.db.list_documents(&pid).unwrap_or_default();
        Json(docs)
    } else {
        let mut all_docs = Vec::new();
        if let Ok(projects) = state.db.list_projects() {
            for p in projects {
                if let Ok(docs) = state.db.list_documents(&p.id) {
                    all_docs.extend(docs);
                }
            }
        }
        Json(all_docs)
    }
}

async fn api_proposals(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let proposals = state
        .db
        .list_pending_proposals(params.project_id.as_deref())
        .unwrap_or_default();
    Json(proposals)
}

async fn api_sync_trigger(State(state): State<AppState>) -> impl IntoResponse {
    state.sync_daemon.trigger();
    Json(json!({"status": "triggered", "message": "Automated sync scheduled"}))
}

