//! A Core's servers: listing and creating them, power, startup and `server.properties`, in
//! Modrinth Hosting's shapes (`Archon.*` in `packages/api-client/src/modules`).

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use modrinth_content_management::icons::IconConfig;
use serde::{Deserialize, Serialize};

use super::ReachabilityReport;
use crate::{ModLoader, ReleaseChannel};

/// A server on a Core, in Modrinth Hosting's server shape (`Archon.Servers.v0.Server`) where a
/// Core has the field, with what a Core adds after it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct Server {
    pub server_id: String,
    pub name: String,
    /// Null until the Core is paired.
    pub owner_id: Option<String>,
    pub net: Net,
    pub status: Status,
    /// Null until something is installed.
    pub loader: Option<Loader>,
    pub loader_version: Option<String>,
    pub mc_version: Option<String>,
    /// The Modrinth pack it runs, if any.
    pub upstream: Option<Upstream>,
    /// `intro` is set while nothing is installed: the server is not set up yet.
    pub flows: Flows,
    pub icon: Option<ServerIcon>,
    pub power_state: ServerPower,
    /// The App instance it follows.
    pub linked_instance: Option<LinkedInstance>,
    /// The `.mrpack` it was set up from by upload, whose files it keeps apart as a Modrinth pack's.
    /// A share cannot carry one, so the server is unlinked from it before it is shared.
    pub uploaded_modpack: Option<UploadedModpack>,
    /// The share it reads: its instance's when it follows one, else its own once it is shared.
    pub shared_instance_id: Option<String>,
    /// Whether players can join from the internet. Null while it does not run.
    pub reachability: Option<ReachabilityReport>,
    /// Who is online. Null while it does not run.
    pub players: Option<Vec<String>>,
    pub memory_mb: u32,
    pub created_at: DateTime<Utc>,
    pub update_channel: ReleaseChannel,
    /// The owner accepted the Minecraft EULA for every server on this Core. None starts before.
    pub eula_accepted: bool,
}

/// Where players join a server.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct Net {
    /// With `port`, where players join from the internet: the router's address when it forwarded
    /// another port than the game port, otherwise the Core's.
    pub ip: Option<String>,
    pub port: u16,
    /// Where the server is on the internet, while it runs and is forwarded or public.
    pub outside: Option<JoinAddress>,
    /// The port the server listens on.
    pub game_port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct JoinAddress {
    pub ip: String,
    pub port: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Its loader or pack is installing now.
    Installing,
    Available,
}

/// A server's loader, as Modrinth Hosting names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub enum Loader {
    Vanilla,
    Fabric,
    Quilt,
    Forge,
    NeoForge,
    Paper,
}

impl From<ModLoader> for Loader {
    fn from(loader: ModLoader) -> Self {
        match loader {
            ModLoader::Vanilla => Self::Vanilla,
            ModLoader::Fabric => Self::Fabric,
            ModLoader::Quilt => Self::Quilt,
            ModLoader::Forge => Self::Forge,
            ModLoader::NeoForge => Self::NeoForge,
            ModLoader::Paper => Self::Paper,
        }
    }
}

impl From<Loader> for ModLoader {
    fn from(loader: Loader) -> Self {
        match loader {
            Loader::Vanilla => Self::Vanilla,
            Loader::Fabric => Self::Fabric,
            Loader::Quilt => Self::Quilt,
            Loader::Forge => Self::Forge,
            Loader::NeoForge => Self::NeoForge,
            Loader::Paper => Self::Paper,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct Upstream {
    pub kind: UpstreamKind,
    pub project_id: String,
    pub version_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum UpstreamKind {
    Modpack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct Flows {
    pub intro: bool,
}

/// A server's icon, which is its instance's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ServerIcon {
    /// The image's content hash: a new icon has a new one.
    pub hash: String,
    /// Set when the icon editor drew it.
    pub config: Option<IconConfig>,
}

/// Where a server is in its run, or how its last run ended: `stopped` (or never run), `crashed`
/// or `killed`. The socket's [`PowerState`](super::PowerState) names the same states, and sends
/// `killed` as `stopped`, as Modrinth Hosting's has no such state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ServerPower {
    Stopped,
    Starting,
    Running,
    Stopping,
    Crashed,
    Killed,
}

/// A `.mrpack` a server was set up from by upload, as the pack names itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct UploadedModpack {
    pub name: Option<String>,
    pub version_number: Option<String>,
}

/// The App instance a server follows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct LinkedInstance {
    /// The App's id for the instance.
    pub id: String,
    pub name: String,
    pub state: LinkedPackState,
    pub error: Option<String>,
    pub shared_instance_id: Option<String>,
    /// The version of the instance's share the server runs.
    pub version: Option<i64>,
}

/// Where a linked server is with the newest version pushed to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum LinkedPackState {
    UpToDate,
    Downloading,
    /// Applies when the server next stops.
    Pending,
    Applying,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct NewServer {
    /// 1 to 64 characters.
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ServerUpdate {
    /// 1 to 64 characters, trimmed.
    pub name: Option<String>,
    pub update_channel: Option<ReleaseChannel>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub enum PowerAction {
    Start,
    Stop,
    Restart,
    Kill,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct PowerChange {
    pub action: PowerAction,
    /// The long op's id (see [`OpStart`](super::OpStart)).
    pub op: String,
}

/// A loader to install on a stopped server. Packs install through the content routes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct LoaderInstall {
    pub loader: Loader,
    /// Null for the newest.
    pub loader_version: Option<String>,
    pub game_version: String,
    /// The long op's id (see [`OpStart`](super::OpStart)).
    pub op: String,
}

/// Body of `ShareServer`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SharePush {
    /// Files to send with this version. Files sent earlier stay shared while the server has
    /// them, unless they are in `removed_paths`.
    pub file_paths: Vec<String>,
    pub removed_paths: Vec<String>,
    /// Restart a running server once the version is out.
    pub restart: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SharedServer {
    pub shared_instance_id: String,
    pub version: Option<i32>,
    pub restarted: bool,
}

/// Body of `FollowInstance`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceFollow {
    pub instance: FollowedInstance,
    /// The instance's share, whose latest version the server applies.
    pub shared_instance_id: String,
    /// Stop a running server to apply the version now, and start it again after.
    pub restart: bool,
}

/// The App instance a server follows, as the App names it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct FollowedInstance {
    pub id: String,
    pub name: String,
}

/// How a server starts. A Core builds the command itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct RuntimeOptions {
    /// The Java major it runs. Null until installed.
    pub java_version: Option<u32>,
    /// The command line it runs, as people read it. Null until installed.
    pub original_invocation: Option<String>,
}

