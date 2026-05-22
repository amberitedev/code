use std::{io::Write, path::Path, sync::Arc};

use sha1::Digest;
use uuid::Uuid;

use crate::{
    application::state::AppState,
    domain::modpack::ModpackManifest,
    infrastructure::minecraft::{
        modrinth_api::{ModrinthClient, ModrinthError},
        mrpack::{install_mrpack, MrpackError},
    },
    ports::instance_store::StoreError,
};

#[derive(Debug, thiserror::Error)]
pub enum ModpackError {
    #[error("store: {0}")]
    Store(#[from] StoreError),
    #[error("mrpack: {0}")]
    Mrpack(#[from] MrpackError),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("modrinth: {0}")]
    Modrinth(#[from] ModrinthError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("instance not found")]
    InstanceNotFound,
    #[error("version has no downloadable file")]
    MissingFile,
}

/// Install a `.mrpack` file to an instance and persist the manifest.
pub async fn install(
    state: &Arc<AppState>,
    instance_id: &str,
    mrpack_path: &Path,
) -> Result<ModpackManifest, ModpackError> {
    let instance_uuid: uuid::Uuid = instance_id
        .parse()
        .map_err(|_| ModpackError::InstanceNotFound)?;
    let iid = crate::domain::instance::InstanceId(instance_uuid);

    let instance =
        state.instance_store.get(&iid).await.map_err(|e| match e {
            StoreError::NotFound(_) => ModpackError::InstanceNotFound,
            other => ModpackError::Store(other),
        })?;

    let pack_format =
        install_mrpack(&state.http, mrpack_path, Path::new(&instance.data_dir))
            .await?;

    let manifest = ModpackManifest {
        id: Uuid::new_v4().to_string(),
        instance_id: instance_id.to_string(),
        pack_name: pack_format.name.clone(),
        pack_version: pack_format.version_id.clone(),
        game_version: pack_format
            .dependencies
            .get("minecraft")
            .cloned()
            .unwrap_or_default(),
        loader: detect_loader(&pack_format.dependencies),
        loader_version: detect_loader_version(&pack_format.dependencies),
        modrinth_project_id: None,
        modrinth_version_id: None,
        installed_at: chrono::Utc::now().to_rfc3339(),
    };

    state.modpack_store.save(&manifest).await?;
    Ok(manifest)
}

/// Download and install a Modrinth `.mrpack` version to an instance.
pub async fn install_modrinth_version(
    state: &Arc<AppState>,
    instance_id: &str,
    project_id: &str,
    version_id: &str,
) -> Result<ModpackManifest, ModpackError> {
    let modrinth = ModrinthClient::new(state.http.clone());
    let version = modrinth.get_version(version_id).await?;
    let file = version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
        .ok_or(ModpackError::MissingFile)?;
    let bytes = state
        .http
        .get(&file.url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    let expected_sha1 = file
        .hashes
        .sha1
        .as_deref()
        .ok_or(ModpackError::MissingFile)?;
    let actual = hex::encode(sha1::Sha1::digest(&bytes));
    if actual != expected_sha1 {
        return Err(ModpackError::Mrpack(MrpackError::HashMismatch {
            path: file.filename.clone(),
            expected: expected_sha1.to_string(),
            actual,
        }));
    }
    let mut tmp = tempfile::NamedTempFile::new()?;
    tmp.write_all(&bytes)?;

    let mut manifest = install(state, instance_id, tmp.path()).await?;
    manifest.modrinth_project_id = Some(project_id.to_string());
    manifest.modrinth_version_id = Some(version_id.to_string());
    state.modpack_store.save(&manifest).await?;
    Ok(manifest)
}

/// Get the installed modpack manifest for an instance, if any.
pub async fn get_manifest(
    state: &Arc<AppState>,
    instance_id: &str,
) -> Result<Option<ModpackManifest>, ModpackError> {
    Ok(state.modpack_store.get_for_instance(instance_id).await?)
}

/// Remove the modpack manifest for an instance.
pub async fn remove(
    state: &Arc<AppState>,
    instance_id: &str,
) -> Result<(), ModpackError> {
    state.modpack_store.delete_for_instance(instance_id).await?;
    Ok(())
}

fn detect_loader(deps: &std::collections::HashMap<String, String>) -> String {
    if deps.contains_key("fabric-loader") {
        "fabric".into()
    } else if deps.contains_key("quilt-loader") {
        "quilt".into()
    } else if deps.contains_key("forge") {
        "forge".into()
    } else if deps.contains_key("neoforge") {
        "neoforge".into()
    } else {
        "vanilla".into()
    }
}

fn detect_loader_version(
    deps: &std::collections::HashMap<String, String>,
) -> Option<String> {
    deps.get("fabric-loader")
        .or_else(|| deps.get("quilt-loader"))
        .or_else(|| deps.get("forge"))
        .or_else(|| deps.get("neoforge"))
        .cloned()
}
