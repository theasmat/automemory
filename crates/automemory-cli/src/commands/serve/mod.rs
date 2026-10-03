pub mod assets;
pub mod routes;
pub mod state;
pub mod ws;

use anyhow::{Context, Result};
use assets::static_asset_handler;
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_core::docs::DocumentWatcher;
use automemory_core::extraction::{ConsolidationWorker, ExtractionWorker};
use automemory_core::sync::AutoSyncDaemon;
use automemory_mcp::{mcp_router, McpServer};
use axum::{routing::get, Router};
use chrono::Utc;
use serde_json::json;
use state::AppState;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::CorsLayer;
use ws::ws_handler;

pub async fn execute(port: u16, db_path: Option<String>) -> Result<()> {
    let config = Config::default_config();
    let path = db_path.map(PathBuf::from).unwrap_or(config.db_path);

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
                let (mem_tokens, event_tokens) = pulse_db.get_token_stats(None).unwrap_or((0, 0));
                let events = pulse_db.list_recent_events(10).unwrap_or_default();
                let proposals = pulse_db.list_pending_proposals(None).unwrap_or_default();
                let traces = pulse_db.list_traces(None, 1000).unwrap_or_default();
                let failures = pulse_db.list_failures(None, Some("unresolved"), 1000).unwrap_or_default();

                let pulse = json!({
                    "type": "pulse",
                    "active_memories": active_mem,
                    "archived_memories": archived_mem,
                    "total_memory_tokens": mem_tokens,
                    "total_event_tokens": event_tokens,
                    "tokens_saved_estimate": event_tokens.saturating_sub(mem_tokens),
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

    // 7. Setup REST API and WebSocket router
    let api_app = routes::build_api_router()
        .route("/ws", get(ws_handler))
        .with_state(state);

    // Combine with static assets and CORS
    let app = Router::new()
        .merge(mcp_app)
        .merge(api_app)
        .fallback(static_asset_handler)
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    println!("┌────────────────────────────────────────────────────────┐");
    println!("│                AutoMemory Daemon Active                │");
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
