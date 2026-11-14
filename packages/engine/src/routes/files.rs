mod sandboxed;
mod uploads;

use std::{
    io::{Cursor, Read},
    sync::Arc,
};

use anyhow::Context;
use axum::extract::State;
use contracts::{ApiError, Empty, ErrorCode, Json, Raw, Side, server::Req};

pub use self::uploads::Uploads;
use super::api_error;
use crate::{
    Engine, ErrorKind,
    access::{Edit, InstanceAccess, View},
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
    instance: InstanceAccess<Edit>,
    request: Req<ListInstanceFiles>,
) -> Result<Json<Vec<InstanceFileItem>>, ApiError> {
    let path = inside_instance(&request.query.path)?;
    if engine.side == Side::Server {
        return sandboxed::list(&engine, &instance.id, path).await.map(Json);
    }
    instance::list_instance_files(&engine, &instance.id, path)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn read_instance_file(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<ReadInstanceFile>,
) -> Result<Raw, ApiError> {
    let path = inside_instance(&request.query.path)?;
    let bytes = if engine.side == Side::Server {
        sandboxed::read(&engine, &instance.id, path).await?
    } else {
        instance::read_instance_file(&engine, &instance.id, path)
            .await
            .map_err(api_error)?
    };
    Ok(Raw {
        content_type: "application/octet-stream".to_owned(),
        bytes: bytes.into(),
    })
}

pub async fn write_instance_file(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<WriteInstanceFile>,
) -> Result<Empty, ApiError> {
    if engine.side == Side::Server {
        return sandboxed::write(
            &engine,
            &instance.id,
            inside_instance(&request.query.path)?,
            request.body.bytes,
            request.query.create_only,
        )
        .await
        .map(|()| Empty);
    }
    instance::write_instance_file(
        &engine,
        &instance.id,
        inside_instance(&request.query.path)?,
        &request.body.bytes,
        request.query.create_only,
    )
    .await
    .map(|()| Empty)
    .map_err(api_error)
}

pub async fn create_instance_directory(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<CreateInstanceDirectory>,
) -> Result<Empty, ApiError> {
    let path = inside_instance(&request.query.path)?;
    if engine.side == Side::Server {
        return sandboxed::create_directory(&engine, &instance.id, path)
            .await
            .map(|()| Empty);
    }
    instance::create_instance_directory(&engine, &instance.id, path)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

pub async fn move_instance_file(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<MoveInstanceFile>,
) -> Result<Empty, ApiError> {
    if engine.side == Side::Server {
        return sandboxed::rename(
            &engine,
            &instance.id,
            inside_instance(&request.body.source)?,
            inside_instance(&request.body.destination)?,
        )
        .await
        .map(|()| Empty);
    }
    instance::rename_instance_file(
        &engine,
        &instance.id,
        inside_instance(&request.body.source)?,
        inside_instance(&request.body.destination)?,
    )
    .await
    .map(|()| Empty)
    .map_err(api_error)
}

pub async fn delete_instance_file(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<DeleteInstanceFile>,
) -> Result<Empty, ApiError> {
    if engine.side == Side::Server {
        return sandboxed::delete(
            &engine,
            &instance.id,
            inside_instance(&request.query.path)?,
            request.query.recursive,
        )
        .await
        .map(|()| Empty);
    }
    instance::delete_instance_file(
        &engine,
        &instance.id,
        inside_instance(&request.query.path)?,
        request.query.recursive,
    )
    .await
    .map(|()| Empty)
    .map_err(api_error)
}

pub async fn extract_instance_zip(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<ExtractInstanceZip>,
) -> Result<Json<Option<ExtractDryRunResult>>, ApiError> {
    let body = request.body;
    let path = inside_instance(&body.path)?;
    let target = body.target.as_deref().map(inside_instance).transpose()?;
    if engine.side == Side::Server {
        return sandboxed::extract(
            &engine,
            &instance.id,
            path,
            target,
            body.override_conflicts,
            body.dry_run,
        )
        .await
        .map(Json);
    }
    extract_zip(
        &engine,
        &instance.id,
        path,
        target,
        body.override_conflicts,
        body.dry_run,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

async fn extract_zip(
    engine: &Engine,
    instance_id: &str,
    file_path: &str,
    target: Option<&str>,
    override_conflicts: bool,
    dry_run: bool,
) -> crate::Result<Option<ExtractDryRunResult>> {
    instance::validate_instance_file_write(engine, instance_id, file_path)
        .await?;
    let parent = match target {
        Some(target) => target.trim_matches('/'),
        None => file_path
            .trim_start_matches('/')
            .rsplit_once('/')
            .map_or("", |(parent, _)| parent),
    };
    let file_bytes =
        instance::read_instance_file(engine, instance_id, file_path).await?;
    let (mut zip_reader, names) = tokio::task::spawn_blocking(move || {
        let mut archive = zip::ZipArchive::new(Cursor::new(file_bytes))
            .context("failed to read zip file")?;
        let mut names = Vec::new();
        for index in 0..archive.len() {
            let entry = archive
                .by_index_raw(index)
                .context("failed to read archive entry")?;
            let name = entry
                .name()
                .context(ErrorKind::InputError(
                    "invalid archive entry name".to_string(),
                ))?
                .into_owned();
            names.push((index, name));
        }
        Ok::<_, anyhow::Error>((archive, names))
    })
    .await
    .context("failed to read zip file")??;
    let mut entries = Vec::new();
    for (index, name) in names {
        if name.ends_with('/') {
            continue;
        }
        if name.starts_with('/') || name.contains('\\') {
            return Err(ErrorKind::InputError(
                "invalid archive path".to_string(),
            )
            .into());
        }
        let target = if parent.is_empty() {
            name.to_string()
        } else {
            format!("{parent}/{name}")
        };
        let resolved = instance::validate_instance_file_write(
            engine,
            instance_id,
            &target,
        )
        .await?;
        entries.push((index, target, resolved));
    }
    if dry_run {
        let conflicting_files = entries
            .iter()
            .filter(|(_, _, path)| path.exists())
            .map(|(_, name, _)| name.clone())
            .collect();
        return Ok(Some(ExtractDryRunResult {
            modpack_name: None,
            conflicting_files,
        }));
    }

    for (index, path, resolved) in entries {
        if !override_conflicts && resolved.exists() {
            continue;
        }
        let entry_path = path.clone();
        let (archive, bytes) = tokio::task::spawn_blocking(move || {
            let mut bytes = Vec::new();
            {
                let mut reader =
                    zip_reader.by_index(index).with_context(|| {
                        format!("failed to read archive entry {entry_path}")
                    })?;
                reader.read_to_end(&mut bytes).with_context(|| {
                    format!("failed to read archive entry {entry_path}")
                })?;
            }
            Ok::<_, anyhow::Error>((zip_reader, bytes))
        })
        .await
        .with_context(|| format!("failed to read archive entry {path}"))??;
        zip_reader = archive;
        instance::write_instance_file(
            engine,
            instance_id,
            &path,
            &bytes,
            !override_conflicts,
        )
        .await?;
    }
    Ok(None)
}

pub async fn stat_instance_file(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<StatInstanceFile>,
) -> Result<Json<InstanceFileStat>, ApiError> {
    let path = inside_instance(&request.query.path)?;
    sandboxed::stat(&engine, &instance.id, path).await.map(Json)
}

pub async fn zip_instance_files(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<ZipInstanceFiles>,
) -> Result<Empty, ApiError> {
    sandboxed::zip(&engine, &instance.id, request.body)
        .await
        .map(|()| Empty)
}

pub async fn open_instance_upload(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
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
    instance: InstanceAccess<Edit>,
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
    instance: InstanceAccess<Edit>,
    request: Req<AddInstanceUploadFile>,
) -> Result<Json<InstanceUpload>, ApiError> {
    uploads::add_file(
        &engine,
        &instance.caller,
        &instance.id,
        &request.params.upload,
        inside_instance(&request.query.path)?,
        request.body,
    )
    .await
    .map(Json)
}

pub async fn finalize_instance_upload(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
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
    instance: InstanceAccess<Edit>,
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
    instance: InstanceAccess<View>,
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
    instance: InstanceAccess<Edit>,
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
    instance: InstanceAccess<Edit>,
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
    instance: InstanceAccess<Edit>,
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
