//! Updating Core. Owner only.

use std::sync::Arc;

use axum::{extract::State, http::HeaderMap};
use contracts::{
    ApiError, ErrorCode, Json,
    core::{
        Action, ChangeUpdateSettings, CheckForUpdate, GetUpdate, InstallUpdate,
        Restarting, RollBack, UpdateStatus,
    },
    server::Req,
};

use super::{CoreState, internal};
use crate::{
    access::{Caller, Dangerous, activity, fresh_ticket},
    update::UpdateError,
};

impl From<UpdateError> for ApiError {
    fn from(error: UpdateError) -> Self {
        let code = match &error {
            UpdateError::Downgrade(..)
            | UpdateError::UpToDate(_)
            | UpdateError::FailedBefore(..)
            | UpdateError::OnTrial
            | UpdateError::NothingToRollBackTo => ErrorCode::Conflict,
            UpdateError::DowngradeRemote => ErrorCode::Forbidden,
            UpdateError::NoRelease(_) => ErrorCode::NotFound,
            _ => ErrorCode::InternalError,
        };
        Self::new(code, error.to_string())
    }
}

pub async fn get(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    _: Req<GetUpdate>,
) -> Result<Json<UpdateStatus>, ApiError> {
    caller.require_owner()?;
    Ok(Json(state.update.status().await?))
}

pub async fn settings(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    headers: HeaderMap,
    Req { body: request, .. }: Req<ChangeUpdateSettings>,
) -> Result<Json<UpdateStatus>, ApiError> {
    caller.require_owner()?;
    // Turning auto-update on or changing the channel decides what Core installs next, so it is
    // as dangerous as installing it. Turning auto-update off is not.
    let (channel, _) = state.update.settings().await.map_err(internal)?;
    if request.auto_update == Some(true)
        || request.channel.is_some_and(|wanted| wanted != channel)
    {
        fresh_ticket(&state, &caller, &headers).await?;
    }
    state
        .update
        .set_settings(request.channel, request.auto_update)
        .await
        .map_err(internal)?;
    Ok(Json(state.update.status().await?))
}

pub async fn check(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    _: Req<CheckForUpdate>,
) -> Result<Json<UpdateStatus>, ApiError> {
    caller.require_owner()?;
    Ok(Json(state.update.check().await?))
}

pub async fn apply(
    Dangerous(caller): Dangerous,
    State(state): State<Arc<CoreState>>,
    Req { body: request, .. }: Req<InstallUpdate>,
) -> Result<Json<Restarting>, ApiError> {
    if request.allow_downgrade && caller != Caller::Local {
        return Err(UpdateError::DowngradeRemote.into());
    }
    let version = state
        .update
        .apply(request.allow_downgrade, request.retry)
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

pub async fn rollback(
    Dangerous(caller): Dangerous,
    State(state): State<Arc<CoreState>>,
    _: Req<RollBack>,
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
