//! Updating Core. Owner only.
//!
//! - `GET /update`: [`UpdateStatus`].
//! - `PATCH /update`: [`UpdateSettings`] in, [`UpdateStatus`] out.
//! - `POST /update/check`: look up the newest release now. [`UpdateStatus`].
//! - `POST /update/apply`: dangerous. [`ApplyUpdate`] in, [`Restarting`] out; Core restarts
//!   into the new version right after answering.
//! - `POST /update/rollback`: dangerous. Back to the previous version, with the database from
//!   before the update. [`Restarting`].

use std::sync::Arc;

use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{
    AppState,
    activity::Action,
    error::{ApiError, ErrorCode},
};
use crate::{
    access::{Caller, Dangerous, activity},
    update::{UpdateError, release::Channel},
};

#[derive(Debug, Clone, Serialize, TS)]
pub struct UpdateStatus {
    /// The running version.
    pub version: String,
    /// Which release files this Core installs, such as `linux-x64`. Null on a platform Copal is
    /// not released for, which cannot update.
    pub target: Option<String>,
    pub channel: Channel,
    /// Updates install by themselves. Off: only when the owner asks.
    pub auto_update: bool,
    /// The newest release on the channel, as of the last check.
    pub latest: Option<String>,
    /// `latest` is newer than the running version.
    pub update_available: bool,
    /// Unix seconds.
    pub checked_at: Option<i64>,
    /// Why the last check failed.
    pub check_error: Option<String>,
    /// The version a rollback goes back to.
    pub previous: Option<String>,
    /// The last version that failed to start and was rolled back from.
    pub failed: Option<String>,
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct UpdateSettings {
    #[ts(optional)]
    pub channel: Option<Channel>,
    #[ts(optional)]
    pub auto_update: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize, TS)]
pub struct ApplyUpdate {
    /// Install the channel's release even when it is older. Only the copal CLI may.
    #[serde(default)]
    #[ts(as = "Option<bool>", optional)]
    pub allow_downgrade: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct Restarting {
    /// The version Core restarts into.
    pub version: String,
}

impl From<UpdateError> for ApiError {
    fn from(error: UpdateError) -> Self {
        let code = match &error {
            UpdateError::Downgrade(..)
            | UpdateError::UpToDate(_)
            | UpdateError::OnTrial
            | UpdateError::NothingToRollBackTo => ErrorCode::Conflict,
            UpdateError::DowngradeRemote => ErrorCode::Forbidden,
            UpdateError::NoRelease(_) => ErrorCode::NotFound,
            _ => ErrorCode::Internal,
        };
        Self::new(code, error.to_string())
    }
}

/// `GET /update`.
pub async fn get(
    caller: Caller,
    State(state): State<Arc<AppState>>,
) -> Result<Json<UpdateStatus>, ApiError> {
    caller.require_owner()?;
    Ok(Json(state.update.status().await?))
}

/// `PATCH /update`.
pub async fn settings(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Json(request): Json<UpdateSettings>,
) -> Result<Json<UpdateStatus>, ApiError> {
    caller.require_owner()?;
    state
        .update
        .set_settings(request.channel, request.auto_update)
        .await?;
    Ok(Json(state.update.status().await?))
}

/// `POST /update/check`.
pub async fn check(
    caller: Caller,
    State(state): State<Arc<AppState>>,
) -> Result<Json<UpdateStatus>, ApiError> {
    caller.require_owner()?;
    Ok(Json(state.update.check().await?))
}

/// `POST /update/apply`.
pub async fn apply(
    Dangerous(caller): Dangerous,
    State(state): State<Arc<AppState>>,
    request: Option<Json<ApplyUpdate>>,
) -> Result<Json<Restarting>, ApiError> {
    let request = request.map(|Json(request)| request).unwrap_or_default();
    if request.allow_downgrade && caller != Caller::Local {
        return Err(UpdateError::DowngradeRemote.into());
    }
    let version = state
        .update
        .apply(request.allow_downgrade)
        .await?
        .to_string();
    activity::log(
        &state.db,
        &caller,
        None,
        Action::CoreUpdated {
            version: version.clone(),
        },
    )
    .await;
    Ok(Json(Restarting { version }))
}

/// `POST /update/rollback`.
pub async fn rollback(
    Dangerous(caller): Dangerous,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Restarting>, ApiError> {
    let version = state.update.roll_back().await?;
    activity::log(
        &state.db,
        &caller,
        None,
        Action::CoreUpdated {
            version: version.clone(),
        },
    )
    .await;
    Ok(Json(Restarting { version }))
}
