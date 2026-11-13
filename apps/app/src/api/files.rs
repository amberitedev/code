use crate::api::Result;
use crate::engine_handle::EngineHandle;
use anyhow::Context;
use engine::ErrorKind;
use serde::Serialize;
use std::io::{Cursor, Read};
use tauri::Runtime;

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("files")
        .invoke_handler(tauri::generate_handler![
            file_extract_zip,
            file_list,
            file_read,
            file_write,
            file_create_directory,
            file_rename,
            file_delete,
        ])
        .build()
}

#[derive(Serialize)]
pub struct ExtractDryRunResult {
    modpack_name: Option<String>,
    conflicting_files: Vec<String>,
}

#[tauri::command]
pub async fn file_extract_zip(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    file_path: &str,
    override_conflicts: bool,
    dry_run: bool,
) -> Result<Option<ExtractDryRunResult>> {
    let state = handle.get().await;
    engine::instance::validate_instance_file_write(
        &state,
        instance_id,
        file_path,
    )
    .await?;
    let parent = file_path
        .trim_start_matches('/')
        .rsplit_once('/')
        .map_or("", |(parent, _)| parent);
    let file_bytes =
        engine::instance::read_instance_file(&state, instance_id, file_path)
            .await?;
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
        let resolved = engine::instance::validate_instance_file_write(
            &state,
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
        engine::instance::write_instance_file(
            &state,
            instance_id,
            &path,
            &bytes,
            !override_conflicts,
        )
        .await?;
    }
    Ok(None)
}

#[tauri::command]
pub async fn file_list(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    path: &str,
) -> Result<Vec<engine::instance::InstanceFileItem>> {
    let state = handle.get().await;
    Ok(
        engine::instance::list_instance_files(&state, instance_id, path)
            .await?,
    )
}

#[tauri::command]
pub async fn file_read(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    path: &str,
) -> Result<Vec<u8>> {
    let state = handle.get().await;
    Ok(engine::instance::read_instance_file(&state, instance_id, path).await?)
}

#[tauri::command]
pub async fn file_write(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    path: &str,
    bytes: Vec<u8>,
    create_only: bool,
) -> Result<()> {
    let state = handle.get().await;
    Ok(engine::instance::write_instance_file(
        &state,
        instance_id,
        path,
        &bytes,
        create_only,
    )
    .await?)
}

#[tauri::command]
pub async fn file_create_directory(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    path: &str,
) -> Result<()> {
    let state = handle.get().await;
    Ok(
        engine::instance::create_instance_directory(&state, instance_id, path)
            .await?,
    )
}

#[tauri::command]
pub async fn file_rename(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    source: &str,
    destination: &str,
) -> Result<()> {
    let state = handle.get().await;
    Ok(engine::instance::rename_instance_file(
        &state,
        instance_id,
        source,
        destination,
    )
    .await?)
}

#[tauri::command]
pub async fn file_delete(
    handle: tauri::State<'_, EngineHandle>,
    instance_id: &str,
    path: &str,
    recursive: bool,
) -> Result<()> {
    let state = handle.get().await;
    Ok(engine::instance::delete_instance_file(
        &state,
        instance_id,
        path,
        recursive,
    )
    .await?)
}
