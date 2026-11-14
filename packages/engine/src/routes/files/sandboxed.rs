//! The file routes through [`sandbox`]: every path is walked from a handle on the instance's
//! folder, so nothing reaches outside it, links included. A server's file routes all come here;
//! the App's keep the engine's own, which follow the links desktop users make on purpose.
//! Uploads, unpacking and zips never replace server-owned files ([`sandbox::is_server_owned`]);
//! a write is an explicit edit and may change them.

use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

use bytes::Bytes;
use chrono::{DateTime, Utc};
use contracts::{ApiError, ErrorCode};
use tokio::sync::OwnedMutexGuard;

use crate::{
    Engine,
    api::pack::install_from::PackFormat,
    archive::{self, ArchiveError},
    endpoints::files::{
        ExtractDryRunResult, InstanceFileKind, InstanceFileStat,
        InstanceZipRequest,
    },
    instance::InstanceFileItem,
    routes::api_error,
    sandbox::{self, Folder},
    state::instances::adapters::sqlite::instance_rows,
};

pub(super) fn internal(error: impl std::fmt::Display) -> ApiError {
    ApiError::new(ErrorCode::InternalError, error.to_string())
}

pub(super) fn not_found(path: &str) -> ApiError {
    ApiError::new(ErrorCode::NotFound, format!("There is no {path}."))
}

fn archive_error(error: ArchiveError) -> ApiError {
    match error {
        // The disk, not the request: the same one may go once there is room.
        ArchiveError::NoRoom(error) => {
            ApiError::new(ErrorCode::Conflict, error.to_string())
        }
        ArchiveError::Io(error) => internal(error),
        error => ApiError::new(ErrorCode::InvalidInput, error.to_string()),
    }
}

/// A path the caller sent, checked. `None` is the folder itself.
pub(super) fn api_path(path: &str) -> Result<Option<String>, ApiError> {
    sandbox::from_api(path).map_err(|error| {
        ApiError::new(ErrorCode::InvalidInput, error.to_string())
    })
}

/// A path that must name something inside the folder, not the folder itself.
pub(super) fn inner_path(path: &str) -> Result<String, ApiError> {
    api_path(path)?.ok_or_else(|| {
        ApiError::new(ErrorCode::InvalidInput, "Name a file or folder.")
    })
}

pub(super) async fn root(
    engine: &Engine,
    id: &str,
) -> Result<PathBuf, ApiError> {
    let instance = instance_rows::get_instance_by_id(id, &engine.pool)
        .await
        .map_err(api_error)?
        .ok_or_else(|| {
            ApiError::new(ErrorCode::NotFound, format!("There is no {id}."))
        })?;
    engine
        .content_store
        .instance_path(&instance.path, "")
        .await
        .map_err(api_error)
}

pub(super) async fn folder(
    engine: &Engine,
    id: &str,
) -> Result<Arc<Folder>, ApiError> {
    Ok(Arc::new(
        Folder::open(&root(engine, id).await?).map_err(internal)?,
    ))
}

/// The folder, taken for a change: the lock installs hold, so an edit never lands between an
/// install's plan and its writes. Do the change through [`locked`].
pub(super) async fn changing(
    engine: &Engine,
    id: &str,
) -> Result<(Arc<Folder>, OwnedMutexGuard<()>), ApiError> {
    let lock = engine.instance_locks.lock_content(id).await;
    Ok((folder(engine, id).await?, lock))
}

pub(super) async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(work).await.map_err(internal)?
}

/// As [`blocking`], keeping the lock until `work` itself ends: a request that is dropped stops
/// waiting, but the work it started holds the lock to the end.
pub(super) async fn locked<T: Send + 'static>(
    lock: OwnedMutexGuard<()>,
    work: impl FnOnce() -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError> {
    blocking(move || {
        let _lock = lock;
        work()
    })
    .await
}

/// Refuse to replace a server-owned file that exists.
pub(super) fn refuse_owned(
    folder: &Folder,
    path: &str,
) -> Result<(), ApiError> {
    if sandbox::is_server_owned(path, &folder.level_name())
        && folder.exists(path)
    {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            format!(
                "{path} belongs to the server; uploads never replace it. Edit it instead."
            ),
        ));
    }
    Ok(())
}

