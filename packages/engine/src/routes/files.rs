use std::{
    io::{Cursor, Read},
    sync::Arc,
};

use anyhow::Context;
use axum::extract::State;
use contracts::{ApiError, Empty, ErrorCode, Json, Raw, server::Req};

use super::api_error;
use crate::{
    Engine, ErrorKind,
    access::{Edit, InstanceAccess},
    endpoints::files::{
        CreateInstanceDirectory, DeleteInstanceFile, ExtractDryRunResult,
        ExtractInstanceZip, ListInstanceFiles, MoveInstanceFile,
        ReadInstanceFile, WriteInstanceFile,
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
    let bytes = instance::read_instance_file(&engine, &instance.id, path)
        .await
        .map_err(api_error)?;
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
    extract_zip(
        &engine,
        &instance.id,
        inside_instance(&body.path)?,
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
    override_conflicts: bool,
    dry_run: bool,
) -> crate::Result<Option<ExtractDryRunResult>> {
    instance::validate_instance_file_write(engine, instance_id, file_path)
        .await?;
    let parent = file_path
        .trim_start_matches('/')
        .rsplit_once('/')
        .map_or("", |(parent, _)| parent);
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
