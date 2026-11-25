//! The store's routes, which a Core forwards unread from its own address under `/store/`. Each
//! takes a ticket for one version in place of a bearer token: a store ticket to upload it, a fetch
//! ticket to read it, a pull ticket to copy it from another Core. Only a refused ticket counts
//! against the caller's address, which the Core forwards in `X-Forwarded-For`.

use serde::{Deserialize, Serialize};

use crate::{Empty, Hash, Json, Stream};

/// A version as a store holds it: its encrypted objects, sorted by hash without repeats. The
/// version's hash is the SHA-256 of these hashes, concatenated.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct VersionList {
    pub objects: Vec<ListedObject>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ListedObject {
    pub hash: Hash,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct VersionHash {
    pub root: Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct VersionObject {
    /// The version's hash.
    pub root: Hash,
    /// The object's hash.
    pub object: Hash,
}

/// Body of [`MissingObjects`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ObjectHashes {
    pub objects: Vec<Hash>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct UploadStatus {
    pub missing: Vec<Hash>,
    /// The version is stored whole, so nothing is left to send.
    pub complete: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct UploadedVersion {
    pub complete: bool,
    /// What the version still lacks, to send in another upload.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<Hash>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct PulledVersion {
    pub complete: bool,
}

crate::endpoints! {
    /// The store's routes, under a Core's `/store/`.
    group store;

    /// The version's list, with a fetch ticket.
    ListVersion: Get "/store/versions/{root}", Ticket, VersionHash, (), Empty => Json<VersionList>;
    /// A tar of `version.json`, the [`VersionList`], then objects named by their hash, with a store
    /// ticket. What arrives is kept even when the upload breaks off, so a retry sends only the rest.
    UploadVersion: Post "/store/versions/{root}", Ticket, VersionHash, (), Stream => Json<UploadedVersion>;
    /// Which of these objects the store lacks for the version, with its store ticket.
    MissingObjects: Post "/store/versions/{root}/have", Ticket, VersionHash, (), Json<ObjectHashes> => Json<UploadStatus>;
    /// One object's bytes, with a fetch ticket, counted against the Core's daily egress as they go.
    /// `Range: bytes=N-` resumes from byte N with 206, and N past the end is 416.
    GetObject: Get "/store/versions/{root}/objects/{object}", Ticket, VersionObject, (), Empty => Stream;
    /// Copy the version from the Core a pull ticket names.
    PullVersion: Post "/store/versions/{root}/pull", Ticket, VersionHash, (), Empty => Json<PulledVersion>;
}
