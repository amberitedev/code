//! A server following an App instance through the instance's share, as one more member: the
//! engine reads the share's latest version for the user who asked, with their read ticket, and
//! applies it as a member's update. A running server keeps running; the version waits for it to
//! stop, or with `restart` the server is stopped for it and started again after.

use std::{sync::Arc, time::Duration};

use axum::extract::State;
use contracts::{
    ApiError, Empty,
    core::{Action, FollowInstance, LinkedInstance, LinkedPackState},
    server::Req,
};
use engine::{
    install::{InstallJobStatus, SharedInstanceInstallData},
    routes::api_error,
    state::{SharedInstanceAttachment, SharedInstanceRole},
};

use super::{CoreState, engine_mount, internal};
use crate::{
    access::{Caller, Permission, activity},
    servers::{Hold, Lifecycle, ServerError},
};

pub async fn follow(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, body, .. }: Req<FollowInstance>,
) -> Result<Empty, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id, Permission::Edit).await?;
    // Read now, so a version that cannot go fails the request.
    let data = engine::instance::followed_shared_instance_version(
        &state.engine,
        &id,
        &body.shared_instance_id,
        &body.ticket,
    )
    .await
    .map_err(api_error)?;
    let hold = match state.servers.hold(&id) {
        Ok(hold) => Some(hold),
        Err(ServerError::AlreadyRunning) => None,
        Err(error) => return Err(error.into()),
    };
    // A version still waiting from an earlier push is superseded.
    let pending = match hold {
        Some(_) => None,
        None => Some(serde_json::to_string(&data).map_err(internal)?),
    };
    sqlx::query(
        "INSERT INTO follows (server_id, instance_id, instance_name, shared_instance_id, state, error, pending, start_after) \
         VALUES (?, ?, ?, ?, ?, NULL, ?, ?) \
         ON CONFLICT (server_id) DO UPDATE SET instance_id = excluded.instance_id, \
         instance_name = excluded.instance_name, shared_instance_id = excluded.shared_instance_id, \
         state = excluded.state, error = NULL, pending = excluded.pending, \
         start_after = excluded.start_after",
    )
    .bind(&id)
    .bind(&body.instance.id)
    .bind(&body.instance.name)
    .bind(&body.shared_instance_id)
    .bind(if hold.is_some() { "applying" } else { "pending" })
    .bind(&pending)
    .bind(body.restart && hold.is_none())
    .execute(&state.db)
    .await
    .map_err(internal)?;
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    match hold {
        Some(hold) => {
            if let Err(error) = apply(&state, hold, data, false).await {
                failed(&state, &id, &error.description).await;
                return Err(error);
            }
        }
        None => {
            if body.restart {
                let (state, id) = (Arc::clone(&state), id.clone());
                tokio::spawn(async move {
                    match state.servers.stop(&id).await {
                        Ok(()) | Err(ServerError::NotRunning) => {}
                        Err(error) => {
                            tracing::warn!(server = %id, %error, "could not stop the server for its update");
                        }
                    }
                });
            }
            // It may have stopped while the version was read.
            tokio::spawn(apply_pending(state, id));
        }
    }
    Ok(Empty)
}

