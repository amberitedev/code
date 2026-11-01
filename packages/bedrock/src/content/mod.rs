//! Content both sides install: the `.mrpack` index, which side a file runs on, the update
//! channel, and Modrinth's API. Version picking and dependency walks are
//! `modrinth_content_management`'s; this module feeds it.

pub mod adapters;
pub mod domain;
pub mod model;

pub use adapters::modrinth::{Modrinth, ModrinthError};
pub use adapters::mrpack::{IndexError, read_index};
pub use domain::channel::{least_stable, version_type_fallbacks};
pub use domain::mrpack::{PackPlatform, loader_dependency, platform};
pub use domain::sides::mrpack_env;
pub use model::modrinth::{
    Project, SidedVersion, Version, VersionEnvironment, VersionFile,
    VersionHashes, ids_from_cdn,
};
pub use model::mrpack::{Env, Hashes, Index, IndexFile, SideType};
