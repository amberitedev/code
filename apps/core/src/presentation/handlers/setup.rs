use std::sync::Arc;

use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{application::state::AppState, presentation::error::ApiError};

#[derive(Deserialize)]
pub struct SetupRequest {
    /// Six-digit pairing code shown on Core's terminal.
    pub code: String,
    /// Supabase project URL (e.g. https://xyz.supabase.co).
    pub supabase_url: String,
    /// Supabase user ID of the owner (from their JWT sub).
    pub owner_user_id: String,
}

/// POST /setup — complete first-run pairing.
///
/// No auth required — the pairing code itself is the credential.
pub async fn complete_setup(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SetupRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut guard = state.pairing_code.lock().await;

    let expected = guard
        .as_deref()
        .ok_or_else(|| ApiError::BadRequest("Core is already paired".into()))?;

    if body.code != expected {
        return Err(ApiError::Unauthorized("invalid pairing code".into()));
    }

    sqlx::query(
        "INSERT OR REPLACE INTO core_config \
         (id, supabase_url, owner_user_id, paired_at) VALUES (1, ?, ?, ?)",
    )
    .bind(&body.supabase_url)
    .bind(&body.owner_user_id)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    *guard = None;
    Ok(Json(json!({ "ok": true })))
}

/// GET /setup/status — check whether Core is paired.
pub async fn setup_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let paired = state.jwks_url().await.is_some();
    Json(json!({ "paired": paired }))
}
