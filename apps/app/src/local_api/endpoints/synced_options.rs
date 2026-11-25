//! Which instances share options, the shared command history and the shared server list.

use std::collections::HashMap;

use contracts::{Empty, Json};
use engine::prelude::{InstanceSyncedOption, InstanceSyncedOptions};
use serde::{Deserialize, Serialize};

use super::IdParams;
use crate::synced_options::{
    GlobalSyncedOptions, SyncedOptionJoinPreview, SyncedOptionJoinResolution,
    SyncedOptionsOverview, SyncedServer,
};

contracts::endpoints! {
    group syncedOptions;

    /// Turns one option's sharing on or off for an instance.
    SetInstanceSyncedOption: Put "/app/instances/{id}/synced-options", None, IdParams, (), Json<InstanceSyncedOptionUpdate> => Empty;
    /// What turning the option on for the instance would do with its own files.
    PreviewSyncedOptionJoin: Get "/app/instances/{id}/synced-options/join-preview", None, IdParams, SyncedOptionQuery, Empty => Json<SyncedOptionJoinPreview>;
    GetInstanceSyncedOptions: Get "/app/instances/{id}/synced-options", None, IdParams, (), Empty => Json<SyncedOptionsOverview>;
    /// Which options each instance shares, by instance id.
    ListInstancesSyncedOptions: Get "/app/synced-options/instances", None, (), (), Empty => Json<HashMap<String, InstanceSyncedOptions>>;
    GetGlobalSyncedOptions: Get "/app/synced-options", None, (), (), Empty => Json<GlobalSyncedOptions>;
    /// Which options have shared files yet.
    GetInitializedSyncedOptions: Get "/app/synced-options/initialized", None, (), (), Empty => Json<GlobalSyncedOptions>;
    /// `base_instance_id` seeds the shared files when the option turns on.
    SetGlobalSyncedOption: Put "/app/synced-options", None, (), (), Json<GlobalSyncedOptionUpdate> => Json<GlobalSyncedOptions>;
    /// Writes the shared files into one instance, or into every instance.
    RebuildSyncedOptions: Post "/app/synced-options/rebuild", None, (), (), Json<SyncedOptionsRebuild> => Empty;
    GetCommandHistory: Get "/app/synced-options/command-history", None, (), (), Empty => Json<String>;
    /// Returns the history as stored.
    SetCommandHistory: Put "/app/synced-options/command-history", None, (), (), Json<CommandHistory> => Json<String>;
    ListSyncedServers: Get "/app/synced-servers", None, (), (), Empty => Json<Vec<SyncedServer>>;
    UpdateSyncedServer: Put "/app/synced-servers", None, (), (), Json<SyncedServer> => Empty;
    RemoveSyncedServer: Delete "/app/synced-servers/{id}", None, IdParams, (), Empty => Empty;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceSyncedOptionUpdate {
    pub option: InstanceSyncedOption,
    pub enabled: bool,
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub resolution: Option<SyncedOptionJoinResolution>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SyncedOptionQuery {
    pub option: InstanceSyncedOption,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct GlobalSyncedOptionUpdate {
    pub option: InstanceSyncedOption,
    pub enabled: bool,
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub base_instance_id: Option<String>,
}

/// Every instance when `instance_id` is absent.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SyncedOptionsRebuild {
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub instance_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct CommandHistory {
    pub contents: String,
}
