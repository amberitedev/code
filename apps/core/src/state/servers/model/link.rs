//! Where the pack a server follows came from: app-lib's `InstanceLink`
//! (`state/instances/model/link.rs`, borrowed through bedrock). Copal writes three variants:
//! `ModrinthModpack`, `ImportedModpack` (a pack file the App uploaded) and `SharedInstance` (a
//! version of an App instance's share). A server that follows no pack has no link.

pub use bedrock::state::InstanceLink;

use crate::state::servers::ContentSourceKind;

/// Copal: the App instance a server's pack came from, and the version of its share the pack is.
/// app-lib keeps an instance's share and applied version in its shared-instance attachment; a
/// server follows at most one share, so they sit beside its `SharedInstance` link, in the marked
/// extra columns of `server_links`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedInstanceLink {
    pub instance_id: String,
    pub instance_name: String,
    pub shared_instance_id: String,
    pub version: i64,
}

/// The source kind of the content set and the files a pack with this link places.
pub fn link_source_kind(link: &InstanceLink) -> ContentSourceKind {
    match link {
        InstanceLink::Unmanaged => ContentSourceKind::Local,
        InstanceLink::ModrinthModpack { .. } => {
            ContentSourceKind::ModrinthModpack
        }
        InstanceLink::ServerProject { .. }
        | InstanceLink::ServerProjectModpack { .. } => {
            ContentSourceKind::ServerProject
        }
        InstanceLink::ModrinthHosting { .. } => {
            ContentSourceKind::ModrinthHosting
        }
        InstanceLink::ImportedModpack { .. } => {
            ContentSourceKind::ImportedModpack
        }
        InstanceLink::SharedInstance { .. } => {
            ContentSourceKind::SharedInstance
        }
    }
}

/// The Modrinth project and version of a Modrinth pack.
pub fn modrinth_pack(link: &InstanceLink) -> Option<(&str, &str)> {
    match link {
        InstanceLink::ModrinthModpack {
            project_id,
            version_id,
        } => Some((project_id, version_id)),
        _ => None,
    }
}