fn since_epoch(time: std::io::Result<cap_std::time::SystemTime>) -> u64 {
    time.ok()
        .map(cap_std::time::SystemTime::into_std)
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_secs())
}

pub(super) async fn list(
    engine: &Engine,
    id: &str,
    path: &str,
) -> Result<Vec<InstanceFileItem>, ApiError> {
    let path = api_path(path)?;
    let folder = folder(engine, id).await?;
    blocking(move || {
        let dir = folder
            .open_dir(path.as_deref())
            .map_err(|_| not_found(path.as_deref().unwrap_or("folder")))?;
        let mut items = Vec::new();
        for entry in dir.entries().map_err(internal)?.filter_map(Result::ok) {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let Ok(meta) = entry.metadata() else { continue };
            let kind = if meta.file_type().is_symlink() {
                "symlink"
            } else if meta.is_dir() {
                "directory"
            } else {
                "file"
            };
            let count = (kind == "directory")
                .then(|| {
                    entry.open_dir().ok()?.entries().ok().map(Iterator::count)
                })
                .flatten();
            items.push(InstanceFileItem {
                path: path
                    .as_ref()
                    .map_or(name.clone(), |path| format!("{path}/{name}")),
                name,
                kind: kind.to_owned(),
                modified: since_epoch(meta.modified()),
                created: since_epoch(meta.created()),
                size: (kind == "file").then(|| meta.len()),
                count,
                read_only: false,
            });
        }
        Ok(items)
    })
    .await
}

pub(super) async fn read(
    engine: &Engine,
    id: &str,
    path: &str,
) -> Result<Vec<u8>, ApiError> {
    let path = inner_path(path)?;
    let folder = folder(engine, id).await?;
    blocking(move || {
        // A link or something other than a file reads as missing; any other error, such as a
        // file the engine may not read, says what it is.
        let mut file = match folder.open_file(&path) {
            Ok(Some(file)) => file,
            Ok(None) => return Err(not_found(&path)),
            Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {
                return Err(not_found(&path));
            }
            Err(error) => return Err(internal(error)),
        };
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(internal)?;
        Ok(bytes)
    })
    .await
}

pub(super) async fn write(
    engine: &Engine,
    id: &str,
    path: &str,
    bytes: Bytes,
    create_only: bool,
) -> Result<(), ApiError> {
    let path = inner_path(path)?;
    let (folder, lock) = changing(engine, id).await?;
    locked(lock, move || {
        if create_only && folder.exists(&path) {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                format!("{path} already exists."),
            ));
        }
        folder.write(&path, &mut bytes.as_ref()).map_err(internal)?;
        Ok(())
    })
    .await
}

pub(super) async fn create_directory(
    engine: &Engine,
    id: &str,
    path: &str,
) -> Result<(), ApiError> {
    let path = inner_path(path)?;
    let (folder, lock) = changing(engine, id).await?;
    locked(lock, move || {
        folder.create_dir_all(&path).map_err(internal)?;
        Ok(())
    })
    .await
}

/// Never onto something that exists.
pub(super) async fn rename(
    engine: &Engine,
    id: &str,
    from: &str,
    to: &str,
) -> Result<(), ApiError> {
    let from = inner_path(from)?;
    let to = inner_path(to)?;
    let (folder, lock) = changing(engine, id).await?;
    locked(lock, move || {
        if !folder.exists(&from) {
            return Err(not_found(&from));
        }
        if folder.exists(&to) {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                format!("{to} already exists."),
            ));
        }
        folder.rename(&from, &to).map_err(internal)
    })
    .await
}

/// A folder with something in it needs `recursive`.
pub(super) async fn delete(
    engine: &Engine,
    id: &str,
    path: &str,
    recursive: bool,
) -> Result<(), ApiError> {
    let path = inner_path(path)?;
    let (folder, lock) = changing(engine, id).await?;
    locked(lock, move || {
        let meta = folder.metadata(&path).map_err(|_| not_found(&path))?;
        if meta.is_dir() && !recursive {
            folder.remove_empty_dir(&path).map_err(|_| {
                ApiError::new(
                    ErrorCode::Conflict,
                    format!("{path} is not empty."),
                )
            })?;
        } else {
            folder.remove(&path).map_err(internal)?;
        }
        Ok(())
    })
    .await
}

