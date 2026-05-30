use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    application::state::AppState,
    presentation::{error::ApiError, extractors::AuthUser},
};

type RelayStatusRow = (
    String,
    String,
    i64,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
);

#[derive(Deserialize)]
pub struct PublishRelayMessage {
    pub id: Option<String>,
    pub r#type: String,
    pub version: i64,
    pub sender_id: String,
    pub recipient_id: String,
    pub payload: Value,
    pub ack: String,
    pub ttl_ms: Option<i64>,
}

#[derive(Deserialize)]
pub struct AckRelayMessage {
    pub recipient_id: String,
    pub result: Option<Value>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct RelayMessage {
    pub id: String,
    pub r#type: String,
    pub version: i64,
    pub sender_id: String,
    pub recipient_id: String,
    pub payload: Value,
    pub ack: String,
    pub status: String,
    pub created_at: String,
    pub expires_at: String,
}

pub async fn publish(
    _user: AuthUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<PublishRelayMessage>,
) -> Result<Json<Value>, ApiError> {
    let now = chrono::Utc::now();
    let ttl_ms = body.ttl_ms.unwrap_or(5 * 60 * 1000).max(1000);
    let expires_at = now + chrono::Duration::milliseconds(ttl_ms);
    let id = body.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    sqlx::query(
        "INSERT INTO core_relay_messages \
         (id, type, version, sender_id, recipient_id, payload, ack, status, created_at, expires_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?)",
    )
    .bind(&id)
    .bind(&body.r#type)
    .bind(body.version)
    .bind(&body.sender_id)
    .bind(&body.recipient_id)
    .bind(body.payload.to_string())
    .bind(&body.ack)
    .bind(now.to_rfc3339())
    .bind(expires_at.to_rfc3339())
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({ "id": id })))
}

pub async fn pending(
    _user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(recipient_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let now = chrono::Utc::now().to_rfc3339();
    let rows: Vec<(String, String, i64, String, String, String, String, String, String, String)> = sqlx::query_as(
        "SELECT id, type, version, sender_id, recipient_id, payload, ack, status, created_at, expires_at \
         FROM core_relay_messages \
         WHERE recipient_id = ? AND status = 'pending' AND expires_at > ? \
         ORDER BY created_at ASC LIMIT 100",
    )
    .bind(&recipient_id)
    .bind(now)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let messages: Vec<RelayMessage> = rows
        .into_iter()
        .map(|row| RelayMessage {
            id: row.0,
            r#type: row.1,
            version: row.2,
            sender_id: row.3,
            recipient_id: row.4,
            payload: serde_json::from_str(&row.5).unwrap_or(Value::Null),
            ack: row.6,
            status: row.7,
            created_at: row.8,
            expires_at: row.9,
        })
        .collect();

    Ok(Json(json!({ "messages": messages })))
}

pub async fn status(
    _user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let row: Option<RelayStatusRow> = sqlx::query_as(
        "SELECT id, type, version, sender_id, recipient_id, payload, ack, status, created_at, expires_at, result, error \
         FROM core_relay_messages WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let message = row.map(|row| {
        json!({
            "id": row.0,
            "type": row.1,
            "version": row.2,
            "sender_id": row.3,
            "recipient_id": row.4,
            "payload": serde_json::from_str::<Value>(&row.5).unwrap_or(Value::Null),
            "ack": row.6,
            "status": row.7,
            "created_at": row.8,
            "expires_at": row.9,
            "result": row.10.and_then(|v| serde_json::from_str::<Value>(&v).ok()),
            "error": row.11,
        })
    });

    Ok(Json(json!({ "message": message })))
}

pub async fn ack(
    _user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<AckRelayMessage>,
) -> Result<Json<Value>, ApiError> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE core_relay_messages \
         SET status = 'received', received_at = ? \
         WHERE id = ? AND recipient_id = ?",
    )
    .bind(now)
    .bind(&id)
    .bind(&body.recipient_id)
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({ "ok": true })))
}

pub async fn complete(
    _user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<AckRelayMessage>,
) -> Result<Json<Value>, ApiError> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE core_relay_messages \
         SET status = 'processed', processed_at = ?, result = ?, error = ? \
         WHERE id = ? AND recipient_id = ?",
    )
    .bind(now)
    .bind(body.result.map(|v| v.to_string()))
    .bind(body.error)
    .bind(&id)
    .bind(&body.recipient_id)
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({ "ok": true })))
}
