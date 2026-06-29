use std::sync::{atomic::Ordering, Arc};

use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    application::{core_projection_service, pairing_service, state::AppState},
    presentation::error::ApiError,
};

/// Maximum wrong pairing-code attempts before lockout (SEC-01).
const MAX_PAIRING_ATTEMPTS: u32 = 5;

#[derive(Deserialize)]
pub struct SetupRequest {
    /// Eight-character pairing code shown on Core's terminal.
    pub code: Option<String>,
    /// One-time secret written locally for app-launched Core auto-pairing.
    pub local_setup_secret: Option<String>,
    /// Convex deployment URL used for durable account and social state.
    pub convex_url: String,
    /// JWKS URL for the current auth provider.
    pub auth_jwks_url: String,
    /// Auth user ID of the owner (from their JWT sub).
    pub owner_user_id: String,
    /// JWT audience claim to validate. Defaults to "authenticated" if omitted.
    pub auth_audience: Option<String>,
    /// Legacy one-time realtime credential issued by Convex after a successful claim.
    pub realtime_credential: Option<String>,
    /// Legacy optional rollout endpoint for Cloudflare presence.
    pub realtime_url: Option<String>,
}

/// POST /setup — complete first-run pairing.
///
/// No auth required — the pairing code itself is the credential.
pub async fn complete_setup(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SetupRequest>,
) -> Result<Json<Value>, ApiError> {
    // SEC-01: refuse if too many wrong attempts have been made.
    let attempts = state.wrong_pairing_attempts.load(Ordering::Relaxed);
    if attempts >= MAX_PAIRING_ATTEMPTS {
        return Err(ApiError::TooManyRequests(format!(
            "too many wrong pairing attempts ({attempts}); restart Core to reset"
        )));
    }

    let mut pairing_code = state.pairing_code.lock().await;
    let mut pairing_code_expires_at =
        state.pairing_code_expires_at.lock().await;
    let mut local_setup_secret = state.local_setup_secret.lock().await;

    if pairing_code.is_none() && local_setup_secret.is_none() {
        return Err(ApiError::BadRequest("Core is already paired".into()));
    }

    let pairing_code_valid = body.code.as_deref().is_some_and(|code| {
        let code = normalize_pairing_code(code);
        let code_is_current = pairing_code
            .as_deref()
            .is_some_and(|expected| code == expected);
        let code_is_unexpired = pairing_code_expires_at
            .as_ref()
            .is_some_and(|expires_at| *expires_at > std::time::Instant::now());
        code_is_current && code_is_unexpired
    });
    let local_secret_valid =
        body.local_setup_secret.as_deref().is_some_and(|secret| {
            let secret_is_current = local_setup_secret
                .as_deref()
                .is_some_and(|expected| secret == expected);
            let secret_is_unexpired =
                pairing_code_expires_at.as_ref().is_some_and(|expires_at| {
                    *expires_at > std::time::Instant::now()
                });
            secret_is_current && secret_is_unexpired
        });

    if !pairing_code_valid && !local_secret_valid {
        state.wrong_pairing_attempts.fetch_add(1, Ordering::Relaxed);
        return Err(ApiError::Unauthorized("invalid pairing code".into()));
    }

    if body.owner_user_id.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "owner_user_id cannot be empty".into(),
        ));
    }
    validate_https_url(&body.convex_url, "convex_url")?;
    validate_https_url(&body.auth_jwks_url, "auth_jwks_url")?;
    if let Some(url) = &body.realtime_url {
        validate_https_url(url, "realtime_url")?;
    }
    let auth_audience = body
        .auth_audience
        .as_deref()
        .unwrap_or("authenticated")
        .trim()
        .to_string();
    if auth_audience.is_empty() || auth_audience.chars().any(char::is_control) {
        return Err(ApiError::BadRequest(
            "auth_audience cannot be empty or contain control characters"
                .into(),
        ));
    }
    if let Some(credential) = &body.realtime_credential {
        if credential.len() != 64
            || !credential
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        {
            return Err(ApiError::BadRequest(
                "realtime_credential must be a 32-byte hexadecimal secret"
                    .into(),
            ));
        }
    }
    if body.realtime_credential.is_some() != body.realtime_url.is_some() {
        return Err(ApiError::BadRequest(
            "realtime_url and realtime_credential must be supplied together"
                .into(),
        ));
    }

    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
		"INSERT OR REPLACE INTO core_config \
		 (id, supabase_url, convex_url, auth_jwks_url, auth_audience, owner_user_id, paired_at, core_id, realtime_credential, realtime_url) VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
	)
	.bind(&body.convex_url)
	.bind(&body.convex_url)
	.bind(&body.auth_jwks_url)
	.bind(&auth_audience)
	.bind(&body.owner_user_id)
    .bind(&now)
    .bind(&state.core_id)
    .bind(&body.realtime_credential)
    .bind(&body.realtime_url)
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let setup_mode = if local_secret_valid {
        "local"
    } else {
        "remote"
    };
    sqlx::query(
		"INSERT INTO core_metadata (id, name, setup_mode, updated_at) VALUES (1, 'Copal', ?, ?) \
		 ON CONFLICT(id) DO UPDATE SET setup_mode = excluded.setup_mode, updated_at = excluded.updated_at",
	)
	.bind(setup_mode)
	.bind(&now)
	.execute(&state.pool)
	.await
	.map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
		"INSERT INTO core_members (user_id, role, permission_preset, status, joined_at, updated_at) \
		 VALUES (?, 'owner', 'owner', 'active', ?, ?) \
		 ON CONFLICT(user_id) DO UPDATE SET role = 'owner', permission_preset = 'owner', status = 'active', updated_at = excluded.updated_at",
	)
	.bind(&body.owner_user_id)
	.bind(&now)
	.bind(&now)
	.execute(&state.pool)
	.await
	.map_err(|e| ApiError::Internal(e.to_string()))?;

    // Reset attempt counter and clear the pairing code.
    state.wrong_pairing_attempts.store(0, Ordering::Relaxed);
    *pairing_code = None;
    *pairing_code_expires_at = None;
    *local_setup_secret = None;
    tokio::fs::remove_file(state.config.data_dir.join(".setup_secret"))
        .await
        .ok();
    drop(pairing_code);
    drop(pairing_code_expires_at);
    drop(local_setup_secret);
    core_projection_service::sync_projection_best_effort(&state, "setup").await;
    Ok(Json(json!({ "ok": true, "core_id": state.core_id })))
}

fn normalize_pairing_code(code: &str) -> String {
    code.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn validate_https_url(value: &str, field: &str) -> Result<(), ApiError> {
    let parsed = url::Url::parse(value).map_err(|_| {
        ApiError::BadRequest(format!("{field} must be a valid URL"))
    })?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() {
        return Err(ApiError::BadRequest(format!(
            "{field} must be an HTTPS URL with a host"
        )));
    }
    Ok(())
}

/// GET /setup/status — check whether Core is paired.
pub async fn setup_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let paired = state.jwks_url().await.is_some();
    Json(json!({
        "paired": paired,
        "dev_mode": state.config.dev_mode,
        "core_id": state.core_id,
    }))
}

/// POST /setup/dev-reset — reset local pairing state in dev builds.
pub async fn dev_reset_setup(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    if !state.config.dev_mode {
        return Err(ApiError::NotFound("not found".into()));
    }

    let registered = pairing_service::reset_running_pairing(state.clone())
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({
        "ok": true,
        "core_id": state.core_id,
        "registered": registered,
    })))
}
