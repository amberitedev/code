//! What a mod, plugin or pack archive says about itself. A clone of the parsing half of app-lib's
//! `state/instances/commands/embedded_content_metadata.rs` (see `ports.toml`). Copal's additions
//! live in the engine's `server_jar`; only [`Environment`] stays here, for [`crate::content`].
//!
//! This file is the glue: what the clone needs from app-lib that bedrock does not borrow.

use serde::{Deserialize, Serialize};

#[allow(dead_code)]
mod embedded_content_metadata;

/// The sides a jar's metadata names. A copy of the engine's `server_jar::Environment`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    ClientAndServer,
    ClientOnly,
    DedicatedServerOnly,
}

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
