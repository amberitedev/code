// list_directory, download_file, delete_entry, upload_file, read_file, write_file,
// create_file, create_dir, move_entry, unzip_file, zip_files, copy_files, search_files
use std::{
    io::Write,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use serde::Serialize;
use uuid::Uuid;
use walkdir::WalkDir;
use zip::write::FileOptions;

use crate::{
    application::state::{AppState, FsDownloadToken},
    domain::event::{Event, FsOperationKind},
    domain::instance::InstanceId,
};

#[derive(Debug, Serialize)]
pub struct FsEntry {
    pub name: String,
    pub path: String,
    pub r#type: String,
    pub size: Option<u64>,
    pub modified_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FsListing {
    pub items: Vec<FsEntry>,
    pub total: usize,
    pub current: usize,
}

/// Response from get_download_url — key is valid for 5 minutes.
#[derive(Debug, Serialize)]
pub struct FsDownloadUrlResponse {
    pub key: String,
    pub expires_in: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum FsError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("instance not found")]
    NotFound,
    #[error("path traversal rejected")]
    PathTraversal,
    #[error("not a file")]
    NotAFile,
}

async fn instance_data_dir(
    state: &Arc<AppState>,
    instance_id: &str,
) -> Result<PathBuf, FsError> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT data_dir FROM instances WHERE id = ?")
            .bind(instance_id)
            .fetch_optional(&state.pool)
            .await?;
    let (dir,) = row.ok_or(FsError::NotFound)?;
    Ok(PathBuf::from(dir))
}

/// Resolve and guard a client-provided path against traversal outside `data_dir`.
fn guard_path(data_dir: &Path, client_path: &str) -> Result<PathBuf, FsError> {
    let rel = Path::new(client_path.trim_start_matches('/'));
    for component in rel.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(FsError::PathTraversal);
        }
    }
    Ok(data_dir.join(rel))
}

/// Canonicalize and assert still inside data_dir (catches symlink escapes).
fn guard_canonical(data_dir: &Path, path: &Path) -> Result<PathBuf, FsError> {
    let canonical = path.canonicalize()?;
    let canonical_base = data_dir.canonicalize()?;
    if !canonical.starts_with(&canonical_base) {
        return Err(FsError::PathTraversal);
    }
    Ok(canonical)
}

fn guard_parent_canonical(data_dir: &Path, path: &Path) -> Result<(), FsError> {
    let parent = path.parent().unwrap_or(data_dir);
    std::fs::create_dir_all(parent)?;
    guard_canonical(data_dir, parent)?;
    Ok(())
}

fn guarded_archive_path(
    base: &Path,
    entry_name: &str,
) -> Result<PathBuf, FsError> {
    let rel = Path::new(entry_name);
    for component in rel.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(FsError::PathTraversal);
        }
    }
    Ok(base.join(rel))
}

fn is_zip_symlink(mode: Option<u32>) -> bool {
    mode.map(|mode| mode & 0o170000 == 0o120000)
        .unwrap_or(false)
}

fn emit(
    state: &Arc<AppState>,
    instance_id: &str,
    operation: FsOperationKind,
    path: &Path,
) {
    let id = instance_id
        .parse::<InstanceId>()
        .unwrap_or_else(|_| InstanceId(Uuid::nil()));
    let path_str = path.to_string_lossy().replace('\\', "/");
    state.broadcaster.send(Event::FsChanged {
        instance_id: id,
        operation,
        path: path_str,
    });
}

fn entry_to_json(
    data_dir: &Path,
    path: &Path,
    is_dir: bool,
    meta: &std::fs::Metadata,
) -> FsEntry {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let rel = path
        .strip_prefix(data_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let modified_at = meta
        .modified()
        .ok()
        .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339());
    FsEntry {
        name,
        path: rel,
        r#type: if is_dir {
            "directory".to_string()
        } else {
            "file".to_string()
        },
        size: if is_dir { None } else { Some(meta.len()) },
        modified_at,
    }
}

