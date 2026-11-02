//! The share version: one type for what the backend gives and takes, read the same way by the
//! App and Core.

mod model;
mod removed_file;

pub use model::{
    ManifestFile, PoolGrant, ShareVersion, StoreGrant, VersionContent,
};
pub use removed_file::{InvalidRemovedFile, RemovedFile};