/// Unpack the zip at `source` into `target`, by default the zip's own folder. A dry run only
/// lists the files that exist already; otherwise they are replaced when `overwrite` says so,
/// except server-owned ones, which are never replaced. A pack's name is reported so the caller
/// can offer to install it as one.
pub(super) async fn extract(
    engine: &Engine,
    id: &str,
    source: &str,
    target: Option<&str>,
    overwrite: bool,
    dry: bool,
) -> Result<Option<ExtractDryRunResult>, ApiError> {
    let source = inner_path(source)?;
    let target = match target {
        Some(target) => api_path(target)?,
        None => source.rsplit_once('/').map(|(parent, _)| parent.to_owned()),
    };
    let room = archive::room(&root(engine, id).await?).map_err(internal)?;
    let (folder, lock) = changing(engine, id).await?;
    locked(lock, move || {
        let file = folder
            .open_file(&source)
            .ok()
            .flatten()
            .ok_or_else(|| not_found(&source))?
            .into_std();
        let mut zip = zip::ZipArchive::new(file)
            .map_err(|error| archive_error(error.into()))?;
        let destination = |entry: &str| match &target {
            Some(target) => format!("{target}/{entry}"),
            None => entry.to_owned(),
        };
        let entries = archive::entries(&mut zip).map_err(archive_error)?;
        if dry {
            let modpack_name = zip
                .by_name("modrinth.index.json")
                .ok()
                .and_then(|index| {
                    serde_json::from_reader::<_, PackFormat>(index).ok()
                })
                .map(|index| index.name);
            let conflicting_files = entries
                .iter()
                .filter(|(_, is_dir)| !is_dir)
                .map(|(entry, _)| destination(entry))
                .filter(|path| folder.exists(path))
                .collect();
            return Ok(Some(ExtractDryRunResult {
                modpack_name,
                conflicting_files,
            }));
        }
        let level = folder.level_name();
        // Paths nothing was at before, so a partway failure can take them away again. Files it
        // replaced stay replaced: what they held is already gone.
        let mut created = Vec::new();
        let extracted = archive::extract(&folder, &mut zip, room, |entry| {
            let path = destination(entry);
            let exists = folder.exists(&path);
            let owned = sandbox::is_server_owned(&path, &level);
            if !exists {
                created.push(path.clone());
            }
            (!exists || (overwrite && !owned)).then_some(path)
        });
        if let Err(error) = extracted {
            for path in created.iter().rev() {
                let _ = folder.remove(path);
            }
            return Err(archive_error(error));
        }
        Ok(None)
    })
    .await
}

pub(super) async fn stat(
    engine: &Engine,
    id: &str,
    path: &str,
) -> Result<InstanceFileStat, ApiError> {
    let path = api_path(path)?;
    let folder = folder(engine, id).await?;
    blocking(move || {
        let meta = match &path {
            Some(path) => folder.metadata(path),
            None => folder.open_dir(None).and_then(|dir| dir.dir_metadata()),
        }
        .map_err(|_| not_found(path.as_deref().unwrap_or("folder")))?;
        let time = |time: std::io::Result<cap_std::time::SystemTime>| {
            DateTime::<Utc>::from_timestamp(since_epoch(time) as i64, 0)
                .unwrap_or_default()
        };
        let name = path
            .as_deref()
            .and_then(|path| path.rsplit('/').next())
            .unwrap_or_default()
            .to_owned();
        Ok(InstanceFileStat {
            name,
            path: path.unwrap_or_default(),
            size: meta.len(),
            kind: if meta.file_type().is_symlink() {
                InstanceFileKind::Symlink
            } else if meta.is_dir() {
                InstanceFileKind::Directory
            } else if meta.is_file() {
                InstanceFileKind::Regular
            } else {
                InstanceFileKind::Other
            },
            modified: time(meta.modified()),
            created: time(meta.created()),
        })
    })
    .await
}

