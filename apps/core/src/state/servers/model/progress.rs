//! What an install is doing and did, as the websocket's progress events carry it
//! (`crate::event`).

use serde::Serialize;

/// The Minecraft version a pack install moved a server from and to. Amberite-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ts_rs::TS)]
pub struct GameVersionChange {
    pub from: String,
    pub to: String,
}

/// A loader or platform. Mirrors the `platform` union of Archon.Websocket.v0.InstallProgressPlatformKey
/// and Archon.Sync.v1.WorldContentPlatform, which take the same values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentPlatform {
    Forge,
    Neoforge,
    Fabric,
    Quilt,
    Paper,
    Vanilla,
}

/// What an install progress item is installing. Mirrors Archon.Websocket.v0.InstallProgressKey.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InstallProgressKey {
    /// Mirrors Archon.Websocket.v0.InstallProgressModrinthModpackKey.
    ModrinthModpack {
        project_id: String,
        version_id: String,
    },
    /// Mirrors Archon.Websocket.v0.InstallProgressLocalModpackKey.
    LocalModpack { filename: String },
    /// Mirrors Archon.Websocket.v0.InstallProgressPlatformKey.
    Platform {
        platform: ContentPlatform,
        platform_version: String,
        game_version: String,
    },
}
