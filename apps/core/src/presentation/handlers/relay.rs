use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    api::{DeliveryStatus, MessageId, RelayStore, SqliteRelayStore, StoredMessage},
    application::state::AppState,
    presentation::{error::ApiError, extractors::AuthUser},
};

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

fn store(state: &Arc<AppState>) -> SqliteRelayStore {
    SqliteRelayStore::new(state.pool.clone())
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

    let message = StoredMessage {
        id: id.clone(),
        r#type: body.r#type,
        version: body.version,
        sender_id: body.sender_id,
        recipient_id: body.recipient_id,
        payload: body.payload,
        ack: body.ack,
        status: DeliveryStatus::Pending.wire().to_string(),
        created_at: now.to_rfc3339(),
        expires_at: expires_at.to_rfc3339(),
        result: None,
        error: None,
    };

    store(&state)
        .insert(&message)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({ "id": id })))
}

pub async fn pending(
    _user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(recipient_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let messages = store(&state)
        .pending(&recipient_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({ "messages": messages })))
}

pub async fn status(
    _user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let message = store(&state)
        .get(&MessageId::new(id))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({ "message": message })))
}

pub async fn ack(
    _user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<AckRelayMessage>,
) -> Result<Json<Value>, ApiError> {
    store(&state)
        .mark(
            &MessageId::new(id),
            &body.recipient_id,
            DeliveryStatus::Received,
            None,
            None,
        )
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
    store(&state)
        .mark(
            &MessageId::new(id),
            &body.recipient_id,
            DeliveryStatus::Processed,
            body.result,
            body.error,
        )
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({ "ok": true })))
}
