use anyhow::{Context, Result};
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_core::docs::DocumentWatcher;
use automemory_core::extraction::{ConsolidationWorker, ExtractionWorker};
use automemory_core::graph::GraphBuilder;
use automemory_core::models::*;
use automemory_core::retrieval::RetrievalEngine;
use automemory_core::sync::AutoSyncDaemon;
use automemory_mcp::{mcp_router, McpServer};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::{header, HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::{json, Value};
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
    broadcast_tx: tokio::sync::broadcast::Sender<String>,
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

    let (broadcast_tx, _) = tokio::sync::broadcast::channel::<String>(256);

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

    // 5. Start real-time background pulse broadcaster
    let pulse_db = db.clone();
    let pulse_tx = broadcast_tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(2000));
        loop {
            interval.tick().await;
            if pulse_tx.receiver_count() > 0 {
                let (active_mem, archived_mem) = pulse_db.count_memories(None).unwrap_or((0, 0));
                let events = pulse_db.list_recent_events(10).unwrap_or_default();
                let proposals = pulse_db.list_pending_proposals(None).unwrap_or_default();
                let traces = pulse_db.list_traces(None, 1000).unwrap_or_default();
                let failures = pulse_db.list_failures(None, Some("unresolved"), 1000).unwrap_or_default();

                let pulse = json!({
                    "type": "pulse",
                    "active_memories": active_mem,
                    "archived_memories": archived_mem,
                    "active_traces": traces.len(),
                    "unresolved_failures": failures.len(),
                    "pending_proposals": proposals.len(),
                    "recent_events": events,
                    "timestamp": Utc::now().to_rfc3339(),
                });
                let _ = pulse_tx.send(pulse.to_string());
            }
        }
    });

    let state = AppState {
        db: db.clone(),
        sync_daemon: sync_daemon.clone(),
        broadcast_tx: broadcast_tx.clone(),
    };

    // 6. Setup MCP server router
    let mcp_server = Arc::new(McpServer::new(db.clone()));
    let mcp_app = mcp_router(mcp_server);

    // 7. Setup REST API, WebSocket & Dashboard router
    let api_app = Router::new()
        .route("/ws", get(ws_handler))
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
        .route("/api/proposals/decide", post(api_proposals_decide))
        .route("/api/graph", get(api_graph))
        .route("/api/stream", get(api_stream))
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
    println!("│  WebSocket Engine: ws://127.0.0.1:{:<5}/ws             │", port);
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
    } else if path.ends_with(".js") || path.ends_with(".mjs") {
        "application/javascript"
    } else if path.ends_with(".json") {
        "application/json"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else if path.ends_with(".woff2") {
        "font/woff2"
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

// ── WebSocket Handler ─────────────────────────────────────────────────────────

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_socket(socket, state))
}

