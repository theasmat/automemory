use crate::protocol::{JsonRpcRequest, JsonRpcResponse};
use crate::server::McpServer;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use std::sync::Arc;

pub fn mcp_router(server: Arc<McpServer>) -> Router {
    Router::new()
        .route("/mcp", post(handle_http_mcp))
        .with_state(server)
}

async fn handle_http_mcp(
    State(server): State<Arc<McpServer>>,
    Json(req): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    if let Some(resp) = server.handle_request(req) {
        (StatusCode::OK, Json(resp))
    } else {
        (
            StatusCode::NO_CONTENT,
            Json(JsonRpcResponse::success(None, serde_json::json!({}))),
        )
    }
}
