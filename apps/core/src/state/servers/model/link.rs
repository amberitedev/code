//! Where the pack a server follows came from. Mirrors app-lib's `InstanceLink`
//! (`state/instances/model/link.rs`) with the variants a server has, under the same names and
//! `link_kind` values. Not borrowed through bedrock: that file declares
//! `SharedInstanceAttachmentInput` crate-private, and app-lib builds it outside the file, so app-lib
//! could not take the file back from bedrock.

use crate::state::servers::ContentSourceKind;

/// The link of a server that follows a pack. A server that follows none has no link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerLink {
    ModrinthModpack {
        project_id: String,
        version_id: String,
    },
    /// A pack file the App uploaded.
    ImportedModpack,
    /// A version of an App instance's share.
    SharedInstance(SharedInstanceLink),
}

/// The App instance a server's pack came from, and the version of its share the pack is. app-lib
/// keeps an instance's share and applied version in its shared-instance attachment; a server
/// follows at most one share, so they are on its link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedInstanceLink {
    pub instance_id: String,
    pub instance_name: String,
    pub shared_instance_id: String,
    pub version: i64,
}

impl ServerLink {
    /// The `link_kind` app-lib stores for it.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::ModrinthModpack { .. } => "modrinth_modpack",
            Self::ImportedModpack => "imported_modpack",
            Self::SharedInstance(_) => "shared_instance",
        }
    }

    /// The source kind of the content set and the files the pack places.
    pub fn source_kind(&self) -> ContentSourceKind {
        match self {
            Self::ModrinthModpack { .. } => ContentSourceKind::ModrinthModpack,
            Self::ImportedModpack => ContentSourceKind::ImportedModpack,
            Self::SharedInstance(_) => ContentSourceKind::SharedInstance,
        }
    }

    /// The Modrinth project and version of a Modrinth pack.
    pub fn modrinth(&self) -> Option<(&str, &str)> {
        match self {
            Self::ModrinthModpack {
                project_id,
                version_id,
            } => Some((project_id, version_id)),
            _ => None,
        }
    }

    pub fn shared_instance(&self) -> Option<&SharedInstanceLink> {
        match self {
            Self::SharedInstance(link) => Some(link),
            _ => None,
        }
    }
}
