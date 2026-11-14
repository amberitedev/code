//! The store's routes, which a Core forwards unread from its own address under `/store/`. Each
//! takes a ticket for one version in place of a bearer token: a store ticket to upload it, a fetch
//! ticket to read it, a pull ticket to copy it from another Core. Only a refused ticket counts
//! against the caller's address, which the Core forwards in `X-Forwarded-For`.

use serde::{Deserialize, Serialize};

use crate::{Auth, Empty, Endpoint, Hash, Json, Method, Stream};

/// A version as a store holds it: its encrypted objects, sorted by hash without repeats. The
/// version's hash is the SHA-256 of these hashes, concatenated.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VersionList {
    pub objects: Vec<ListedObject>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ListedObject {
    pub hash: Hash,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VersionHash {
    pub root: Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VersionObject {
    /// The version's hash.
    pub root: Hash,
    /// The object's hash.
    pub object: Hash,
}

/// Body of [`MissingObjects`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ObjectHashes {
    pub objects: Vec<Hash>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UploadStatus {
    pub missing: Vec<Hash>,
    /// The version is stored whole, so nothing is left to send.
    pub complete: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UploadedVersion {
    pub complete: bool,
    /// What the version still lacks, to send in another upload.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<Hash>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PulledVersion {
    pub complete: bool,
}

/// The version's list, with a fetch ticket.
pub struct ListVersion;

/// A tar of `version.json`, the [`VersionList`], then objects named by their hash, with a store
/// ticket. What arrives is kept even when the upload breaks off, so a retry sends only the rest.
pub struct UploadVersion;

/// Which of these objects the store lacks for the version, with its store ticket.
pub struct MissingObjects;

/// One object's bytes, with a fetch ticket, counted against the Core's daily egress as they go.
/// `Range: bytes=N-` resumes from byte N with 206, and N past the end is 416.
pub struct GetObject;

/// Copy the version from the Core a pull ticket names.
pub struct PullVersion;

impl Endpoint for ListVersion {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/store/versions/{root}";
    const AUTH: Auth = Auth::Ticket;
    type Params = VersionHash;
    type Query = ();
    type Body = Empty;
    type Response = Json<VersionList>;
}

impl Endpoint for UploadVersion {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/store/versions/{root}";
    const AUTH: Auth = Auth::Ticket;
    type Params = VersionHash;
    type Query = ();
    type Body = Stream;
    type Response = Json<UploadedVersion>;
}

impl Endpoint for MissingObjects {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/store/versions/{root}/have";
    const AUTH: Auth = Auth::Ticket;
    type Params = VersionHash;
    type Query = ();
    type Body = Json<ObjectHashes>;
    type Response = Json<UploadStatus>;
}

impl Endpoint for GetObject {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/store/versions/{root}/objects/{object}";
    const AUTH: Auth = Auth::Ticket;
    type Params = VersionObject;
    type Query = ();
    type Body = Empty;
    type Response = Stream;
}

impl Endpoint for PullVersion {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/store/versions/{root}/pull";
    const AUTH: Auth = Auth::Ticket;
    type Params = VersionHash;
    type Query = ();
    type Body = Empty;
    type Response = Json<PulledVersion>;
}

#[cfg(test)]
pub(crate) fn ts() -> String {
    use ts_rs::TS;

    use crate::ts::endpoint;

    let cfg = crate::ts::config();
    let declarations = [
        VersionList::decl(&cfg),
        ListedObject::decl(&cfg),
        VersionHash::decl(&cfg),
        VersionObject::decl(&cfg),
        ObjectHashes::decl(&cfg),
        UploadStatus::decl(&cfg),
        UploadedVersion::decl(&cfg),
        PulledVersion::decl(&cfg),
    ]
    .map(|declaration| format!("\nexport {declaration}\n"))
    .concat();
    let endpoints = [
        ("ListVersion", endpoint::<ListVersion>()),
        ("UploadVersion", endpoint::<UploadVersion>()),
        ("MissingObjects", endpoint::<MissingObjects>()),
        ("GetObject", endpoint::<GetObject>()),
        ("PullVersion", endpoint::<PullVersion>()),
    ]
    .map(|(name, endpoint)| format!("\t{name}: {endpoint},\n"))
    .concat();
    format!(
        "{declarations}\n/** The store's routes, under a Core's `/store/`. */\nexport const store = {{\n{endpoints}}}\n"
    )
}
