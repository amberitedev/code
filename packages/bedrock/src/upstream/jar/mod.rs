//! What a mod, plugin or pack archive says about itself. A clone of the parsing half of app-lib's
//! `state/instances/commands/embedded_content_metadata.rs` (see `ports.toml`), with Copal's
//! additions beside it in `server.rs`: Paper and Bukkit `plugin.yml`, the sides Fabric and Quilt
//! name, and warnings for malformed metadata or several mods in one jar.
//!
//! This file is the glue: what the clone needs from app-lib that bedrock does not borrow.

use serde::{Deserialize, Serialize};

#[allow(dead_code)]
mod embedded_content_metadata;

pub use embedded_content_metadata::server::{
    Cache, Environment, Inspection, Manifest, Platform, Warnings, inspect,
    read_icon,
};

/// What a jar's own metadata names. A copy of app-lib's `EmbeddedContentMetadata`
/// (`state/instances/content.rs`), which the App's original reader returns.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct EmbeddedContentMetadata {
    pub name: Option<String>,
    pub version: Option<String>,
    pub icon_path: Option<String>,
}

impl EmbeddedContentMetadata {
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.version.is_none()
            && self.icon_path.is_none()
    }
}
