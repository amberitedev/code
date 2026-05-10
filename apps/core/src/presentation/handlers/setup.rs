use std::sync::Arc;

use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{application::state::AppState, presentation::error::ApiError};

/// Maximum wrong pairing-code attempts before lockout (SEC-01).
const MAX_PAIRING_ATTEMPTS: u32 = 5;

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
    use std::sync::atomic::Ordering;

    // SEC-01: refuse if too many wrong attempts have been made.
    let attempts = state.wrong_pairing_attempts.load(Ordering::Relaxed);
    if attempts >= MAX_PAIRING_ATTEMPTS {
        return Err(ApiError::TooManyRequests(format!(
            "too many wrong pairing attempts ({attempts}); restart Core to reset"
        )));
    }

    let mut guard = state.pairing_code.lock().await;

    let expected = guard
        .as_deref()
        .ok_or_else(|| ApiError::BadRequest("Core is already paired".into()))?;

    if body.code != expected {
        state.wrong_pairing_attempts.fetch_add(1, Ordering::Relaxed);
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

    // Reset attempt counter and clear the pairing code.
    state.wrong_pairing_attempts.store(0, Ordering::Relaxed);
    *guard = None;
    Ok(Json(json!({ "ok": true })))
}

/// GET /setup/status — check whether Core is paired.
/// In dev mode, always reports paired so the App skips the pairing screen.
pub async fn setup_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    if state.config.dev_mode {
        return Json(json!({ "paired": true, "dev_mode": true }));
    }
    let paired = state.jwks_url().await.is_some();
    Json(json!({ "paired": paired }))
}
