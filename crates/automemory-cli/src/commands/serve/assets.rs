use axum::{
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "src/dashboard/assets/"]
pub struct DashboardAssets;

pub fn get_mime_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") | Some("htm") => "text/html",
        Some("css") => "text/css",
        Some("js") | Some("mjs") => "application/javascript",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("ttf") => "font/ttf",
        _ => "application/octet-stream",
    }
}

pub async fn static_asset_handler(uri: axum::http::Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }

    match DashboardAssets::get(&path) {
        Some(content) => {
            let mime = get_mime_type(&path);
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
            (StatusCode::OK, headers, content.data).into_response()
        }
        None => match DashboardAssets::get("index.html") {
            Some(content) => {
                let mut headers = HeaderMap::new();
                headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html"));
                (StatusCode::OK, headers, content.data).into_response()
            }
            None => (StatusCode::NOT_FOUND, "Not Found").into_response(),
        },
    }
}
