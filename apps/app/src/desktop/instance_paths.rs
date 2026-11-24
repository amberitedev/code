use std::{path::PathBuf, sync::Arc};

use axum::{Router, extract::State};
use contracts::{
    ApiError, Empty, Json,
    server::{Mount, Req},
};
use engine::{Engine, routes::api_error};

use crate::local_api::{
    AppState,
    endpoints::instance_paths::{
        AddProjectFromPath, CheckFileOnModrinth, ExportMrpack,
        GetInstanceContentPath, GetInstancePath, SetInstanceIconFromPath,
    },
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .endpoint::<GetInstancePath, _, _>(get_instance_path)
        .endpoint::<GetInstanceContentPath, _, _>(get_instance_content_path)
        .endpoint::<AddProjectFromPath, _, _>(add_project_from_path)
        .endpoint::<CheckFileOnModrinth, _, _>(check_file_on_modrinth)
        .endpoint::<ExportMrpack, _, _>(export_mrpack)
        .endpoint::<SetInstanceIconFromPath, _, _>(set_instance_icon_from_path)
}

async fn get_instance_path(
    State(engine): State<Arc<Engine>>,
    request: Req<GetInstancePath>,
) -> Result<Json<PathBuf>, ApiError> {
    engine::instance::get_full_path(&engine, &request.params.id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_instance_content_path(
    State(engine): State<Arc<Engine>>,
    request: Req<GetInstanceContentPath>,
) -> Result<Json<PathBuf>, ApiError> {
    engine::instance::get_mod_full_path(
        &engine,
        &request.params.id,
        &request.query.project_path,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

async fn add_project_from_path(
    State(engine): State<Arc<Engine>>,
    request: Req<AddProjectFromPath>,
) -> Result<Json<String>, ApiError> {
    let project = request.body;
    engine::instance::add_project_from_path(
        &engine,
        &request.params.id,
        &project.path,
        project.project_type,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

async fn check_file_on_modrinth(
    State(engine): State<Arc<Engine>>,
    request: Req<CheckFileOnModrinth>,
) -> Result<Json<bool>, ApiError> {
    engine::instance::is_file_on_modrinth(&engine, &request.body.path)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn export_mrpack(
    State(engine): State<Arc<Engine>>,
    request: Req<ExportMrpack>,
) -> Result<Empty, ApiError> {
    let export = request.body;
    engine::instance::export_mrpack(
        &engine,
        &request.params.id,
        export.location,
        export.included_overrides,
        export.excluded_overrides,
        export.version_id,
        export.description,
        export.name,
    )
    .await
    .map(|()| Empty)
    .map_err(api_error)
}

async fn set_instance_icon_from_path(
    State(engine): State<Arc<Engine>>,
    request: Req<SetInstanceIconFromPath>,
) -> Result<Empty, ApiError> {
    engine::instance::edit_icon(
        &engine,
        &request.params.id,
        request.body.path.as_deref(),
    )
    .await
    .map(|()| Empty)
    .map_err(api_error)
}