pub async fn list_directory(
    state: &Arc<AppState>,
    instance_id: &str,
    client_path: &str,
    page: usize,
    page_size: usize,
) -> Result<FsListing, FsError> {
    let data_dir = instance_data_dir(state, instance_id).await?;
    let dir = guard_path(&data_dir, client_path)?;

    let mut rd = tokio::fs::read_dir(&dir).await?;
    let mut dirs: Vec<FsEntry> = vec![];
    let mut files: Vec<FsEntry> = vec![];

    while let Some(e) = rd.next_entry().await? {
        let meta = e.metadata().await?;
        let entry = entry_to_json(&data_dir, &e.path(), meta.is_dir(), &meta);
        if meta.is_dir() {
            dirs.push(entry);
        } else {
            files.push(entry);
        }
    }

    dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    dirs.extend(files);

    let total_items = dirs.len();
    let total_pages = if page_size == 0 {
        1
    } else {
        total_items.div_ceil(page_size)
    };
    let start = page * page_size;
    let items = dirs.into_iter().skip(start).take(page_size).collect();

    Ok(FsListing {
        items,
        total: total_pages,
        current: page,
    })
}

pub async fn download_file(
    state: &Arc<AppState>,
    instance_id: &str,
    client_path: &str,
) -> Result<(Vec<u8>, String), FsError> {
    let data_dir = instance_data_dir(state, instance_id).await?;
    let path = guard_path(&data_dir, client_path)?;
    let canonical = guard_canonical(&data_dir, &path)?;

    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let bytes = tokio::fs::read(&canonical).await?;
    Ok((bytes, filename))
}

pub async fn delete_entry(
    state: &Arc<AppState>,
    instance_id: &str,
    client_path: &str,
    recursive: bool,
) -> Result<(), FsError> {
    let data_dir = instance_data_dir(state, instance_id).await?;
    let path = guard_path(&data_dir, client_path)?;
    let canonical = guard_canonical(&data_dir, &path)?;

    if canonical.is_dir() {
        if recursive {
            tokio::fs::remove_dir_all(&canonical).await?;
        } else {
            tokio::fs::remove_dir(&canonical).await?;
        }
    } else {
        tokio::fs::remove_file(&canonical).await?;
    }
    emit(state, instance_id, FsOperationKind::Delete, &canonical);
    Ok(())
}

pub async fn upload_file(
    state: &Arc<AppState>,
    instance_id: &str,
    target_dir: &str,
    filename: &str,
    data: bytes::Bytes,
) -> Result<(), FsError> {
    if filename.contains("..")
        || filename.contains('/')
        || filename.contains('\\')
    {
        return Err(FsError::PathTraversal);
    }
    let data_dir = instance_data_dir(state, instance_id).await?;
    let dir = guard_path(&data_dir, target_dir)?;
    tokio::fs::create_dir_all(&dir).await?;
    guard_canonical(&data_dir, &dir)?;
    let dest = dir.join(filename);
    tokio::fs::write(&dest, &data).await?;
    emit(state, instance_id, FsOperationKind::Upload, &dest);
    Ok(())
}

pub async fn read_file(
    state: &Arc<AppState>,
    instance_id: &str,
    client_path: &str,
) -> Result<Vec<u8>, FsError> {
    let data_dir = instance_data_dir(state, instance_id).await?;
    let path = guard_path(&data_dir, client_path)?;
    let canonical = guard_canonical(&data_dir, &path)?;
    if canonical.is_dir() {
        return Err(FsError::NotAFile);
    }
    let bytes = tokio::fs::read(&canonical).await?;
    Ok(bytes)
}

pub async fn write_file(
    state: &Arc<AppState>,
    instance_id: &str,
    client_path: &str,
    data: bytes::Bytes,
) -> Result<(), FsError> {
    let data_dir = instance_data_dir(state, instance_id).await?;
    let path = guard_path(&data_dir, client_path)?;
    guard_parent_canonical(&data_dir, &path)?;
    tokio::fs::write(&path, &data).await?;
    emit(state, instance_id, FsOperationKind::Write, &path);
    Ok(())
}

pub async fn create_file(
    state: &Arc<AppState>,
    instance_id: &str,
    client_path: &str,
) -> Result<(), FsError> {
    let data_dir = instance_data_dir(state, instance_id).await?;
    let path = guard_path(&data_dir, client_path)?;
    guard_parent_canonical(&data_dir, &path)?;
    tokio::fs::File::create(&path).await?;
    emit(state, instance_id, FsOperationKind::Create, &path);
    Ok(())
}

