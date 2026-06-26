use std::{collections::HashMap, path::Path, sync::Arc};

use bytes::Bytes;
use serde::Serialize;
use sha2::Digest;

use crate::{
    application::{social_models::SocialError, state::AppState},
    domain::modpack::PackFormat,
    infrastructure::minecraft::mrpack::validate_download_url,
};

#[derive(Debug, Serialize)]
pub struct SyncDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub updated: Vec<String>,
    pub unchanged: Vec<String>,
}

struct DesiredMod {
    filename: String,
    sha512: Option<String>,
    sha1: Option<String>,
    source: ModSource,
}

enum ModSource {
    Download(String),
    Archive(Bytes),
}

pub async fn apply_snapshot(
    state: &Arc<AppState>,
    instance_id: &str,
    archive_path: &Path,
    metadata: &PackFormat,
) -> Result<SyncDiff, SocialError> {
    let data_dir: String =
        sqlx::query_scalar("SELECT data_dir FROM instances WHERE id = ?")
            .bind(instance_id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or(SocialError::NotFound)?;
    let mods_dir = Path::new(&data_dir).join("mods");
    tokio::fs::create_dir_all(&mods_dir).await?;

    let desired = desired_mods(archive_path, metadata).await?;
    let current = current_mod_hashes(&mods_dir).await?;
    let mut diff = SyncDiff {
        added: vec![],
        removed: vec![],
        updated: vec![],
        unchanged: vec![],
    };

    for filename in current.keys() {
        if !desired.contains_key(filename) {
            remove_mod_file(&mods_dir, filename).await?;
            sqlx::query(
                "DELETE FROM mods WHERE instance_id = ? AND filename = ?",
            )
            .bind(instance_id)
            .bind(filename)
            .execute(&state.pool)
            .await?;
            diff.removed.push(filename.clone());
        }
    }

    for desired_mod in desired.values() {
        let current_hash = current.get(&desired_mod.filename);
        if desired_mod
            .sha512
            .as_ref()
            .is_some_and(|sha| current_hash == Some(sha))
        {
            diff.unchanged.push(desired_mod.filename.clone());
            continue;
        }
        let bytes = mod_bytes(state, desired_mod).await?;
        verify_hashes(desired_mod, &bytes)?;
        let sha512 = hex::encode(sha2::Sha512::digest(&bytes));
        let tmp = mods_dir.join(format!("{}.tmp", desired_mod.filename));
        tokio::fs::write(&tmp, &bytes).await?;
        remove_mod_file(&mods_dir, &desired_mod.filename).await?;
        tokio::fs::rename(tmp, mods_dir.join(&desired_mod.filename)).await?;
        upsert_mod_row(state, instance_id, &desired_mod.filename, &sha512)
            .await?;
        if current_hash.is_some() {
            diff.updated.push(desired_mod.filename.clone());
        } else {
            diff.added.push(desired_mod.filename.clone());
        }
    }
    Ok(diff)
}

async fn desired_mods(
    archive_path: &Path,
    metadata: &PackFormat,
) -> Result<HashMap<String, DesiredMod>, SocialError> {
    let mut desired = HashMap::new();
    for file in &metadata.files {
        if super::sync_mrpack_files::is_client_only(file)
            || !file.path.starts_with("mods/")
            || !file.path.ends_with(".jar")
        {
            continue;
        }
        let filename = file.path.trim_start_matches("mods/").to_string();
        validate_filename(&filename)?;
        let url = file.downloads.first().ok_or_else(|| {
            SocialError::Invalid(format!(
                "mod file has no download: {}",
                file.path
            ))
        })?;
        validate_download_url(url)?;
        desired.insert(
            filename.clone(),
            DesiredMod {
                filename,
                sha512: file.hashes.sha512.clone(),
                sha1: file.hashes.sha1.clone(),
                source: ModSource::Download(url.clone()),
            },
        );
    }
    for (filename, data) in
        super::sync_mrpack_files::archive_mods(archive_path).await?
    {
        let sha512 = hex::encode(sha2::Sha512::digest(&data));
        desired.insert(
            filename.clone(),
            DesiredMod {
                filename,
                sha512: Some(sha512),
                sha1: Some(hex::encode(sha1::Sha1::digest(&data))),
                source: ModSource::Archive(Bytes::from(data)),
            },
        );
    }
    Ok(desired)
}

async fn current_mod_hashes(
    mods_dir: &Path,
) -> Result<HashMap<String, String>, SocialError> {
    let mut current = HashMap::new();
    let mut entries = tokio::fs::read_dir(mods_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let filename = entry.file_name().to_string_lossy().to_string();
        if !filename.ends_with(".jar") && !filename.ends_with(".jar.disabled") {
            continue;
        }
        let canonical = filename.trim_end_matches(".disabled").to_string();
        let bytes = tokio::fs::read(entry.path()).await?;
        current.insert(canonical, hex::encode(sha2::Sha512::digest(&bytes)));
    }
    Ok(current)
}

async fn mod_bytes(
    state: &Arc<AppState>,
    desired: &DesiredMod,
) -> Result<Bytes, SocialError> {
    match &desired.source {
        ModSource::Archive(bytes) => Ok(bytes.clone()),
        ModSource::Download(url) => Ok(state
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| SocialError::Invalid(e.to_string()))?
            .error_for_status()
            .map_err(|e| SocialError::Invalid(e.to_string()))?
            .bytes()
            .await
            .map_err(|e| SocialError::Invalid(e.to_string()))?),
    }
}

fn verify_hashes(
    desired: &DesiredMod,
    bytes: &[u8],
) -> Result<(), SocialError> {
    if let Some(expected) = &desired.sha1 {
        let actual = hex::encode(sha1::Sha1::digest(bytes));
        if &actual != expected {
            return Err(SocialError::Invalid(format!(
                "sha1 mismatch for {}",
                desired.filename
            )));
        }
    }
    if let Some(expected) = &desired.sha512 {
        let actual = hex::encode(sha2::Sha512::digest(bytes));
        if &actual != expected {
            return Err(SocialError::Invalid(format!(
                "sha512 mismatch for {}",
                desired.filename
            )));
        }
    }
    Ok(())
}

async fn remove_mod_file(
    mods_dir: &Path,
    filename: &str,
) -> Result<(), SocialError> {
    let _ = tokio::fs::remove_file(mods_dir.join(filename)).await;
    let _ =
        tokio::fs::remove_file(mods_dir.join(format!("{filename}.disabled")))
            .await;
    Ok(())
}

async fn upsert_mod_row(
    state: &Arc<AppState>,
    instance_id: &str,
    filename: &str,
    sha512: &str,
) -> Result<(), SocialError> {
    sqlx::query("INSERT INTO mods (id, instance_id, filename, sha512, enabled, installed_at) VALUES (?, ?, ?, ?, 1, ?) ON CONFLICT(instance_id, filename) DO UPDATE SET sha512 = excluded.sha512, enabled = 1")
		.bind(uuid::Uuid::new_v4().to_string())
		.bind(instance_id)
		.bind(filename)
		.bind(sha512)
		.bind(chrono::Utc::now().to_rfc3339())
		.execute(&state.pool)
		.await?;
    Ok(())
}

fn validate_filename(name: &str) -> Result<(), SocialError> {
    if name.is_empty()
        || name.contains("..")
        || name.contains('/')
        || name.contains('\\')
    {
        return Err(SocialError::Invalid(
            "invalid mod filename in mrpack".into(),
        ));
    }
    Ok(())
}
