mod adapters;
mod commands;
mod domain;
mod model;
mod store;

use crate::util::content_hash::hash_file_with_progress;
pub use crate::util::content_hash::{FileHashes, hash_file};
pub use adapters::filesystem::sync_directory;
pub(crate) use adapters::filesystem::{
    remove_instance_file, validate_parent_directories, writable_copy,
};
pub use adapters::sqlite::retained_owners;
pub(crate) use adapters::sqlite::{
    find_file, instance_storage, remove_file_storage, set_file_storage,
    set_setting, setting,
};
pub(crate) use commands::PendingFileChange;
pub use domain::{
    content_file_path, file_path_on_disk, validate_instance_path,
    validate_relative,
};
pub(crate) use domain::{
    is_managed_content_path, normalize, object_relative_path, relative_link,
    validate_digest,
};
pub use model::StoreUsage;
pub(crate) use model::{FileChangeRequest, InstanceFileStatus};
pub use model::{FileStorageKind, InstanceFileStorage, StoredFileStatus};
pub use store::ContentStore;
pub(crate) use store::StoredFileRecord;
pub use store::{
    FileContent, GetFileResult, ReadableContent, StoredFileHandle,
};

#[cfg(windows)]
pub(crate) use adapters::filesystem::link_unavailable;

pub use adapters::sqlite::StoredFileMetadata;
