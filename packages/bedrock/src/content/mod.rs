//! Content both sides install: the `.mrpack` index, which side a file runs on, the update
//! channel, and Modrinth's API. Version picking and dependency walks are
//! `modrinth_content_management`'s; this module feeds it.

pub mod adapters;
pub mod domain;
pub mod model;

pub use crate::upstream::jar::Environment as JarEnvironment;
pub use adapters::modrinth::{Modrinth, ModrinthError};
pub use adapters::mrpack::{IndexError, read_index};
pub use domain::channel::{
    least_stable, pack_update, parse_version_type, version_type_fallbacks,
    version_type_name,
};
pub use domain::mrpack::{PackPlatform, loader_dependency, platform};
pub use domain::sides::{mrpack_env, server_takes};
pub use model::modrinth::{
    Project, Version, VersionEnvironment, VersionFile, VersionHashes,
    ids_from_cdn,
};
pub use model::mrpack::{Env, Hashes, Index, IndexFile, SideType};
