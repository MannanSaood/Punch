//! Live events for the local dashboard WebSocket (/ws).

use crate::logger::ConnectionType;

pub fn emit_session_start(code: &str, connection_type: ConnectionType, role: &str) -> String {
    let session_id = uuid::Uuid::new_v4().to_string();
    let conn = match connection_type {
        ConnectionType::Direct => "Direct",
        ConnectionType::Relay => "Relay",
    };
    crate::dashboard_server::emit(
        "session_start",
        serde_json::json!({
            "session_id": session_id,
            "token_code": code,
            "connection_type": conn,
            "role": role,
        }),
    );
    session_id
}

pub fn print_connect_session_help() {
    println!("Session active — link is up between both peers.");
    println!("  Type a line and press Enter to send over this tunnel (link test).");
    println!("  For files use: punch send / punch receive");
    println!("  For ports use: punch forward expose / punch forward connect");
    println!("  For shell use: punch shell host / punch shell connect");
    println!("  Press Ctrl+C to disconnect.\n");
}

pub fn emit_session_end(
    session_id: &str,
    code: &str,
    connection_type: ConnectionType,
    bytes_sent: u64,
    bytes_received: u64,
) {
    let conn = match connection_type {
        ConnectionType::Direct => "Direct",
        ConnectionType::Relay => "Relay",
    };
    crate::dashboard_server::emit(
        "session_end",
        serde_json::json!({
            "session_id": session_id,
            "token_code": code,
            "connection_type": conn,
            "bytes_sent": bytes_sent,
            "bytes_received": bytes_received,
        }),
    );
}
