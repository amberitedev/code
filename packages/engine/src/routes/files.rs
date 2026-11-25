//! The Files tab's routes on both sides, through [`sandboxed`], and the instance's icon.

mod sandboxed;
mod uploads;

use std::sync::Arc;

use axum::{
    body::Body,
    extract::State,
    http::header::{CONTENT_LENGTH, CONTENT_TYPE},
    response::{IntoResponse, Response},
};
use contracts::{ApiError, Empty, ErrorCode, Json, Raw, server::Req};
use tokio_util::io::ReaderStream;

pub use self::uploads::Uploads;
use super::api_error;
use crate::{
    Engine,
    access::InstanceAccess,
    endpoints::files::{
        AddInstanceUploadFile, CancelInstanceUpload, CreateInstanceDirectory,
        DeleteInstanceFile, ExtractDryRunResult, ExtractInstanceZip,
        FinalizeInstanceUpload, GetInstanceIcon, GetInstanceUpload,
        InstanceFileStat, InstanceUpload, ListInstanceFiles, MoveInstanceFile,
        OpenInstanceUpload, ReadInstanceFile, RemoveInstanceIcon,
        SetEditedInstanceIcon, SetInstanceIcon, StatInstanceFile,
        WriteInstanceFile, ZipInstanceFiles,
    },
    instance::{self, InstanceFileItem},
    state::content_store::validate_relative,
};

/// A caller's path stays inside the instance's folder. The engine refuses one that leaves it too,
/// but as an internal error. A leading `/` is the folder itself, as the engine reads it.
pub(super) fn inside_instance(path: &str) -> Result<&str, ApiError> {
    let relative = path.trim_start_matches('/');
    if relative.is_empty() || validate_relative(relative).is_ok() {
        Ok(path)
    } else {
        Err(ApiError::new(
            ErrorCode::InvalidInput,
            format!("{path:?} is not a path inside the instance's folder"),
        ))
    }
}

pub async fn list_instance_files(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<ListInstanceFiles>,
) -> Result<Json<Vec<InstanceFileItem>>, ApiError> {
    sandboxed::list(&engine, &instance.id, &request.query.path)
        .await
        .map(Json)
}

pub async fn read_instance_file(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<ReadInstanceFile>,
) -> Result<Response, ApiError> {
    let file = sandboxed::open_file(&engine, &instance.id, &request.query.path)
        .await?;
    let length = file.metadata().map_err(sandboxed::internal)?.len();
    Ok((
        [
            (CONTENT_TYPE, "application/octet-stream".to_owned()),
            (CONTENT_LENGTH, length.to_string()),
        ],
        Body::from_stream(ReaderStream::new(tokio::fs::File::from_std(file))),
    )
        .into_response())
}

pub async fn write_instance_file(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<WriteInstanceFile>,
) -> Result<Empty, ApiError> {
    sandboxed::write(
        &engine,
        &instance.id,
        &request.query.path,
        request.body.bytes,
        request.query.create_only,
    )
    .await
    .map(|()| Empty)
}

pub async fn create_instance_directory(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<CreateInstanceDirectory>,
) -> Result<Empty, ApiError> {
    sandboxed::create_directory(&engine, &instance.id, &request.query.path)
        .await
        .map(|()| Empty)
}

pub async fn move_instance_file(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<MoveInstanceFile>,
) -> Result<Empty, ApiError> {
    sandboxed::rename(
        &engine,
        &instance.id,
        &request.body.source,
        &request.body.destination,
    )
    .await
    .map(|()| Empty)
}

pub async fn delete_instance_file(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<DeleteInstanceFile>,
) -> Result<Empty, ApiError> {
    sandboxed::delete(
        &engine,
        &instance.id,
        &request.query.path,
        request.query.recursive,
    )
    .await
    .map(|()| Empty)
}

pub async fn extract_instance_zip(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<ExtractInstanceZip>,
) -> Result<Json<Option<ExtractDryRunResult>>, ApiError> {
    let body = request.body;
    sandboxed::extract(
        &engine,
        &instance.id,
        &body.path,
        body.target.as_deref(),
        body.override_conflicts,
        body.dry_run,
    )
    .await
    .map(Json)
}

pub async fn stat_instance_file(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<StatInstanceFile>,
) -> Result<Json<InstanceFileStat>, ApiError> {
    sandboxed::stat(&engine, &instance.id, &request.query.path)
        .await
        .map(Json)
}

pub async fn zip_instance_files(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<ZipInstanceFiles>,
) -> Result<Empty, ApiError> {
    sandboxed::zip(&engine, &instance.id, request.body)
        .await
        .map(|()| Empty)
}

pub async fn open_instance_upload(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<OpenInstanceUpload>,
) -> Json<InstanceUpload> {
    Json(
        uploads::open(
            &engine,
            &instance.caller,
            &instance.id,
            request.body.scope,
        )
        .await,
    )
}

pub async fn get_instance_upload(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<GetInstanceUpload>,
) -> Json<Option<InstanceUpload>> {
    Json(uploads::get(
        &engine,
        &instance.caller,
        &instance.id,
        request.query.scope,
    ))
}

pub async fn add_instance_upload_file(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<AddInstanceUploadFile>,
) -> Result<Json<InstanceUpload>, ApiError> {
    uploads::add_file(
        &engine,
        &instance.caller,
        &instance.id,
        &request.params.upload,
        &request.query.path,
        request.body,
    )
    .await
    .map(Json)
}

pub async fn finalize_instance_upload(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<FinalizeInstanceUpload>,
) -> Result<Json<InstanceUpload>, ApiError> {
    uploads::finalize(
        &engine,
        &instance.caller,
        &instance.id,
        &request.params.upload,
    )
    .await
    .map(Json)
}

pub async fn cancel_instance_upload(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<CancelInstanceUpload>,
) -> Result<Json<InstanceUpload>, ApiError> {
    uploads::cancel(
        &engine,
        &instance.caller,
        &instance.id,
        &request.params.upload,
    )
    .await
    .map(Json)
}

fn no_icon() -> ApiError {
    ApiError::new(ErrorCode::NotFound, "This instance has no icon.")
}

pub async fn get_instance_icon(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    _: Req<GetInstanceIcon>,
) -> Result<Raw, ApiError> {
    let bytes = instance::read_icon(&engine, &instance.id)
        .await
        .map_err(api_error)?
        .ok_or_else(no_icon)?;
    Ok(Raw {
        content_type: "image/png".to_owned(),
        bytes: bytes.into(),
    })
}

pub async fn set_instance_icon(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<SetInstanceIcon>,
) -> Result<Empty, ApiError> {
    instance::edit_icon_image(
        &engine,
        &instance.id,
        request.body.bytes.into(),
        None,
    )
    .await
    .map(|()| Empty)
    .map_err(api_error)
}

pub async fn set_edited_instance_icon(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    request: Req<SetEditedInstanceIcon>,
) -> Result<Empty, ApiError> {
    let body = request.body;
    instance::edit_icon_image(
        &engine,
        &instance.id,
        body.image,
        Some(body.config),
    )
    .await
    .map(|()| Empty)
    .map_err(api_error)
}

pub async fn remove_instance_icon(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
    _: Req<RemoveInstanceIcon>,
) -> Result<Empty, ApiError> {
    if instance::remove_icon(&engine, &instance.id)
        .await
        .map_err(api_error)?
    {
        Ok(Empty)
    } else {
        Err(no_icon())
    }
}
