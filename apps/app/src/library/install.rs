//! New instances and modpack installs from a file on this desktop.

use std::sync::Arc;

use axum::{Router, extract::State};
use contracts::{
    ApiError, Json,
    server::{Req, route},
};
use engine::{
    Engine, data::InstanceLink, install::InstallJobSnapshot, routes::api_error,
};

use crate::server::{
    AppState,
    endpoints::install::{
        CreateInstance, CreateModpackInstance, InstallModpackPreview,
        InstallPackToInstance, PreviewModpack,
    },
};

pub fn routes() -> Router<AppState> {
    [
        route::<PreviewModpack, _, _, _>(preview_modpack),
        route::<CreateInstance, _, _, _>(create_instance),
        route::<CreateModpackInstance, _, _, _>(create_modpack_instance),
        route::<InstallPackToInstance, _, _, _>(install_pack_to_instance),
    ]
    .into_iter()
    .fold(Router::new(), |router, (path, method)| {
        router.route(path, method)
    })
}

async fn preview_modpack(
    State(engine): State<Arc<Engine>>,
    request: Req<PreviewModpack>,
) -> Result<Json<InstallModpackPreview>, ApiError> {
    engine::pack::install_from::get_instance_from_pack(&engine, request.body)
        .await
        .map(|preview| Json(preview.into()))
        .map_err(api_error)
}

async fn create_instance(
    State(engine): State<Arc<Engine>>,
    request: Req<CreateInstance>,
) -> Result<Json<InstallJobSnapshot>, ApiError> {
    let request = request.body;
    let link = match request.link {
        Some(link) => link.into_core().map_err(api_error)?,
        None => InstanceLink::Unmanaged,
    };
    engine::install::create_instance(
        request.name.trim().to_string(),
        request.game_version,
        request.loader,
        request.loader_version,
        request.icon_path,
        request.icon_config,
        link,
        None,
        None,
        &engine,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

async fn create_modpack_instance(
    State(engine): State<Arc<Engine>>,
    request: Req<CreateModpackInstance>,
) -> Result<Json<InstallJobSnapshot>, ApiError> {
    let install = request.body;
    let edit = install
        .post_install_edit
        .map(|edit| edit.into_core())
        .transpose()
        .map_err(api_error)?;
    engine::install::create_modpack_instance(install.location, edit, &engine)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn install_pack_to_instance(
    State(engine): State<Arc<Engine>>,
    request: Req<InstallPackToInstance>,
) -> Result<Json<InstallJobSnapshot>, ApiError> {
    let install = request.body;
    let edit = install
        .post_install_edit
        .map(|edit| edit.into_core())
        .transpose()
        .map_err(api_error)?;
    engine::install::install_pack_to_existing_instance(
        request.params.id,
        install.location,
        edit,
        &engine,
    )
    .await
    .map(Json)
    .map_err(api_error)
}
