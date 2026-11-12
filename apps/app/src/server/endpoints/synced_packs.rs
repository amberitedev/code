//! Resource and data packs shared between instances.

use std::path::PathBuf;

use contracts::{Empty, Json};
use engine::{
    data::ContentItem,
    instance::{DesyncServerMode, PackSyncPreview},
    prelude::ProjectType,
};
use serde::{Deserialize, Serialize};

use super::IdParams;

endpoints! {
    ListSyncedPacks: Get "/app/synced-packs", (), SyncedPackQuery, Empty
        => Json<SyncedPacks>;
    /// Adds the pack file at `path` to the shared packs.
    UploadSyncedPack: Post "/app/synced-packs", (), (), Json<NewSyncedPack>
        => Empty;
    SetSyncedPackEnabled: Patch "/app/synced-packs/{id}", IdParams, (),
        Json<SyncedPackUpdate> => Empty;
    RemoveSyncedPack: Delete "/app/synced-packs/{id}", IdParams, (), Empty
        => Empty;
    /// Which instances an instance's pack would reach if it were shared.
    PreviewPackSync: Get "/app/instances/{id}/synced-packs/preview", IdParams,
        InstancePack, Empty => Json<PackSyncPreview>;
    /// Shares one of the instance's packs.
    SyncPack: Post "/app/instances/{id}/synced-packs", IdParams, (),
        Json<InstancePack> => Empty;
    /// Stops sharing a pack with the instance.
    DesyncPack: Delete "/app/instances/{id}/synced-packs/{pack_id}",
        InstancePackParams, DesyncQuery, Empty => Empty;
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum SyncedPackType {
    ResourcePack,
    DataPack,
}

impl From<SyncedPackType> for ProjectType {
    fn from(project_type: SyncedPackType) -> Self {
        match project_type {
            SyncedPackType::ResourcePack => Self::ResourcePack,
            SyncedPackType::DataPack => Self::DataPack,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SyncedPackQuery {
    pub project_type: SyncedPackType,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SyncedPacks(
    #[cfg_attr(test, ts(type = "Array<unknown>"))] pub Vec<ContentItem>,
);

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct NewSyncedPack {
    pub path: PathBuf,
    pub project_type: SyncedPackType,
    pub game_versions: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SyncedPackUpdate {
    pub enabled: bool,
}

/// One of an instance's packs, by its path in the instance.
#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct InstancePack {
    pub project_path: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct InstancePackParams {
    pub id: String,
    pub pack_id: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct DesyncQuery {
    pub mode: DesyncServerMode,
}
