pub mod assets;
pub mod jobs;
pub mod policy;
mod routes;
pub mod state;
pub mod ws;

use anyhow::{Context, Result};
use assets::static_asset_handler;
use automemory_core::config::{Config, GlobalConfig};
use automemory_core::db::Database;
use automemory_core::docs::DocumentWatcher;
use automemory_core::extraction::{ConsolidationWorker, ExtractionWorker};
use automemory_core::sync::AutoSyncDaemon;
use automemory_mcp::{mcp_router, McpServer};
use axum::{response::IntoResponse, routing::get, Router};
use chrono::Utc;
use serde_json::json;
use state::AppState;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use ws::ws_handler;

pub async fn execute(port: u16, db_path: Option<String>) -> Result<()> {
    let config = Config::default_config();
    let path = db_path.map(PathBuf::from).unwrap_or(config.db_path);

    let db = Arc::new(
        Database::open(&path).with_context(|| format!("Failed to open database at {:?}", path))?,
    );

    let (broadcast_tx, _) = tokio::sync::broadcast::channel::<String>(256);

    // 1. Start background extraction worker
    let worker = Arc::new(ExtractionWorker::new(db.clone()));
    let auto_skills = GlobalConfig::load().map(|c| c.skills.auto).unwrap_or(false);
    tokio::spawn(async move {
        worker.run_loop(Duration::from_secs(4), auto_skills).await;
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

    // 3b. Watch project folders for changes under an idle holder's lease (agents without hooks)
    {
        let mut roots = Vec::new();
        for p in db.list_projects().unwrap_or_default() {
            for pp in db.get_project_paths(&p.id).unwrap_or_default() {
                let dir = PathBuf::from(&pp.local_path);
                if dir.is_dir() {
                    roots.push((Some(p.id.clone()), dir));
                }
            }
        }
        if !roots.is_empty() {
            let tx = broadcast_tx.clone();
            let settings = automemory_core::config::GlobalConfig::load()
                .map(|g| g.locks)
                .unwrap_or_default();
            let _ = automemory_core::lockwatch::start(db.clone(), settings, roots, move |msg| {
                let _ = tx.send(json!({ "type": "lock_alert", "message": msg, "timestamp": Utc::now().to_rfc3339() }).to_string());
            });
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
                let failures = pulse_db
                    .list_failures(None, Some("unresolved"), 1000)
                    .unwrap_or_default();

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
        .layer(axum::middleware::from_fn_with_state(
            port,
            local_origin_guard,
        ));

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    println!("┌────────────────────────────────────────────────────────┐");
    println!("│                AutoMemory Daemon Active                │");
    println!("├────────────────────────────────────────────────────────┤");
    println!(
        "│  Dashboard Web UI: http://127.0.0.1:{:<5}              │",
        port
    );
    println!(
        "│  WebSocket Engine: ws://127.0.0.1:{:<5}/ws             │",
        port
    );
    println!(
        "│  MCP HTTP Gateway: http://127.0.0.1:{:<5}/mcp          │",
        port
    );
    println!("│  SQLite Database:  {:<35} │", path.to_string_lossy());
    println!("└────────────────────────────────────────────────────────┘");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Blocks browser-driven attacks on the localhost API. A web page the user
/// visits can POST to 127.0.0.1 (CSRF) or reach us through a rebinding DNS
/// name, so: the `Host` must be our own loopback address, and any `Origin`
/// (browsers always send one on cross-site requests) must be the dashboard
/// itself. Non-browser clients (curl, MCP over HTTP) send no `Origin` and pass.
async fn local_origin_guard(
    axum::extract::State(port): axum::extract::State<u16>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::http::{header, StatusCode};
    let allowed = |v: &str, scheme: &str| {
        v == format!("{scheme}127.0.0.1:{port}") || v == format!("{scheme}localhost:{port}")
    };
    let host_ok = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|h| allowed(h, ""));
    let origin_ok = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .is_none_or(|o| allowed(o, "http://"));
    if host_ok && origin_ok {
        next.run(req).await
    } else {
        (
            StatusCode::FORBIDDEN,
            "AutoMemory: cross-origin request blocked",
        )
            .into_response()
    }
}

#[cfg(test)]
mod guard_tests {
    use super::*;
    use axum::{body::Body, http::Request, routing::post, Router};
    use tower::ServiceExt;

    fn app() -> Router {
        Router::new().route("/x", post(|| async { "ok" })).layer(
            axum::middleware::from_fn_with_state(4545u16, local_origin_guard),
        )
    }

    async fn status(host: &str, origin: Option<&str>) -> u16 {
        let mut b = Request::builder()
            .method("POST")
            .uri("/x")
            .header("host", host);
        if let Some(o) = origin {
            b = b.header("origin", o);
        }
        app()
            .oneshot(b.body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
            .as_u16()
    }

    #[tokio::test]
    async fn guard_allows_dashboard_and_cli_blocks_foreign_origin_and_rebinding() {
        assert_eq!(status("127.0.0.1:4545", None).await, 200);
        assert_eq!(
            status("localhost:4545", Some("http://localhost:4545")).await,
            200
        );
        assert_eq!(
            status("127.0.0.1:4545", Some("https://evil.example")).await,
            403
        );
        assert_eq!(status("evil.example:4545", None).await, 403);
    }
}
