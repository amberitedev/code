//! WebSocket handlers - Console streaming.

use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::extract::ws::{Message, WebSocket};
use uuid::Uuid;
use std::sync::Arc;
use std::collections::HashMap;
use crate::application::registry::ServiceRegistry;
use crate::domain::instances::InstanceId;
use crate::application::instance_actor::InstanceEvent;
use crate::infrastructure::supabase_auth::SupabaseJwtValidator;

#[derive(serde::Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}

/// Handle WebSocket upgrade.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(instance_id): Path<Uuid>,
    Query(query): Query<WsQuery>,
    State(state): State<(Arc<ServiceRegistry>, Arc<SupabaseJwtValidator>)>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let id = InstanceId::new(instance_id);
    let registry = state.0.clone();
    let validator = state.1.clone();

    let token = query
        .token
        .or_else(|| {
            headers
                .get("Sec-WebSocket-Protocol")
                .and_then(|h| h.to_str().ok())
                .and_then(|h| h.strip_prefix("Bearer "))
                .map(|s| s.to_string())
        });

    let token = match token {
        Some(t) => t,
        None => {
            return (StatusCode::UNAUTHORIZED, "Missing token").into_response();
        }
    };

    if let Err(e) = validator.validate(&token) {
        let msg = match e {
            crate::infrastructure::supabase_auth::JwtError::Expired => "Token expired",
            crate::infrastructure::supabase_auth::JwtError::InvalidFormat => "Invalid token format",
            crate::infrastructure::supabase_auth::JwtError::MissingToken => "Missing token",
            crate::infrastructure::supabase_auth::JwtError::ValidationFailed(_) => "Invalid token",
        };
        return (StatusCode::UNAUTHORIZED, msg).into_response();
    }

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
    let mut event_rx = match registry.instance_service.subscribe_to_events(instance_id) {
        Some(rx) => rx,
        None => {
            let _ = socket.send(Message::Text(
                serde_json::json!({"error": "Instance not found"}).to_string()
            )).await;
            return;
        }
    };

    let _ = socket.send(Message::Text(
        serde_json::json!({
            "type": "connected",
            "instance": instance_id.0.to_string(),
            "message": "Console connected - PTY colors enabled"
        }).to_string()
    )).await;

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

#[derive(serde::Serialize)]
struct ConsoleEvent {
    timestamp: i64,
    line: String,
    #[serde(rename = "type")]
    event_type: String,
}
