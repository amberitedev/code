//! WebSocket handlers - Console streaming.

use axum::extract::Path;
use axum::extract::State;
use axum::extract::WebSocketUpgrade;
use axum::response::IntoResponse;
use axum::extract::ws::{Message, WebSocket};
use uuid::Uuid;
use std::sync::Arc;
use crate::application::registry::ServiceRegistry;
use crate::domain::instances::InstanceId;
use crate::application::instance_actor::InstanceEvent;

/// Handle WebSocket upgrade.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(instance_id): Path<Uuid>,
    State(registry): State<Arc<ServiceRegistry>>,
) -> impl IntoResponse {
    let id = InstanceId::new(instance_id);
    let registry = registry.clone();

    ws.on_upgrade(move |socket| async move {
        handle_console_socket(socket, id, registry).await;
    })
}

/// Handle console WebSocket connection.
async fn handle_console_socket(
    mut socket: WebSocket,
    instance_id: InstanceId,
    registry: Arc<ServiceRegistry>,
) {
    // Subscribe to instance events
    let mut event_rx = match registry.instance_service.subscribe_to_events(instance_id) {
        Some(rx) => rx,
        None => {
            let _ = socket.send(Message::Text(
                serde_json::json!({"error": "Instance not found"}).to_string()
            )).await;
            return;
        }
    };

    // Send connection confirmation
    let _ = socket.send(Message::Text(
        serde_json::json!({
            "type": "connected",
            "instance": instance_id.0.to_string(),
            "message": "Console connected - PTY colors enabled"
        }).to_string()
    )).await;

    // Stream live console events
    while let Ok(event) = event_rx.recv().await {
        match event {
            InstanceEvent::ConsoleLine(line) => {
                let msg = ConsoleEvent {
                    timestamp: chrono::Utc::now().timestamp(),
                    line,
                    event_type: "stdout".into(),
                };

                let json = match serde_json::to_string(&msg) {
                    Ok(j) => j,
                    Err(e) => {
                        tracing::error!("Failed to serialize console event: {}", e);
                        continue;
                    }
                };

                if socket.send(Message::Text(json)).await.is_err() {
                    // Client disconnected
                    break;
                }
            }
            InstanceEvent::Started => {
                let _ = socket.send(Message::Text(
                    serde_json::json!({
                        "type": "system",
                        "message": "Server started"
                    }).to_string()
                )).await;
            }
            InstanceEvent::Stopped { exit_code } => {
                let _ = socket.send(Message::Text(
                    serde_json::json!({
                        "type": "system",
                        "message": format!("Server stopped (exit code: {})", exit_code)
                    }).to_string()
                )).await;
            }
            InstanceEvent::Error(msg) => {
                let _ = socket.send(Message::Text(
                    serde_json::json!({
                        "type": "error",
                        "message": msg
                    }).to_string()
                )).await;
            }
        }
    }

    tracing::debug!("Console WebSocket client disconnected");
}

/// Console event.
#[derive(serde::Serialize)]
pub struct ConsoleEvent {
    pub timestamp: i64,
    pub line: String,
    #[serde(rename = "type")]
    pub event_type: String,
}
