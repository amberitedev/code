//! What Core asks Modrinth's API. The client and its types are bedrock's, shared with the App;
//! what stays here is Copal's own: the update channel as servers store it and which loaders a
//! Copal loader takes content of.

pub use bedrock::content::{
    Modrinth, ModrinthError, Project, Version, ids_from_cdn,
};
use modrinth_content_management::VersionType;
use serde::{Deserialize, Serialize};

/// How stable a Modrinth version is, ordered from most to least stable. As a server's
/// update channel, the least stable type it takes versions of. Mirrors
/// Archon.Servers.v1.ReleaseChannel.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Default,
    Serialize,
    Deserialize,
    sqlx::Type,
)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum ReleaseChannel {
    #[default]
    Release,
    Beta,
    Alpha,
}

impl ReleaseChannel {
    pub fn name(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Beta => "beta",
            Self::Alpha => "alpha",
        }
    }

    /// The version types a server on this channel takes.
    pub fn version_types(self) -> &'static [VersionType] {
        use VersionType::*;
        match self {
            Self::Release => &[Release],
            Self::Beta => &[Release, Beta],
            Self::Alpha => &[Release, Beta, Alpha],
        }
    }
}

impl From<ReleaseChannel> for VersionType {
    fn from(channel: ReleaseChannel) -> Self {
        match channel {
            ReleaseChannel::Release => Self::Release,
            ReleaseChannel::Beta => Self::Beta,
            ReleaseChannel::Alpha => Self::Alpha,
        }
    }
}

/// The Modrinth names of the loaders whose content runs on a server with `loader`.
pub fn loaders_for(loader: crate::installs::ModLoader) -> Vec<&'static str> {
    use crate::installs::ModLoader;
    match loader {
        ModLoader::Fabric => vec!["fabric"],
        // Quilt runs Fabric mods.
        ModLoader::Quilt => vec!["quilt", "fabric"],
        ModLoader::Forge => vec!["forge"],
        ModLoader::NeoForge => vec!["neoforge"],
        ModLoader::Paper => vec!["paper", "spigot", "bukkit"],
        ModLoader::Vanilla => vec![],
    }
}
