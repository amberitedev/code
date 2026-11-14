use std::sync::Arc;

use contracts::tickets::StoreHost;
use serde::{Deserialize, Serialize};

use crate::crypto;
use contracts::backend::sharing::{ExternalFile, InstanceVersion};

/// What a reader needs to fetch one version's files: where they are, and the ticket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolAccess {
    pub share_id: String,
    pub version: i32,
    /// The version's hash.
    pub root: [u8; 32],
    pub holders: Vec<StoreHost>,
    /// The fetch ticket for the version.
    pub ticket: String,
}

/// One manifest entry that lives in the pool, ready to fetch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolFile {
    /// The SHA-256 of the encrypted object.
    pub object: [u8; 32],
    pub object_size: u64,
    /// The key the object decrypts with: the entry's own, or for a content file the one its
    /// plain bytes' hash derives.
    pub key: [u8; 32],
    /// The SHA-256 of the plain bytes.
    pub sha256: [u8; 32],
    /// The access of the version this entry came from.
    pub access: Arc<PoolAccess>,
}

/// A manifest entry a reader cannot fetch.
#[derive(Debug, Clone, thiserror::Error)]
#[error("The share's file {0} has an invalid pool entry.")]
pub struct InvalidPoolEntry(pub String);

/// Where readers fetch `version`'s files, as the share `share_id`'s. `None` while the backend
/// names no holders: the version is not ready, or has no files.
pub fn pool_access(
    version: &InstanceVersion,
    share_id: &str,
) -> Option<Arc<PoolAccess>> {
    let download = version.download.as_ref()?;
    Some(Arc::new(PoolAccess {
        share_id: share_id.to_owned(),
        version: version.version,
        root: version.manifest.version_hash.0,
        holders: download.holders.clone(),
        ticket: download.ticket.clone(),
    }))
}

impl PoolFile {
    /// `file` as fetched through `access`. Refused when it names no key although it is not a
    /// content file, or its object size does not fit its size.
    pub fn new(
        file: &ExternalFile,
        access: Arc<PoolAccess>,
    ) -> Result<Self, InvalidPoolEntry> {
        let key = match file.key {
            Some(key) => key.0,
            None if crypto::is_content_type(&file.file_type) => {
                crypto::content_key(&file.sha256.0)
            }
            None => return Err(InvalidPoolEntry(file.file_name.clone())),
        };
        if crypto::object_size(file.file_size) != file.object_size {
            return Err(InvalidPoolEntry(file.file_name.clone()));
        }
        Ok(Self {
            object: file.object.0,
            object_size: file.object_size,
            key,
            sha256: file.sha256.0,
            access,
        })
    }
}
