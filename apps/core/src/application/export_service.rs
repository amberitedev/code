use std::{io::Write, sync::Arc};

use serde_json::json;
use sqlx::Row;
use zip::write::SimpleFileOptions;

use crate::application::state::AppState;

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("db: {0}")] Db(#[from] sqlx::Error),
    #[error("io: {0}")] Io(#[from] std::io::Error),
    #[error("zip: {0}")] Zip(String),
    #[error("instance not found")] InstanceNotFound,
}

impl From<zip::result::ZipError> for ExportError {
    fn from(e: zip::result::ZipError) -> Self {
        Self::Zip(e.to_string())
    }
}

/// Export an instance's mods as a `.mrpack` zip archive.
/// Returns `(zip_bytes, filename)`.
pub async fn export_modpack(
    state: &Arc<AppState>,
    instance_id: &str,
) -> Result<(bytes::Bytes, String), ExportError> {
    let row: Option<(String, String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT name, game_version, loader, loader_version, data_dir \
         FROM instances WHERE id = ?",
    )
    .bind(instance_id)
    .fetch_optional(&state.pool)
    .await?;
    let (name, game_version, loader, loader_version, data_dir_str) =
        row.ok_or(ExportError::InstanceNotFound)?;
    let data_dir = std::path::PathBuf::from(&data_dir_str);

    let mod_rows = sqlx::query(
        "SELECT filename, modrinth_project_id, modrinth_version_id, sha512 \
         FROM mods WHERE instance_id = ?",
    )
    .bind(instance_id)
    .fetch_all(&state.pool)
    .await?;

    let mut files = vec![];
    let mut private_mods: Vec<(String, std::path::PathBuf)> = vec![];

    for row in &mod_rows {
        let filename: String = row.get("filename");
        let pid: Option<String> = row.get("modrinth_project_id");
        let sha: Option<String> = row.get("sha512");
        if let (Some(pid), Some(sha)) = (pid, sha) {
            files.push(json!({
                "path": format!("mods/{filename}"),
                "hashes": { "sha512": sha },
                "downloads": [format!("https://cdn.modrinth.com/data/{pid}/versions/{filename}")],
                "fileSize": 0,
            }));
        } else {
            private_mods.push((filename.clone(), data_dir.join("mods").join(filename)));
        }
    }

    let index = json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": format!("{game_version}-{loader}"),
        "name": name,
        "dependencies": {
            "minecraft": game_version,
            loader: loader_version.as_deref().unwrap_or(""),
        },
        "files": files,
    });

    let index_bytes = serde_json::to_vec_pretty(&index).unwrap_or_default();
    let config_dir = data_dir.join("config");
    let pack_name = name.clone();

    let zip_bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, ExportError> {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let opts = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        zip.start_file("modrinth.index.json", opts)?;
        zip.write_all(&index_bytes)?;

        for (filename, path) in &private_mods {
            if let Ok(data) = std::fs::read(path) {
                zip.start_file(format!("overrides/mods/{filename}"), opts)?;
                zip.write_all(&data)?;
            }
        }

        if config_dir.exists() {
            for entry in walkdir::WalkDir::new(&config_dir).into_iter().flatten() {
                if !entry.file_type().is_file() {
                    continue;
                }
                if let Ok(rel) = entry.path().strip_prefix(&config_dir) {
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    zip.start_file(format!("overrides/config/{rel_str}"), opts)?;
                    if let Ok(data) = std::fs::read(entry.path()) {
                        zip.write_all(&data)?;
                    }
                }
            }
        }

        let cursor = zip.finish()?;
        Ok(cursor.into_inner())
    })
    .await
    .map_err(|e| ExportError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))??;

    let filename = format!("{}.mrpack", sanitize_filename::sanitize(&pack_name));
    Ok((bytes::Bytes::from(zip_bytes), filename))
}