async fn handle_ws_socket(mut socket: WebSocket, state: AppState) {
    let mut broadcast_rx = state.broadcast_tx.subscribe();

    loop {
        tokio::select! {
            // Outgoing broadcast message to this client
            recv_res = broadcast_rx.recv() => {
                match recv_res {
                    Ok(msg) => {
                        if socket.send(Message::Text(msg)).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // Client fell behind, ignore lagged frame
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
            // Incoming message from this client
            client_msg = socket.recv() => {
                match client_msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Some(resp) = handle_ws_json_rpc(&text, &state).await {
                            if socket.send(Message::Text(resp)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        if socket.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
}

#[derive(Deserialize)]
struct WsRequest {
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

async fn handle_ws_json_rpc(raw: &str, state: &AppState) -> Option<String> {
    let req: WsRequest = match serde_json::from_str(raw) {
        Ok(r) => r,
        Err(_) => return None,
    };

    let req_id = req.id.unwrap_or(Value::Null);

    let result = match req.method.as_str() {
        "ping" => json!({ "status": "pong", "timestamp": Utc::now().to_rfc3339() }),
        "stats" => {
            let (active_mem, archived_mem) = state.db.count_memories(None).unwrap_or((0, 0));
            let traces = state.db.list_traces(None, 1000).unwrap_or_default();
            let failures = state.db.list_failures(None, Some("unresolved"), 1000).unwrap_or_default();
            let projects = state.db.list_projects().unwrap_or_default();
            let proposals = state.db.list_pending_proposals(None).unwrap_or_default();
            json!({
                "active_memories": active_mem,
                "archived_memories": archived_mem,
                "active_traces": traces.len(),
                "unresolved_failures": failures.len(),
                "total_projects": projects.len(),
                "pending_proposals": proposals.len(),
            })
        }
        "events" => {
            let limit = req.params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
            let events = state.db.list_recent_events(limit).unwrap_or_default();
            json!(events)
        }
        "memories" => {
            let project_id = req.params.get("project_id").and_then(|v| v.as_str());
            let query = req.params.get("q").and_then(|v| v.as_str());
            let limit = req.params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

            if let Some(q) = query {
                let results = state.db.search_memories_fts(q, project_id, limit).unwrap_or_default();
                let memories: Vec<_> = results.into_iter().map(|(m, _)| m).collect();
                json!(memories)
            } else {
                let memories = state.db.list_memories(project_id, Some(MemoryStatus::Active), None, limit).unwrap_or_default();
                json!(memories)
            }
        }
        "traces" => {
            let project_id = req.params.get("project_id").and_then(|v| v.as_str());
            let limit = req.params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
            let traces = state.db.list_traces(project_id, limit).unwrap_or_default();
            json!(traces)
        }
        "failures" => {
            let project_id = req.params.get("project_id").and_then(|v| v.as_str());
            let status = req.params.get("status").and_then(|v| v.as_str());
            let limit = req.params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
            let failures = state.db.list_failures(project_id, status, limit).unwrap_or_default();
            json!(failures)
        }
        "projects" => {
            let projects = state.db.list_projects().unwrap_or_default();
            json!(projects)
        }
        "preferences" => {
            let project_id = req.params.get("project_id").and_then(|v| v.as_str());
            let prefs = state.db.list_preferences(project_id).unwrap_or_default();
            json!(prefs)
        }
        "docs" => {
            let project_id = req.params.get("project_id").and_then(|v| v.as_str());
            if let Some(pid) = project_id {
                let docs = state.db.list_documents(pid).unwrap_or_default();
                json!(docs)
            } else {
                let mut all_docs = Vec::new();
                if let Ok(projects) = state.db.list_projects() {
                    for p in projects {
                        if let Ok(docs) = state.db.list_documents(&p.id) {
                            all_docs.extend(docs);
                        }
                    }
                }
                json!(all_docs)
            }
        }
        "proposals" => {
            let project_id = req.params.get("project_id").and_then(|v| v.as_str());
            let proposals = state.db.list_pending_proposals(project_id).unwrap_or_default();
            json!(proposals)
        }
        "decide_proposal" => {
            let id = req.params.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let approved = req.params.get("approved").and_then(|v| v.as_bool()).unwrap_or(false);
            match state.db.decide_proposal(id, approved) {
                Ok(true) => {
                    let _ = state.broadcast_tx.send(json!({
                        "type": "proposal_decided",
                        "id": id,
                        "approved": approved,
                        "timestamp": Utc::now().to_rfc3339(),
                    }).to_string());
                    json!({ "success": true, "id": id, "approved": approved })
                }
                Ok(false) => json!({ "error": "Proposal not found" }),
                Err(e) => json!({ "error": e.to_string() }),
            }
        }
        "graph" => {
            let project_id = req.params.get("project_id").and_then(|v| v.as_str());
            let limit_entities = req.params.get("limit_entities").and_then(|v| v.as_u64()).unwrap_or(400) as usize;
            let limit_edges = req.params.get("limit_edges").and_then(|v| v.as_u64()).unwrap_or(800) as usize;

            let entities = state.db.list_all_graph_entities(project_id, limit_entities).unwrap_or_default();
            let edges = state.db.list_all_graph_edges(limit_edges).unwrap_or_default();
            json!({
                "entities": entities,
                "edges": edges,
            })
        }
        "trace_impact" => {
            let project_id = req.params.get("project_id").and_then(|v| v.as_str());
            let target = req.params.get("target").and_then(|v| v.as_str()).unwrap_or("");
            let depth = req.params.get("depth").and_then(|v| v.as_u64()).unwrap_or(3) as usize;

            let builder = GraphBuilder::new(&state.db);
            match builder.trace_impact(project_id, target, depth) {
                Ok(Some(report)) => json!({ "success": true, "report": report }),
                Ok(None) => json!({ "success": false, "message": "Target entity not found in graph" }),
                Err(e) => json!({ "success": false, "error": e.to_string() }),
            }
        }
        "assemble_context" => {
            let project_id = req.params.get("project_id").and_then(|v| v.as_str());
            let query = req.params.get("q").and_then(|v| v.as_str());
            let budget = req.params.get("token_budget").and_then(|v| v.as_u64()).unwrap_or(3000) as usize;

            let engine = RetrievalEngine::new(&state.db);
            let ctx = engine.assemble_context(project_id, query, budget).unwrap_or_default();
            json!({ "context": ctx })
        }
        "trigger_sync" => {
            state.sync_daemon.trigger();
            json!({ "status": "triggered", "message": "Automated git vault sync triggered" })
        }
        unknown => json!({ "error": format!("Unknown method: {}", unknown) }),
    };

    Some(
        json!({
            "id": req_id,
            "result": result,
        })
        .to_string(),
    )
}

// ── REST API Endpoints ────────────────────────────────────────────────────────

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

#[derive(Deserialize)]
struct DecideProposalPayload {
    id: String,
    approved: bool,
}

async fn api_proposals_decide(
    State(state): State<AppState>,
    Json(payload): Json<DecideProposalPayload>,
) -> impl IntoResponse {
    match state.db.decide_proposal(&payload.id, payload.approved) {
        Ok(true) => {
            let _ = state.broadcast_tx.send(json!({
                "type": "proposal_decided",
                "id": payload.id,
                "approved": payload.approved,
                "timestamp": Utc::now().to_rfc3339(),
            }).to_string());
            (
                StatusCode::OK,
                Json(json!({ "success": true, "id": payload.id, "approved": payload.approved })),
            )
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Proposal not found" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

async fn api_graph(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let entities = state
        .db
        .list_all_graph_entities(params.project_id.as_deref(), 300)
        .unwrap_or_default();
    let edges = state.db.list_all_graph_edges(500).unwrap_or_default();

    Json(json!({
        "entities": entities,
        "edges": edges,
    }))
}

async fn api_stream(State(state): State<AppState>) -> Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let stream = async_stream::stream! {
        let mut interval = tokio::time::interval(Duration::from_millis(2500));
        loop {
            interval.tick().await;
            let (active_mem, _) = state.db.count_memories(None).unwrap_or((0, 0));
            let events = state.db.list_recent_events(10).unwrap_or_default();
            let proposals = state.db.list_pending_proposals(None).unwrap_or_default();

            let payload = json!({
                "type": "pulse",
                "active_memories": active_mem,
                "recent_events": events,
                "pending_proposals": proposals.len(),
                "timestamp": Utc::now().to_rfc3339(),
            });

            yield Ok(Event::default().data(payload.to_string()));
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn api_sync_trigger(State(state): State<AppState>) -> impl IntoResponse {
    state.sync_daemon.trigger();
    Json(json!({"status": "triggered", "message": "Automated sync scheduled"}))
}