/// Apply each version waiting for its server once the server stops, starting with those pushed
/// before the Core last stopped to servers that are stopped now.
pub async fn apply_on_stop(state: Arc<CoreState>) {
    use tokio::sync::broadcast::error::RecvError;

    let mut events = state.servers.events.subscribe();
    let waiting: Vec<String> = sqlx::query_scalar(
        "SELECT server_id FROM follows WHERE pending IS NOT NULL",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    for id in waiting {
        tokio::spawn(apply_pending(Arc::clone(&state), id));
    }
    loop {
        match events.recv().await {
            Ok(Lifecycle::Ended(id)) => {
                tokio::spawn(apply_pending(Arc::clone(&state), id));
            }
            Ok(_) | Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => return,
        }
    }
}

/// Apply the version waiting for server `id`, when one waits and the server is stopped. Another
/// change to its content is waited for, so it never costs the version.
async fn apply_pending(state: Arc<CoreState>, id: String) {
    let hold = loop {
        match state.servers.hold(&id) {
            Ok(hold) => break hold,
            Err(ServerError::Installing) => {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(_) => return,
        }
    };
    let taken: Option<(String, bool)> = match sqlx::query_as(
        "UPDATE follows SET state = 'applying', pending = NULL \
         WHERE server_id = ? AND pending IS NOT NULL RETURNING pending, start_after",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(taken) => taken,
        Err(error) => {
            tracing::warn!(server = %id, %error, "could not read the version waiting for the server");
            return;
        }
    };
    let Some((pending, start_after)) = taken else {
        return;
    };
    let applied = match serde_json::from_str(&pending) {
        Ok(data) => apply(&state, hold, data, start_after).await,
        Err(error) => Err(internal(error)),
    };
    if let Err(error) = applied {
        failed(&state, &id, &error.description).await;
    }
}

/// Apply `data` to the held server in the background, after letting go of the share it had: its
/// own, or the one it followed before. Starts the server again after when `start_after`.
async fn apply(
    state: &Arc<CoreState>,
    hold: Hold,
    data: SharedInstanceInstallData,
    start_after: bool,
) -> Result<(), ApiError> {
    let id = hold.id().to_owned();
    let had = engine::instance::get(&state.engine, &id)
        .await
        .map_err(api_error)?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?
        .shared_instance;
    match had {
        Some(own) if own.role == SharedInstanceRole::Owner => {
            engine::instance::unpublish_shared_instance(&state.engine, &id)
                .await
                .map_err(api_error)?;
        }
        Some(followed) if followed.id != data.shared_instance_id => {
            engine::instance::unlink_shared_instance(&state.engine, &id)
                .await
                .map_err(api_error)?;
            engine_mount::forget_players(state, &id, followed.id).await?;
        }
        _ => {}
    }
    let job = engine::install::update_shared_instance(
        id.clone(),
        Some(data),
        &state.engine,
    )
    .await
    .map_err(api_error)?;
    let state = Arc::clone(state);
    tokio::spawn(async move {
        match job_error(&state, &job.job_id).await {
            None => {
                let _ = sqlx::query(
                    "UPDATE follows SET state = NULL, error = NULL WHERE server_id = ?",
                )
                .bind(&id)
                .execute(&state.db)
                .await;
            }
            Some(error) => failed(&state, &id, &error).await,
        }
        engine_mount::finish_job(Arc::clone(&state), hold).await;
        if start_after && let Err(error) = state.servers.start(&id).await {
            tracing::warn!(server = %id, %error, "could not start the server after its update");
        }
    });
    Ok(())
}

/// Wait for install job `job_id` to end, and say why it failed when it did.
async fn job_error(state: &CoreState, job_id: &str) -> Option<String> {
    let job_id = match job_id.parse() {
        Ok(job_id) => job_id,
        Err(error) => return Some(format!("{error}")),
    };
    loop {
        match engine::install::get_job(job_id, &state.engine).await {
            Ok(job) if job.status == InstallJobStatus::Succeeded => {
                return None;
            }
            Ok(job) if job.status.is_finished() => {
                return Some(job.error.map_or_else(
                    || job.status.as_str().to_owned(),
                    |error| error.message,
                ));
            }
            Ok(_) => tokio::time::sleep(Duration::from_secs(1)).await,
            Err(error) => return Some(format!("{error:#}")),
        }
    }
}

async fn failed(state: &CoreState, id: &str, error: &str) {
    tracing::warn!(server = %id, error, "the server could not take its instance's version");
    let _ = sqlx::query(
        "UPDATE follows SET state = 'failed', error = ? WHERE server_id = ?",
    )
    .bind(error)
    .bind(id)
    .execute(&state.db)
    .await;
}

/// Stop following: a server unlinked from its instance's share.
pub(super) async fn forget(state: &CoreState, id: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM follows WHERE server_id = ?")
        .bind(id)
        .execute(&state.db)
        .await
        .map(drop)
}

/// The App instance server `id` follows, with `attachment`, its instance's share: shown while it
/// follows the share, and while a version pushed to it waits, applies or failed.
pub(super) async fn linked(
    state: &CoreState,
    id: &str,
    attachment: Option<&SharedInstanceAttachment>,
) -> Result<Option<LinkedInstance>, ApiError> {
    let row: Option<(String, String, String, Option<String>, Option<String>)> =
        sqlx::query_as(
            "SELECT instance_id, instance_name, shared_instance_id, state, error \
             FROM follows WHERE server_id = ?",
        )
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .map_err(internal)?;
    let Some((instance_id, name, share, update, error)) = row else {
        return Ok(None);
    };
    let version = attachment
        .filter(|attachment| {
            attachment.role.is_member() && attachment.id == share
        })
        .and_then(|attachment| attachment.applied_version);
    let state = match update.as_deref() {
        None if version.is_none() => return Ok(None),
        None => LinkedPackState::UpToDate,
        Some("pending") => LinkedPackState::Pending,
        Some("applying") => LinkedPackState::Applying,
        Some(_) => LinkedPackState::Failed,
    };
    Ok(Some(LinkedInstance {
        id: instance_id,
        name,
        state,
        error,
        shared_instance_id: Some(share),
        version: version.map(i64::from),
    }))
}