pub async fn create_dir(
    state: &Arc<AppState>,
    instance_id: &str,
    client_path: &str,
) -> Result<(), FsError> {
    let data_dir = instance_data_dir(state, instance_id).await?;
    let path = guard_path(&data_dir, client_path)?;
    tokio::fs::create_dir_all(&path).await?;
    guard_canonical(&data_dir, &path)?;
    emit(state, instance_id, FsOperationKind::Create, &path);
    Ok(())
}

pub async fn move_entry(
    state: &Arc<AppState>,
    instance_id: &str,
    from_path: &str,
    to_path: &str,
) -> Result<(), FsError> {
    let data_dir = instance_data_dir(state, instance_id).await?;
    let from = guard_path(&data_dir, from_path)?;
    let to = guard_path(&data_dir, to_path)?;

    let canonical_from = guard_canonical(&data_dir, &from)?;
    guard_parent_canonical(&data_dir, &to)?;
    tokio::fs::rename(&canonical_from, &to).await?;
    emit(
        state,
        instance_id,
        FsOperationKind::Move {
            from: from_path.to_string(),
        },
        &to,
    );
    Ok(())
}

/// Unzip an archive into its containing directory.
/// `option`: "normal" (flat), "smart" (collapse single-root), "to_dir" (named subdir)
pub async fn unzip_file(
    state: &Arc<AppState>,
    instance_id: &str,
    client_path: &str,
    option: UnzipOption,
) -> Result<(), FsError> {
    let data_dir = instance_data_dir(state, instance_id).await?;
    let path = guard_path(&data_dir, client_path)?;
    let canonical = guard_canonical(&data_dir, &path)?;
    let dest_dir = canonical.parent().unwrap_or(&data_dir).to_path_buf();

    tokio::task::spawn_blocking(move || -> Result<(), FsError> {
        let file = std::fs::File::open(&canonical)?;
        let mut archive = zip::ZipArchive::new(file)?;

        let effective_dest = match option {
            UnzipOption::Normal | UnzipOption::Smart => dest_dir.clone(),
            UnzipOption::ToDir => {
                let stem =
                    canonical.file_stem().unwrap_or_default().to_string_lossy();
                dest_dir.join(stem.as_ref())
            }
        };

        std::fs::create_dir_all(&effective_dest)
            .map_err(zip::result::ZipError::Io)?;

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            if is_zip_symlink(entry.unix_mode()) {
                return Err(FsError::PathTraversal);
            }
            let entry_path =
                guarded_archive_path(&effective_dest, entry.name())?;
            if entry.is_dir() {
                std::fs::create_dir_all(&entry_path)?;
            } else {
                guard_parent_canonical(&effective_dest, &entry_path)?;
                let mut out = std::fs::File::create(&entry_path)?;
                std::io::copy(&mut entry, &mut out)?;
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| FsError::Io(std::io::Error::other(e.to_string())))??;

    emit(state, instance_id, FsOperationKind::Unzip, &path);
    Ok(())
}

/// Zip a list of paths into a destination archive.
pub async fn zip_files(
    state: &Arc<AppState>,
    instance_id: &str,
    sources: Vec<String>,
    dest_path: &str,
) -> Result<(), FsError> {
    let data_dir = instance_data_dir(state, instance_id).await?;
    let dest = guard_path(&data_dir, dest_path)?;
    guard_parent_canonical(&data_dir, &dest)?;

    let mut canonical_sources = Vec::new();
    for src in &sources {
        let p = guard_path(&data_dir, src)?;
        canonical_sources.push(guard_canonical(&data_dir, &p)?);
    }

    let dest_clone = dest.clone();
    tokio::task::spawn_blocking(move || -> Result<(), FsError> {
        if let Some(p) = dest_clone.parent() {
            std::fs::create_dir_all(p)?;
        }
        let out_file = std::fs::File::create(&dest_clone)?;
        let mut zip = zip::ZipWriter::new(out_file);
        let options: FileOptions<'_, ()> = FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for src in &canonical_sources {
            if src.is_dir() {
                for entry in
                    WalkDir::new(src).into_iter().filter_map(|e| e.ok())
                {
                    let entry_path = entry.path();
                    if entry.file_type().is_symlink() {
                        continue;
                    }
                    let name = entry_path
                        .strip_prefix(src.parent().unwrap_or(src))
                        .unwrap_or(entry_path);
                    let name_str = name.to_string_lossy().replace('\\', "/");
                    if entry_path.is_dir() {
                        zip.add_directory(&name_str, options)
                            .map_err(|e| FsError::Zip(e))?;
                    } else {
                        zip.start_file(&name_str, options)
                            .map_err(|e| FsError::Zip(e))?;
                        let bytes = std::fs::read(entry_path)?;
                        zip.write_all(&bytes)?;
                    }
                }
            } else {
                let name =
                    src.file_name().unwrap_or_default().to_string_lossy();
                zip.start_file(name.as_ref(), options)
                    .map_err(|e| FsError::Zip(e))?;
                let bytes = std::fs::read(src)?;
                zip.write_all(&bytes)?;
            }
        }
        zip.finish().map_err(|e| FsError::Zip(e))?;
        Ok(())
    })
    .await
    .map_err(|e| FsError::Io(std::io::Error::other(e.to_string())))??;

    emit(state, instance_id, FsOperationKind::Zip, &dest);
    Ok(())
}

/// Copy a list of source paths into a destination directory.
pub async fn copy_files(
    state: &Arc<AppState>,
    instance_id: &str,
    sources: Vec<String>,
    dest_dir_path: &str,
) -> Result<(), FsError> {
    let data_dir = instance_data_dir(state, instance_id).await?;
    let dest_dir = guard_path(&data_dir, dest_dir_path)?;
    tokio::fs::create_dir_all(&dest_dir).await?;
    guard_canonical(&data_dir, &dest_dir)?;

    for src_str in &sources {
        let src = guard_path(&data_dir, src_str)?;
        let canonical = guard_canonical(&data_dir, &src)?;
        let name = canonical.file_name().unwrap_or_default();
        let dest = dest_dir.join(name);
        if canonical.is_dir() {
            fs_extra::dir::copy(
                &canonical,
                &dest_dir,
                &fs_extra::dir::CopyOptions::new().overwrite(true),
            )
            .map_err(|e| FsError::Io(std::io::Error::other(e.to_string())))?;
        } else {
            tokio::fs::copy(&canonical, &dest).await?;
        }
    }
    emit(state, instance_id, FsOperationKind::Copy, &dest_dir);
    Ok(())
}

/// Issue a one-time download token. Tokens expire after 5 minutes.
pub async fn get_download_url(
    state: &Arc<AppState>,
    instance_id: &str,
    client_path: &str,
) -> Result<FsDownloadUrlResponse, FsError> {
    let data_dir = instance_data_dir(state, instance_id).await?;
    let path = guard_path(&data_dir, client_path)?;
    let canonical = guard_canonical(&data_dir, &path)?;

    let key = Uuid::new_v4().to_string();
    let expires_in: u64 = 300;
    state.fs_download_tokens.insert(
        key.clone(),
        FsDownloadToken {
            instance_id: instance_id.to_string(),
            path: canonical,
            expires_at: Instant::now() + Duration::from_secs(expires_in),
        },
    );
    Ok(FsDownloadUrlResponse { key, expires_in })
}

/// Search files by name within an instance's directory.
pub async fn search_files(
    state: &Arc<AppState>,
    instance_id: &str,
    base_path: &str,
    query: &str,
    recursive: bool,
) -> Result<Vec<FsEntry>, FsError> {
    let data_dir = instance_data_dir(state, instance_id).await?;
    let base = guard_path(&data_dir, base_path)?;
    let base = guard_canonical(&data_dir, &base)?;
    let query_lower = query.to_lowercase();

    let mut results = Vec::new();

    if recursive {
        for entry in WalkDir::new(&base).into_iter().filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.contains(&query_lower) {
                if let Ok(meta) = entry.metadata() {
                    results.push(entry_to_json(
                        &data_dir,
                        entry.path(),
                        meta.is_dir(),
                        &meta,
                    ));
                }
            }
        }
    } else {
        let mut rd = tokio::fs::read_dir(&base).await?;
        while let Some(e) = rd.next_entry().await? {
            let name = e.file_name().to_string_lossy().to_lowercase();
            if name.contains(&query_lower) {
                if let Ok(meta) = e.metadata().await {
                    results.push(entry_to_json(
                        &data_dir,
                        &e.path(),
                        meta.is_dir(),
                        &meta,
                    ));
                }
            }
        }
    }

    Ok(results)
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnzipOption {
    Normal,
    Smart,
    ToDir,
}
