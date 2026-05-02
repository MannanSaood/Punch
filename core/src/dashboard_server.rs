use axum::{Router, Json, response::Html, routing::get};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

const DASHBOARD_HTML: &str = include_str!("dashboard.html");

pub async fn serve() -> anyhow::Result<()> {
    let app = Router::new()
        .route("/", get(serve_dashboard))
        .route("/api/sessions", get(api_sessions))
        .route("/api/tokens", get(api_tokens))
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([127, 0, 0, 1], 7777));
    println!("Dashboard running at http://localhost:7777");
    println!("Press Ctrl+C to stop.\n");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn serve_dashboard() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

async fn api_sessions() -> Json<serde_json::Value> {
    let path = crate::logger::log_path();

    if !path.exists() {
        return Json(serde_json::json!([]));
    }

    match tokio::fs::read_to_string(&path).await {
        Ok(content) => {
            let sessions: serde_json::Value = serde_json::from_str(&content)
                .unwrap_or(serde_json::json!([]));
            Json(sessions)
        }
        Err(_) => Json(serde_json::json!([])),
    }
}

async fn api_tokens() -> Json<serde_json::Value> {
    let tokens = crate::token_store::list_tokens().await;
    Json(serde_json::json!(tokens))
}
