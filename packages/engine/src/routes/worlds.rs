use std::{
    path::{Component, Path},
    sync::Arc,
};

use axum::extract::State;
use contracts::{ApiError, Empty, ErrorCode, Json, server::Req};

use super::{api_error, input_error};
use crate::{
    Engine,
    access::{EngineAccess, InstanceAccess},
    endpoints::worlds::{
        AddInstanceServer, BackupWorld, DeleteWorld, DesyncInstanceServer,
        EditInstanceServer, EnsureManagedInstanceServer,
        GetInstanceProtocolVersion, GetServerStatus, GetSingleplayerWorld,
        RemoveInstanceServer, RenameWorld, ResetWorldIcon,
        SetWorldDisplayStatus,
    },
    instance::get_full_path,
    worlds::{self, ProtocolVersion, ServerStatus, World},
};

pub async fn get_server_status(
    _: EngineAccess,
    request: Req<GetServerStatus>,
) -> Result<Json<ServerStatus>, ApiError> {
    let protocol_version =
        request
            .query
            .protocol_version
            .map(|version| ProtocolVersion {
                version,
                legacy: request.query.legacy,
            });
    worlds::get_server_status(&request.query.address, protocol_version)
        .await
        .map(Json)
        .map_err(input_error)
}

pub async fn set_world_display_status(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<SetWorldDisplayStatus>,
) -> Result<Empty, ApiError> {
    worlds::set_world_display_status(
        &engine,
        &instance.id,
        request.body.world_type,
        &request.body.world_id,
        request.body.display_status,
    )
    .await
    .map(|()| Empty)
    .map_err(api_error)
}

pub async fn get_instance_protocol_version(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    _: Req<GetInstanceProtocolVersion>,
) -> Result<Json<Option<ProtocolVersion>>, ApiError> {
    worlds::get_instance_protocol_version(&engine, &instance.id)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn get_singleplayer_world(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<GetSingleplayerWorld>,
) -> Result<Json<World>, ApiError> {
    let world = world_folder(&request.params.world)?;
    worlds::get_singleplayer_world(&engine, &instance.id, world)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn delete_world(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<DeleteWorld>,
) -> Result<Empty, ApiError> {
    let path = get_full_path(&engine, &instance.id)
        .await
        .map_err(api_error)?;
    worlds::delete_world(&path, world_folder(&request.params.world)?)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

pub async fn rename_world(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<RenameWorld>,
) -> Result<Empty, ApiError> {
    let path = get_full_path(&engine, &instance.id)
        .await
        .map_err(api_error)?;
    let world = world_folder(&request.params.world)?;
    worlds::rename_world(&path, world, &request.body.name)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

pub async fn reset_world_icon(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<ResetWorldIcon>,
) -> Result<Empty, ApiError> {
    let path = get_full_path(&engine, &instance.id)
        .await
        .map_err(api_error)?;
    worlds::reset_world_icon(&path, world_folder(&request.params.world)?)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

pub async fn backup_world(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<BackupWorld>,
) -> Result<Json<u64>, ApiError> {
    let path = get_full_path(&engine, &instance.id)
        .await
        .map_err(api_error)?;
    worlds::backup_world(&path, world_folder(&request.params.world)?)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn add_instance_server(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<AddInstanceServer>,
) -> Result<Json<usize>, ApiError> {
    let server = request.body;
    worlds::add_server_to_instance(
        &engine,
        &instance.id,
        server.name,
        server.address,
        server.pack_status,
        server.project_id,
        server.content_kind,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn ensure_managed_instance_server(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<EnsureManagedInstanceServer>,
) -> Result<Empty, ApiError> {
    worlds::ensure_managed_server_in_instance(
        &engine,
        &instance.id,
        request.body.name,
        request.body.address,
    )
    .await
    .map(|()| Empty)
    .map_err(api_error)
}

/// Needs Owner over the engine: desyncing can drop the server from every participating instance.
pub async fn desync_instance_server(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess,
    instance: InstanceAccess,
    request: Req<DesyncInstanceServer>,
) -> Result<Empty, ApiError> {
    engine
        .hooks()
        .desync_server(
            &engine,
            &instance.id,
            &request.body.server_id,
            request.body.mode,
        )
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

pub async fn edit_instance_server(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<EditInstanceServer>,
) -> Result<Empty, ApiError> {
    let server = request.body;
    worlds::edit_server_in_instance(
        &engine,
        &instance.id,
        request.params.index,
        server.name,
        server.address,
        server.pack_status,
    )
    .await
    .map(|()| Empty)
    .map_err(api_error)
}

pub async fn remove_instance_server(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<RemoveInstanceServer>,
) -> Result<Empty, ApiError> {
    worlds::remove_server_from_instance(
        &engine,
        &instance.id,
        request.params.index,
    )
    .await
    .map(|()| Empty)
    .map_err(api_error)
}

/// The world's folder in `saves`, refused when it names any other path.
fn world_folder(world: &str) -> Result<&str, ApiError> {
    let mut components = Path::new(world).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(world),
        _ => Err(ApiError::new(
            ErrorCode::InvalidInput,
            "a world is one folder in saves",
        )),
    }
}
