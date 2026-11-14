//! A server's `server.properties`. Editors and the owner only: the file can hold secrets, such as
//! a resource pack URL.

use std::sync::Arc;

use axum::extract::State;
use contracts::{
    ApiError, Json,
    core::{Action, ChangeProperties, GetProperties, ServerProperties},
    server::Req,
};

use super::{CoreState, internal};
use crate::{
    access::{Caller, Permission, activity},
    servers::properties,
};

pub async fn get(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, .. }: Req<GetProperties>,
) -> Result<Json<ServerProperties>, ApiError> {
    state
        .access
        .require(&caller, &params.id, Permission::Edit)
        .await?;
    let dir = state.servers.paths().server(&params.id);
    Ok(Json(ServerProperties {
        properties: properties::read(&dir).map_err(internal)?,
    }))
}

/// Takes effect at the next start.
pub async fn change(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, body, .. }: Req<ChangeProperties>,
) -> Result<Json<ServerProperties>, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id, Permission::Edit).await?;
    let dir = state.servers.paths().server(&id);
    properties::patch(&dir, &body.properties).map_err(internal)?;
    activity::log(&state.db, &caller, Some(&id), Action::SettingsChanged).await;
    Ok(Json(ServerProperties {
        properties: properties::read(&dir).map_err(internal)?,
    }))
}
