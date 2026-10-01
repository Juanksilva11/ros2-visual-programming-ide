//! WebSocket handler — streams real-time system events to connected clients.

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use std::sync::Arc;

use super::AppState;

/// Upgrades the HTTP connection to a WebSocket.
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Forwards every `SystemEvent` from the broadcast channel to the client.
async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.event_bus.subscribe();
    tracing::debug!("WebSocket client connected.");

    while let Ok(event) = rx.recv().await {
        if let Ok(json_msg) = serde_json::to_string(&event)
            && socket.send(Message::Text(json_msg.into())).await.is_err()
        {
            tracing::debug!("WebSocket client disconnected.");
            break;
        }
    }
}
