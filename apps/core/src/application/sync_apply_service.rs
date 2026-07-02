use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::Arc,
};

use bytes::Bytes;
use serde::Serialize;
use sha2::Digest;

use crate::{
    application::{
        social_models::SocialError,
        state::AppState,
        sync_mrpack_plan::{
            self, InvalidOverride, PlannedModSource, PlannedModState,
            PlannedServerMod,
        },
    },
    domain::modpack::PackFormat,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub updated: Vec<String>,
    pub unchanged: Vec<String>,
    pub invalid_overrides: Vec<InvalidOverride>,
}

struct CurrentMod {
    sha512: String,
    enabled: bool,
}

pub async fn apply_snapshot(
    state: &Arc<AppState>,
    instance_id: &str,
    archive_path: &Path,
    metadata: &PackFormat,
    previous_archive_path: Option<&Path>,
    previous_metadata: Option<&PackFormat>,
) -> Result<SyncDiff, SocialError> {
    let data_dir: String =
        sqlx::query_scalar("SELECT data_dir FROM instances WHERE id = ?")
            .bind(instance_id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or(SocialError::NotFound)?;
    let mods_dir = Path::new(&data_dir).join("mods");
    tokio::fs::create_dir_all(&mods_dir).await?;

    let plan = sync_mrpack_plan::server_plan(archive_path, metadata).await?;
    let desired_names = plan.mods.keys().cloned().collect::<HashSet<_>>();
    let previous_managed =
        previous_managed_names(previous_archive_path, previous_metadata)
            .await?;
    let current = current_mods(&mods_dir).await?;
    let mut diff = SyncDiff {
        added: vec![],
        removed: vec![],
        updated: vec![],
        unchanged: vec![],
        invalid_overrides: plan.invalid_overrides,
    };

    let mut removals = plan.removed;
    for filename in previous_managed {
        if !desired_names.contains(&filename) {
            removals.insert(filename);
        }
    }

    for filename in sorted_strings(removals) {
        validate_filename(&filename)?;
        let existed_on_disk = current.contains_key(&filename);
        remove_mod_file(&mods_dir, &filename).await?;
        let deleted = sqlx::query(
            "DELETE FROM mods WHERE instance_id = ? AND filename = ?",
        )
        .bind(instance_id)
        .bind(&filename)
        .execute(&state.pool)
        .await?
        .rows_affected()
            > 0;
        if existed_on_disk || deleted {
            diff.removed.push(filename.clone());
        }
    }

    for desired_mod in sorted_mods(plan.mods) {
        let current_mod = current.get(&desired_mod.filename);
        if desired_mod.sha512.as_ref().is_some_and(|sha| {
            current_mod.is_some_and(|current| &current.sha512 == sha)
        }) {
            let desired_enabled =
                matches!(desired_mod.state, PlannedModState::Enabled);
            if current_mod
                .is_some_and(|current| current.enabled == desired_enabled)
            {
                upsert_mod_row(
                    state,
                    instance_id,
                    &desired_mod,
                    desired_mod.sha512.as_deref().unwrap(),
                )
                .await?;
                diff.unchanged.push(desired_mod.filename.clone());
            } else {
                set_existing_mod_state(
                    &mods_dir,
                    &desired_mod.filename,
                    desired_enabled,
                )
                .await?;
                upsert_mod_row(
                    state,
                    instance_id,
                    &desired_mod,
                    desired_mod.sha512.as_deref().unwrap(),
                )
                .await?;
                diff.updated.push(desired_mod.filename.clone());
            }
            continue;
        }
        let bytes = mod_bytes(state, &desired_mod).await?;
        verify_hashes(&desired_mod, &bytes)?;
        let sha512 = hex::encode(sha2::Sha512::digest(&bytes));
        write_mod_file(&mods_dir, &desired_mod, &bytes).await?;
        upsert_mod_row(state, instance_id, &desired_mod, &sha512).await?;
        if current_mod.is_some() {
            diff.updated.push(desired_mod.filename.clone());
        } else {
            diff.added.push(desired_mod.filename.clone());
        }
    }
    Ok(diff)
}

async fn previous_managed_names(
    archive_path: Option<&Path>,
    metadata: Option<&PackFormat>,
) -> Result<HashSet<String>, SocialError> {
    if let Some(metadata) = metadata {
        sync_mrpack_plan::installed_managed_names(archive_path, metadata).await
    } else {
        Ok(HashSet::new())
    }
}

async fn current_mods(
    mods_dir: &Path,
) -> Result<HashMap<String, CurrentMod>, SocialError> {
    let mut current = HashMap::new();
    let mut entries = tokio::fs::read_dir(mods_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let filename = entry.file_name().to_string_lossy().to_string();
        let (canonical, enabled) = if filename.ends_with(".jar") {
            (filename.clone(), true)
        } else if filename.ends_with(".jar.disabled") {
            (filename.trim_end_matches(".disabled").to_string(), false)
        } else {
            continue;
        };
        if !enabled
            && current
                .get(&canonical)
                .is_some_and(|current: &CurrentMod| current.enabled)
        {
            continue;
        }
        let bytes = tokio::fs::read(entry.path()).await?;
        current.insert(
            canonical,
            CurrentMod {
                sha512: hex::encode(sha2::Sha512::digest(&bytes)),
                enabled,
            },
        );
    }
    Ok(current)
}

async fn mod_bytes(
    state: &Arc<AppState>,
    desired: &PlannedServerMod,
) -> Result<Bytes, SocialError> {
    match &desired.source {
        PlannedModSource::Archive(bytes) => Ok(bytes.clone()),
        PlannedModSource::Download(url) => Ok(state
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
    desired: &PlannedServerMod,
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

async fn write_mod_file(
    mods_dir: &Path,
    desired: &PlannedServerMod,
    bytes: &[u8],
) -> Result<(), SocialError> {
    let tmp = mods_dir.join(format!("{}.tmp", desired.filename));
    tokio::fs::write(&tmp, bytes).await?;
    remove_mod_file(mods_dir, &desired.filename).await?;
    let dest = if matches!(desired.state, PlannedModState::Enabled) {
        mods_dir.join(&desired.filename)
    } else {
        mods_dir.join(format!("{}.disabled", desired.filename))
    };
    tokio::fs::rename(tmp, dest).await?;
    Ok(())
}

async fn set_existing_mod_state(
    mods_dir: &Path,
    filename: &str,
    enabled: bool,
) -> Result<(), SocialError> {
    let jar = mods_dir.join(filename);
    let disabled = mods_dir.join(format!("{filename}.disabled"));
    if enabled {
        if disabled.exists() {
            let _ = tokio::fs::remove_file(&jar).await;
            tokio::fs::rename(disabled, jar).await?;
        }
    } else if jar.exists() {
        let _ = tokio::fs::remove_file(&disabled).await;
        tokio::fs::rename(jar, disabled).await?;
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
    desired: &PlannedServerMod,
    sha512: &str,
) -> Result<(), SocialError> {
    sqlx::query("INSERT INTO mods (id, instance_id, filename, modrinth_project_id, modrinth_version_id, sha512, enabled, installed_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(instance_id, filename) DO UPDATE SET modrinth_project_id = excluded.modrinth_project_id, modrinth_version_id = excluded.modrinth_version_id, sha512 = excluded.sha512, enabled = excluded.enabled")
		.bind(uuid::Uuid::new_v4().to_string())
		.bind(instance_id)
		.bind(&desired.filename)
        .bind(&desired.project_id)
        .bind(&desired.version_id)
		.bind(sha512)
        .bind(matches!(desired.state, PlannedModState::Enabled) as i64)
		.bind(chrono::Utc::now().to_rfc3339())
		.execute(&state.pool)
		.await?;
    Ok(())
}

fn sorted_mods(
    mods: HashMap<String, PlannedServerMod>,
) -> Vec<PlannedServerMod> {
    let mut mods = mods.into_values().collect::<Vec<_>>();
    mods.sort_by(|a, b| a.filename.cmp(&b.filename));
    mods
}

fn sorted_strings(values: HashSet<String>) -> Vec<String> {
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort();
    values
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
