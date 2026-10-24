//! Sessions: one per device per Core, made from a ticket.
//!
//! - `POST /sessions`: sign in. [`SignIn`] in, [`SignedIn`] out. Limited to 10 a minute per
//!   address.
//! - `GET /sessions`: the owner sees every session, anyone else their own. `Vec<Session>`.
//! - `DELETE /sessions/{id}`: revoke a session. Dangerous when it is someone else's, which only
//!   the owner may revoke.
//! - `DELETE /sessions/current`: sign out of this Core.
//!
//! The token goes in `Authorization: Bearer` on every other route. Core stores only its hash.

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{
    AppState,
    access::User,
    activity::Action,
    error::{ApiError, ErrorCode},
};
use crate::access::{Caller, Peer, activity, fresh_ticket, now, sessions};

#[derive(Debug, Clone, Deserialize, TS)]
pub struct SignIn {
    /// A ticket the backend signed for this Core, under five minutes old and never used.
    pub ticket: String,
    /// Up to 64 characters.
    pub device_name: String,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct SignedIn {
    /// 256 random bits. Kept in the OS keychain by the App's Rust side; the WebView never sees
    /// it.
    pub token: String,
    pub session: Session,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Session {
    /// Public id for listing and revoking. Not the token.
    pub id: String,
    pub user: User,
    pub device_name: String,
    /// Unix seconds.
    pub created_at: i64,
    /// Unix seconds, written at most once an hour.
    pub last_used_at: i64,
    /// Unix seconds. The session ends at this time unless it is used before then; it never
    /// lasts past a year from `created_at`.
    pub expires_at: i64,
    /// The session making this request.
    pub current: bool,
}

/// `POST /sessions`.
pub async fn sign_in(
    State(state): State<Arc<AppState>>,
    peer: Option<Extension<Peer>>,
    Json(request): Json<SignIn>,
) -> Result<Json<SignedIn>, ApiError> {
    if let Some(Extension(Peer(address))) = peer
        && !state.access.allow_sign_in(address)
    {
        return Err(ApiError::new(
            ErrorCode::RateLimited,
            "Too many sign-ins from this address. Try again in a minute.",
        ));
    }
    let signed = sessions::sign_in(
        &state.db,
        &state.access.keys,
        &request.ticket,
        &request.device_name,
        now(),
    )
    .await?;
    Ok(Json(signed))
}

/// `GET /sessions`.
pub async fn list(
    caller: Caller,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Session>>, ApiError> {
    let only = if caller.is_owner() {
        None
    } else {
        caller.user_id()
    };
    let current = match &caller {
        Caller::Session { id, .. } => Some(id.as_str()),
        Caller::Local => None,
    };
    Ok(Json(sessions::list(&state.db, only, current, now()).await?))
}

/// `DELETE /sessions/{id}`.
pub async fn revoke(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let not_found =
        || ApiError::new(ErrorCode::NotFound, "There is no such session.");
    let session = sessions::get(&state.db, &id, None)
        .await?
        .ok_or_else(not_found)?;
    if caller.user_id() != Some(session.user.id.as_str()) {
        if !caller.is_owner() {
            return Err(not_found());
        }
        fresh_ticket(&state, &caller, &headers).await?;
    }
    end(&state, &caller, &id).await
}

/// `DELETE /sessions/current`.
pub async fn sign_out(
    caller: Caller,
    State(state): State<Arc<AppState>>,
) -> Result<StatusCode, ApiError> {
    match &caller {
        Caller::Session { id, .. } => end(&state, &caller, id).await,
        Caller::Local => Err(ApiError::new(
            ErrorCode::InvalidRequest,
            "The local CLI has no session to sign out of.",
        )),
    }
}

async fn end(
    state: &AppState,
    caller: &Caller,
    id: &str,
) -> Result<StatusCode, ApiError> {
    if let Some(user) = sessions::revoke(&state.db, id).await? {
        state.access.changed(&user);
        activity::log(
            &state.db,
            caller,
            None,
            Action::SessionRevoked {
                session_id: id.into(),
            },
        )
        .await;
    }
    Ok(StatusCode::NO_CONTENT)
}
