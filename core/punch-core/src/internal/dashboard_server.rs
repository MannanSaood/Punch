use axum::{
    Router, Json,
    extract::ws::{WebSocket, WebSocketUpgrade, Message as WsMessage},
    response::{Html, Response, IntoResponse},
    routing::get,
    http::{StatusCode, header, Uri},
};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use rust_embed::RustEmbed;

// This macro embeds the frontend files directly into your .exe at compile time!
#[derive(RustEmbed)]
#[folder = "../../dashboard/dist/"]
struct Assets;

#[derive(Clone)]
pub struct DashboardState {
    pub tx: broadcast::Sender<String>,
}

impl Default for DashboardState {
    fn default() -> Self {
        Self::new()
    }
}

impl DashboardState {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        DashboardState { tx }
    }

    pub fn emit(&self, event_type: &str, data: serde_json::Value) {
        let msg = serde_json::json!({
            "type": event_type,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "data": data,
        });
        let _ = self.tx.send(msg.to_string());
    }
}

static DASHBOARD: std::sync::OnceLock<DashboardState> = std::sync::OnceLock::new();

pub fn get_dashboard() -> &'static DashboardState {
    DASHBOARD.get_or_init(DashboardState::new)
}

pub fn emit(event_type: &str, data: serde_json::Value) {
    get_dashboard().emit(event_type, data);
}

pub async fn serve() -> anyhow::Result<()> {
    let state = Arc::new(get_dashboard().clone());

    let app = Router::new()
        .route("/api/sessions",  get(api_sessions))
        .route("/api/tokens",    get(api_tokens))
        .route("/api/transfers", get(api_transfers))
        .route("/api/forwards",  get(api_forwards))
        .route("/api/shells",    get(api_shells))
        .route("/api/active",    get(api_active))
        .route("/ws",            get({
            let state = Arc::clone(&state);
            move |ws| ws_handler(ws, Arc::clone(&state))
        }))
        // The fallback handles serving the index.html, JS, and CSS files
        .fallback(serve_asset)
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([127, 0, 0, 1], 7777));
    println!("Dashboard running at http://localhost:7777");
    println!("Press Ctrl+C to stop.\n");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

// Automatically serve embedded files (HTML, JS, CSS)
async fn serve_asset(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match Assets::get(path) {
        // If the file exists (like an .html, .js, or .css file), serve it with the right mime type
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref())],
                content.data.into_owned(),
            ).into_response()
        }
        // If the file isn't found (useful for SPA client-side routing), fallback to index.html
        None => {
            match Assets::get("index.html") {
                Some(content) => {
                    (
                        StatusCode::OK,
                        [(header::CONTENT_TYPE, "text/html")],
                        content.data.into_owned(),
                    ).into_response()
                }
                None => {
                    // Fallback just in case something went terribly wrong
                    Html(r#"<!DOCTYPE html>
<html style="background:#080808;color:#f0ece4;font-family:'Courier New',monospace;display:flex;align-items:center;justify-content:center;height:100vh;margin:0">
<body style="text-align:center">
  <div>
    <div style="font-family:serif;font-size:48px;font-weight:900;letter-spacing:0.3em;margin-bottom:24px">PUNCH</div>
    <div style="font-size:12px;letter-spacing:0.2em;opacity:0.5;margin-bottom:32px;text-transform:uppercase">Dashboard completely missing</div>
  </div>
</body></html>"#.to_string()).into_response()
                }
            }
        }
    }
}

async fn api_sessions()  -> Json<serde_json::Value> { read_log("sessions.json").await }
async fn api_tokens()    -> Json<serde_json::Value> {
    let t = crate::token_store::list_tokens().await;
    Json(serde_json::json!(t))
}
async fn api_transfers() -> Json<serde_json::Value> { read_log("transfers.json").await }
async fn api_forwards()  -> Json<serde_json::Value> { read_log("forward.json").await }
async fn api_shells()    -> Json<serde_json::Value> { read_log("shell_sessions.json").await }
async fn api_active()    -> Json<serde_json::Value> { read_log("active.json").await }

async fn ws_handler(ws: WebSocketUpgrade, state: Arc<DashboardState>) -> Response {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: Arc<DashboardState>) {
    let mut rx = state.tx.subscribe();

    let welcome = serde_json::json!({
        "type": "connected",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "data": { "message": "Punch dashboard connected" }
    });
    let _ = socket.send(WsMessage::Text(welcome.to_string())).await;

    loop {
        tokio::select! {
            Ok(msg) = rx.recv() => {
                if socket.send(WsMessage::Text(msg)).await.is_err() { break; }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(WsMessage::Close(_))) | None => break,
                    Some(Ok(WsMessage::Ping(p))) => {
                        let _ = socket.send(WsMessage::Pong(p)).await;
                    }
                    _ => {}
                }
            }
        }
    }
}

fn log_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".punch").join("logs")
}

async fn read_log(filename: &str) -> Json<serde_json::Value> {
    let path = log_dir().join(filename);
    if !path.exists() { return Json(serde_json::json!([])); }
    match tokio::fs::read_to_string(&path).await {
        Ok(c) => Json(serde_json::from_str(&c).unwrap_or(serde_json::json!([]))),
        Err(_) => Json(serde_json::json!([])),
    }
}