/// Where files wait before they go into an instance: outside every instance folder.
pub(super) fn staging(engine: &Engine) -> PathBuf {
    engine.directories.caches_dir().join("staging")
}

/// Add `dir` to `zip` under `prefix`, skipping links.
fn zip_tree(
    zip: &mut zip::ZipWriter<std::fs::File>,
    dir: &cap_std::fs::Dir,
    prefix: &str,
) -> Result<(), ApiError> {
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(true);
    for entry in dir.entries().map_err(internal)?.filter_map(Result::ok) {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let kind = entry.file_type().map_err(internal)?;
        let path = format!("{prefix}{name}");
        if kind.is_dir() {
            zip.add_directory(format!("{path}/"), options)
                .map_err(internal)?;
            zip_tree(
                zip,
                &sandbox::open_child(dir, &name, &path).map_err(internal)?,
                &format!("{path}/"),
            )?;
        } else if kind.is_file() {
            zip.start_file(&path, options).map_err(internal)?;
            std::io::copy(
                &mut sandbox::open_plain(dir, &name, &path)
                    .map_err(internal)?,
                zip,
            )
            .map_err(internal)?;
        }
    }
    Ok(())
}

/// Zip a folder into `<folder>.zip`, or paths inside `parent` into `target`.
pub(super) async fn zip(
    engine: &Engine,
    id: &str,
    request: InstanceZipRequest,
) -> Result<(), ApiError> {
    let (parent, include, target) = match request {
        InstanceZipRequest::Directory { path } => {
            let path = inner_path(&path)?;
            let (parent, name) = path
                .rsplit_once('/')
                .map_or((None, path.clone()), |(parent, name)| {
                    (Some(parent.to_owned()), name.to_owned())
                });
            (parent, vec![name], format!("{path}.zip"))
        }
        InstanceZipRequest::ManyPaths {
            parent,
            include,
            target,
        } => {
            let include = include
                .iter()
                .map(|name| sandbox::clean(name.trim_matches('/')))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| {
                    ApiError::new(ErrorCode::InvalidInput, error.to_string())
                })?;
            (api_path(&parent)?, include, inner_path(&target)?)
        }
    };
    let staged = staging(engine).join(format!("zip-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(staging(engine))
        .await
        .map_err(internal)?;
    let (folder, lock) = changing(engine, id).await?;
    refuse_owned(&folder, &target)?;
    let result = locked(lock, {
        let staged = staged.clone();
        move || {
            let mut zip = zip::ZipWriter::new(
                std::fs::File::create(&staged).map_err(internal)?,
            );
            for name in &include {
                let path = match &parent {
                    Some(parent) => format!("{parent}/{name}"),
                    None => name.clone(),
                };
                let meta =
                    folder.metadata(&path).map_err(|_| not_found(name))?;
                if meta.is_dir() {
                    zip.add_directory(
                        format!("{name}/"),
                        zip::write::SimpleFileOptions::default(),
                    )
                    .map_err(internal)?;
                    zip_tree(
                        &mut zip,
                        &folder.open_dir(Some(&path)).map_err(internal)?,
                        &format!("{name}/"),
                    )?;
                } else if meta.is_file() {
                    zip.start_file(
                        name,
                        zip::write::SimpleFileOptions::default()
                            .large_file(true),
                    )
                    .map_err(internal)?;
                    let mut file = folder
                        .open_file(&path)
                        .map_err(internal)?
                        .ok_or_else(|| not_found(name))?;
                    std::io::copy(&mut file, &mut zip).map_err(internal)?;
                }
            }
            zip.finish().map_err(internal)?;
            folder
                .write(
                    &target,
                    &mut std::fs::File::open(&staged).map_err(internal)?,
                )
                .map_err(internal)?;
            Ok(())
        }
    })
    .await;
    let _ = tokio::fs::remove_file(&staged).await;
    result
}

/// Write a staged file into `folder`.
pub(super) fn place(
    folder: &Folder,
    path: &str,
    staged: &Path,
) -> Result<(), ApiError> {
    folder
        .write(path, &mut std::fs::File::open(staged).map_err(internal)?)
        .map_err(internal)?;
    Ok(())
}
