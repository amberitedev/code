//! A server's content as Copal records it: app-lib's content model
//! (`state/instances/model/`, borrowed through bedrock) with `server` for `instance`. The files
//! on disk are the truth; a record only says who put a file there, what it was then, and which
//! sides it is on for.
//!
//! The borrowed types keep app-lib's field names, so a `ContentSet::instance_id` here holds a
//! server id.

use bedrock::state::ContentRequirement;
pub use bedrock::state::{ContentSet, ContentSetStatus, ContentSourceKind};
use modrinth_content_management::ContentType;

/// What a pack update wants for a file it found edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pending {
    /// The new pack has another version of the file.
    Changed,
    /// The new pack no longer has the file.
    Removed,
}

impl Pending {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Changed => "changed",
            Self::Removed => "removed",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "changed" => Some(Self::Changed),
            "removed" => Some(Self::Removed),
            _ => None,
        }
    }
}

/// A file Core tracks on a server, by its enabled path: a `server_files` row and the
/// `server_content_entries` row that says where it came from. This is what installs and the
/// pack planner read and write; the sides are [`EntrySides`], kept apart so neither overwrites
/// the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedFile {
    pub relative_path: String,
    /// `Local` for a file someone on the server added, or the source kind of the pack's set.
    pub source_kind: ContentSourceKind,
    /// As Core placed it.
    pub sha1: String,
    /// As Core placed it; 0 for a file it found there rather than placed.
    pub size: u64,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub pending: Option<Pending>,
}

impl TrackedFile {
    /// Whether the pack the server follows put it there.
    pub fn by_pack(&self) -> bool {
        self.source_kind != ContentSourceKind::Local
    }
}

/// Which sides a tracked file is on for, from its entry. A file Core does not track is on for
/// both, the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntrySides {
    /// Off for both sides: the file is `.disabled` on disk.
    pub enabled: bool,
    /// `Unsupported`: the file is `.disabled` on disk, and players still get it.
    pub server_requirement: ContentRequirement,
    /// `Unsupported`: the server runs it, and its share leaves it out.
    pub client_requirement: ContentRequirement,
    /// Someone set the sides by hand.
    pub unlocked: bool,
}

impl Default for EntrySides {
    fn default() -> Self {
        Self {
            enabled: true,
            server_requirement: ContentRequirement::Required,
            client_requirement: ContentRequirement::Required,
            unlocked: false,
        }
    }
}

/// A file of the pack the server leaves out because it is client only. It is never on the
/// server's disk; the content list shows it as not installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedFile {
    pub relative_path: String,
    pub sha1: String,
    pub size: u64,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
}

/// The pack a server follows: its content set, and where it came from.
#[derive(Debug, Clone)]
pub struct Pack {
    pub set: ContentSet,
    /// The Modrinth project and version, or `None` for a pack file the App uploaded.
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    /// As the pack's index names it.
    pub version_number: String,
    pub summary: Option<String>,
    /// SHA-1 of the `.mrpack`, kept in `meta/packs/`.
    pub sha1: String,
    pub instance_id: Option<String>,
    pub instance_name: Option<String>,
    /// The share and version the pack is, for a pack from an App instance (`commands::write_shared_pack`).
    pub shared_instance_id: Option<String>,
    pub shared_version: Option<i64>,
    pub error: Option<String>,
}

/// A share version pushed while its server ran, waiting for it to stop.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PendingPack {
    pub sha1: String,
    pub instance_id: String,
    pub instance_name: String,
    pub shared_instance_id: String,
    pub shared_version: i64,
    pub start_after: bool,
    pub force: bool,
}

/// The project type a file at `relative_path` is, from its folder, as app-lib's
/// `ProjectType::get_from_parent_folder` reads one, with plugins. `None` for anything not
/// directly inside a content folder, such as a config.
pub fn project_type_of(relative_path: &str) -> Option<ContentType> {
    let (folder, name) = relative_path.rsplit_once('/')?;
    if name.is_empty() {
        return None;
    }
    Some(match folder.rsplit('/').next()? {
        "mods" if !folder.contains('/') => ContentType::Mod,
        "plugins" if !folder.contains('/') => ContentType::Plugin,
        "resourcepacks" if !folder.contains('/') => ContentType::ResourcePack,
        "shaderpacks" if !folder.contains('/') => ContentType::Shader,
        // Inside the world, `<level>/datapacks/`.
        "datapacks" if folder.matches('/').count() == 1 => {
            ContentType::DataPack
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_s_type_is_its_content_folder_s() {
        assert_eq!(project_type_of("mods/a.jar"), Some(ContentType::Mod));
        assert_eq!(
            project_type_of("world/datapacks/a.zip"),
            Some(ContentType::DataPack)
        );
        assert_eq!(project_type_of("config/a.toml"), None);
        assert_eq!(project_type_of("mods/sub/a.jar"), None);
        assert_eq!(project_type_of("a/b/datapacks/a.zip"), None);
    }
}
