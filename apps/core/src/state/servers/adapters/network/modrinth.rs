//! What Core asks Modrinth's API. The client and its types are bedrock's, shared with the App;
//! what stays here is Copal's own: which loaders a Copal loader takes content of. A server's
//! update channel is a [`VersionType`]: the least stable type it takes versions of.

pub use bedrock::content::{
    Modrinth, ModrinthError, Project, Version, ids_from_cdn,
};
use modrinth_content_management::VersionType;

/// `servers.update_channel` as stored: a version type's name. Read into a [`VersionType`] with
/// `#[sqlx(try_from = "ChannelText")]`.
#[derive(sqlx::Type)]
#[sqlx(transparent)]
pub struct ChannelText(String);

impl TryFrom<ChannelText> for VersionType {
    type Error = &'static str;

    fn try_from(text: ChannelText) -> Result<Self, Self::Error> {
        bedrock::content::parse_version_type(&text.0)
            .ok_or("an update channel is release, beta, or alpha")
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
