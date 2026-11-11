//! The engine's content schema, compiled from the engine's own files so both sides share one
//! definition. The module paths copy the engine's, so the borrowed files resolve unchanged.

#[path = "../../../engine/src/state/instance_types.rs"]
pub mod instance_types;
pub use self::instance_types::*;

pub mod instances;
pub use self::instances::*;

// The content store clone (`upstream/store`), at the path its files name it by, and the glue
// items they find on `crate::state`.
#[path = "../upstream/store/mod.rs"]
#[allow(dead_code, unused_imports)]
pub(crate) mod content_store;
pub(crate) use crate::upstream::store::{
    DirectoryInfo, Instance, Version, file_modified_at_ns,
    instance_has_running_process, runtime_cache,
};