/// Used from the server's next start.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct PatchRuntimeOptions {
    /// A Java major the Core installs (8, 11, 17, 21 or 25), or null for the one the Minecraft
    /// version needs.
    pub java_version: Option<u32>,
}

/// A server's `server.properties`: the keys Modrinth Hosting names, with dashes as underscores,
/// and every other key as the file names it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct PropertiesFields {
    pub known: KnownPropertiesFields,
    pub custom: BTreeMap<String, String>,
}

/// What to change. Absent keys stay; in `custom`, null removes the key.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct PatchPropertiesFields {
    #[serde(default)]
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub known: Option<KnownPropertiesFields>,
    #[serde(default)]
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub custom: Option<BTreeMap<String, Option<String>>>,
}

macro_rules! known_properties {
    ($($field:ident),* $(,)?) => {
        /// The `server.properties` keys Modrinth Hosting names, each optional.
        #[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
        #[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
        pub struct KnownPropertiesFields {
            $(
                #[serde(default, skip_serializing_if = "Option::is_none")]
                #[cfg_attr(feature = "export-ts", ts(optional = nullable))]
                pub $field: Option<String>,
            )*
        }

        impl KnownPropertiesFields {
            /// Each field set, under the file's key for it.
            pub fn entries(&self) -> Vec<(String, &str)> {
                let mut entries = Vec::new();
                $(
                    if let Some(value) = &self.$field {
                        entries.push((stringify!($field).replace('_', "-"), value.as_str()));
                    }
                )*
                entries
            }

            /// Takes the value of the file's `key` when it is a known key. False when it is not.
            pub fn set(&mut self, key: &str, value: String) -> bool {
                match key.replace('-', "_").as_str() {
                    $(stringify!($field) => self.$field = Some(value),)*
                    _ => return false,
                }
                true
            }
        }
    };
}

known_properties!(
    allow_cheats,
    allow_flight,
    difficulty,
    enforce_whitelist,
    force_gamemode,
    gamemode,
    generate_structures,
    generator_settings,
    hardcore,
    level_seed,
    level_type,
    max_players,
    max_tick_time,
    motd,
    pause_when_empty_seconds,
    player_idle_timeout,
    require_resource_pack,
    resource_pack,
    resource_pack_id,
    resource_pack_sha1,
    simulation_distance,
    spawn_protection,
    sync_chunk_writes,
    view_distance,
    white_list,
);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct BackupRef {
    /// The server.
    pub id: String,
    pub backup_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct OperationRef {
    /// The server.
    pub id: String,
    pub operation_id: i64,
}

#[cfg(test)]
pub(super) fn ts_types() -> Vec<String> {
    use ts_rs::TS;

    let cfg = crate::ts::config();
    vec![
        Server::decl(&cfg),
        Net::decl(&cfg),
        JoinAddress::decl(&cfg),
        Status::decl(&cfg),
        Loader::decl(&cfg),
        Upstream::decl(&cfg),
        UpstreamKind::decl(&cfg),
        Flows::decl(&cfg),
        ServerIcon::decl(&cfg),
        ServerPower::decl(&cfg),
        UploadedModpack::decl(&cfg),
        LinkedInstance::decl(&cfg),
        LinkedPackState::decl(&cfg),
        NewServer::decl(&cfg),
        ServerUpdate::decl(&cfg),
        PowerAction::decl(&cfg),
        PowerChange::decl(&cfg),
        LoaderInstall::decl(&cfg),
        SharePush::decl(&cfg),
        SharedServer::decl(&cfg),
        InstanceFollow::decl(&cfg),
        FollowedInstance::decl(&cfg),
        RuntimeOptions::decl(&cfg),
        PatchRuntimeOptions::decl(&cfg),
        PropertiesFields::decl(&cfg),
        PatchPropertiesFields::decl(&cfg),
        KnownPropertiesFields::decl(&cfg),
        BackupRef::decl(&cfg),
        OperationRef::decl(&cfg),
    ]
}
