use std::sync::Arc;

use crate::{
    application::state::AppState,
    domain::modpack::ModpackManifest,
    infrastructure::minecraft::{
        modrinth_api::ModrinthError, mrpack::MrpackError,
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
