//! What a mod or plugin jar says about itself: its name, version, sides, and logo. The content
//! list shows these for files Modrinth does not know, such as CurseForge jars in a pack. The
//! reader is bedrock's clone of app-lib's `embedded_content_metadata` with Copal's additions.

pub use bedrock::upstream::jar::{Cache, Manifest, Platform, read_icon};

use crate::installs::ModLoader;

/// The metadata a server with `loader` reads first. Vanilla reads none first.
pub fn platform(loader: Option<ModLoader>) -> Option<Platform> {
    Some(match loader? {
        ModLoader::Fabric => Platform::Fabric,
        ModLoader::Quilt => Platform::Quilt,
        ModLoader::NeoForge => Platform::NeoForge,
        ModLoader::Forge => Platform::Forge,
        ModLoader::Paper => Platform::Paper,
        ModLoader::Vanilla => return None,
    })
}
