use std::{path::Path, sync::Arc};

use uuid::Uuid;

use crate::{
    application::state::AppState,
    domain::modpack::ModpackManifest,
    infrastructure::minecraft::mrpack::{install_mrpack, MrpackError},
    ports::instance_store::StoreError,
};

#[derive(Debug, thiserror::Error)]
pub enum ModpackError {
    #[error("store: {0}")]
    Store(#[from] StoreError),
    #[error("mrpack: {0}")]
    Mrpack(#[from] MrpackError),
    #[error("instance not found")]
    InstanceNotFound,
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

    let instance = state.instance_store.get(&iid).await
        .map_err(|e| match e {
            StoreError::NotFound(_) => ModpackError::InstanceNotFound,
            other => ModpackError::Store(other),
        })?;

    let pack_format = install_mrpack(&state.http, mrpack_path, Path::new(&instance.data_dir)).await?;

    let manifest = ModpackManifest {
        id: Uuid::new_v4().to_string(),
        instance_id: instance_id.to_string(),
        pack_name: pack_format.name.clone(),
        pack_version: pack_format.version_id.clone(),
        game_version: pack_format.dependencies.get("minecraft").cloned().unwrap_or_default(),
        loader: detect_loader(&pack_format.dependencies),
        loader_version: detect_loader_version(&pack_format.dependencies),
        modrinth_project_id: None,
        modrinth_version_id: None,
        installed_at: chrono::Utc::now().to_rfc3339(),
    };

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
    if deps.contains_key("fabric-loader") { "fabric".into() }
    else if deps.contains_key("quilt-loader") { "quilt".into() }
    else if deps.contains_key("forge") { "forge".into() }
    else if deps.contains_key("neoforge") { "neoforge".into() }
    else { "vanilla".into() }
}

fn detect_loader_version(deps: &std::collections::HashMap<String, String>) -> Option<String> {
    deps.get("fabric-loader")
        .or_else(|| deps.get("quilt-loader"))
        .or_else(|| deps.get("forge"))
        .or_else(|| deps.get("neoforge"))
        .cloned()
}
