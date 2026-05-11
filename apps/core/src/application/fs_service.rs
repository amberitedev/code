use std::{path::{Component, Path, PathBuf}, sync::Arc};

use serde::Serialize;

use crate::application::state::AppState;

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

#[derive(Debug, thiserror::Error)]
pub enum FsError {
	#[error("io: {0}")] Io(#[from] std::io::Error),
	#[error("db: {0}")] Db(#[from] sqlx::Error),
	#[error("instance not found")] NotFound,
	#[error("path traversal rejected")] PathTraversal,
}

async fn instance_data_dir(state: &Arc<AppState>, instance_id: &str) -> Result<PathBuf, FsError> {
	let row: Option<(String,)> = sqlx::query_as("SELECT data_dir FROM instances WHERE id = ?")
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
		if matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_)) {
			return Err(FsError::PathTraversal);
		}
	}
	Ok(data_dir.join(rel))
}

fn entry_to_json(data_dir: &Path, path: &Path, is_dir: bool, meta: &std::fs::Metadata) -> FsEntry {
	let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
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
		r#type: if is_dir { "directory".to_string() } else { "file".to_string() },
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
	let total_pages = if page_size == 0 { 1 } else { total_items.div_ceil(page_size) };
	let start = page * page_size;
	let items = dirs.into_iter().skip(start).take(page_size).collect();

	Ok(FsListing { items, total: total_pages, current: page })
}

pub async fn download_file(
	state: &Arc<AppState>,
	instance_id: &str,
	client_path: &str,
) -> Result<(Vec<u8>, String), FsError> {
	let data_dir = instance_data_dir(state, instance_id).await?;
	let path = guard_path(&data_dir, client_path)?;

	// Extra guard: canonicalize to catch symlink escapes.
	let canonical = path.canonicalize()?;
	let canonical_base = data_dir.canonicalize()?;
	if !canonical.starts_with(&canonical_base) {
		return Err(FsError::PathTraversal);
	}

	let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
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
	let canonical = path.canonicalize()?;
	let canonical_base = data_dir.canonicalize()?;
	if !canonical.starts_with(&canonical_base) {
		return Err(FsError::PathTraversal);
	}

	if canonical.is_dir() {
		if recursive {
			tokio::fs::remove_dir_all(&canonical).await?;
		} else {
			tokio::fs::remove_dir(&canonical).await?;
		}
	} else {
		tokio::fs::remove_file(&canonical).await?;
	}
	Ok(())
}

pub async fn upload_file(
	state: &Arc<AppState>,
	instance_id: &str,
	target_dir: &str,
	filename: &str,
	data: bytes::Bytes,
) -> Result<(), FsError> {
	if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
		return Err(FsError::PathTraversal);
	}
	let data_dir = instance_data_dir(state, instance_id).await?;
	let dir = guard_path(&data_dir, target_dir)?;
	tokio::fs::create_dir_all(&dir).await?;
	let dest = dir.join(filename);
	tokio::fs::write(&dest, &data).await?;
	Ok(())
}
