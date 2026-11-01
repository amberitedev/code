//! The writer of a share's versions (`docs/internals/pool.md`, Push): hash the files, carry
//! configs and keys forward from the previous version, encrypt, create the version on the
//! backend, store the objects on the Core it picks, and wait until that Core confirms them.
//!
//! The App pushes an instance's share with [`push`], and a Core pushes a server's share with
//! the same call. What goes into a version is the caller's: it picks the files and passes the
//! upstream content fields (Modrinth ids, loader, and so on) as they are.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures::{StreamExt, TryStreamExt};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use tokio::io::AsyncReadExt;

use crate::crypto::{self, parse_hash};
use crate::store::{ListedObject, PoolCore, StoreClient, StoreError};

/// Most files one version can hold.
pub const MAX_FILES: usize = 4096;
/// Most plain bytes one version can hold.
pub const MAX_BYTES: u64 = 1024 * 1024 * 1024;
/// A single file the owner picked to share, named by its path in the instance.
pub const CONFIG_FILE_TYPE: &str = "config";
/// Upstream's zip of config files, which older versions carry.
pub const CONFIG_BUNDLE_FILE_TYPE: &str = "configs";
/// Leading bytes of each file passed to [`is_executable_content`].
pub const SCAN_HEAD_SIZE: usize = 8192;

/// Files hashed or encrypted at once.
const CONCURRENCY: usize = 4;
/// The Core confirms a stored version to the backend on its own, so a push waits up to a minute.
const CONFIRM_INTERVAL: Duration = Duration::from_secs(2);
const CONFIRM_ATTEMPTS: usize = 30;
/// Uploads rounds of whatever the Core still lacks until it holds the whole version.
const UPLOAD_ROUNDS: usize = 3;
const UPLOAD_PROGRESS_INTERVAL: Duration = Duration::from_millis(200);

/// Whether a file's leading bytes are an executable, script or installer, which a share never
/// carries.
pub fn is_executable_content(head: &[u8]) -> bool {
    infer::get(head).is_some_and(|kind| match kind.matcher_type() {
        infer::MatcherType::App => {
            !matches!(kind.extension(), "der" | "pem" | "obj" | "bc")
        }
        _ => matches!(
            kind.extension(),
            "sh" | "msi" | "cab" | "deb" | "rpm" | "crx"
        ),
    })
}

/// The backend's share routes, reached with `token` as the bearer.
pub struct Backend {
    http: reqwest::Client,
    base_url: String,
    token: String,
    read_ticket: Option<String>,
}

impl Backend {
    /// `base_url` is the backend's root, without `/v1`. The caller builds `http`, so TLS stays
    /// set up the way the rest of the process does it.
    pub fn new(
        http: reqwest::Client,
        base_url: impl Into<String>,
        token: impl Into<String>,
    ) -> Self {
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token: token.into(),
            read_ticket: None,
        }
    }

    /// A Core key reading a share its server follows, with the read ticket the asking user's App
    /// got for this Core and share. The backend then reads it as that user, and only while they
    /// can read it.
    pub fn with_read_ticket(mut self, ticket: impl Into<String>) -> Self {
        self.read_ticket = Some(ticket.into());
        self
    }

    fn request(
        &self,
        method: reqwest::Method,
        path: &str,
    ) -> reqwest::RequestBuilder {
        let request = self
            .http
            .request(method, format!("{}/v1{path}", self.base_url))
            .bearer_auth(&self.token);
        match &self.read_ticket {
            Some(ticket) => request.header("X-Amberite-Read-Ticket", ticket),
            None => request,
        }
    }

    /// A version of the share, or the latest one without `version`, read as `T`. `None` when it
    /// does not exist.
    pub(crate) async fn version<T: serde::de::DeserializeOwned>(
        &self,
        share_id: &str,
        version: Option<i32>,
    ) -> Result<Option<T>, PushError> {
        let path = match version {
            Some(version) => {
                format!("/instances/{share_id}/versions/{version}")
            }
            None => format!("/instances/{share_id}/versions"),
        };
        let response = self.request(reqwest::Method::GET, &path).send().await?;
        match unavailable(response.status()) {
            Some(Unavailable::Deleted) => Ok(None),
            Some(reason) => Err(PushError::Unavailable(reason)),
            None => Ok(Some(
                success(response, "get_instance_version")
                    .await?
                    .json()
                    .await?,
            )),
        }
    }

    /// Creates a version. The pool's refusals come back as their own errors, named by the
    /// backend's `code`.
    async fn create_version<C: Serialize>(
        &self,
        share_id: &str,
        body: &NewVersion<'_, C>,
    ) -> Result<Version, PushError> {
        let operation = "create_instance_version";
        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/instances/{share_id}/versions"),
            )
            .json(body)
            .send()
            .await?;
        let status = response.status();
        if let Some(reason) = unavailable(status) {
            return Err(PushError::Unavailable(reason));
        }
        if matches!(
            status,
            StatusCode::CONFLICT
                | StatusCode::PAYLOAD_TOO_LARGE
                | StatusCode::SERVICE_UNAVAILABLE
        ) {
            let body = response.text().await.unwrap_or_default();
            return Err(refusal(status, &body).unwrap_or(PushError::Backend {
                operation,
                status: status.as_u16(),
            }));
        }
        Ok(success(response, operation).await?.json().await?)
    }

    /// The share of the calling Core's server `server_id`, created now unless it has one. Only
    /// a Core key may, for a server that follows no instance.
    pub async fn create_server_share(
        &self,
        name: &str,
        server_id: &str,
    ) -> Result<String, PushError> {
        #[derive(Deserialize)]
        struct Created {
            id: String,
        }
        let response = self
            .request(reqwest::Method::POST, "/instances")
            .json(&serde_json::json!({ "name": name, "server_id": server_id }))
            .send()
            .await?;
        let created: Created =
            success(response, "create_instance").await?.json().await?;
        Ok(created.id)
    }

    pub async fn rename_share(
        &self,
        share_id: &str,
        name: &str,
    ) -> Result<(), PushError> {
        let response = self
            .request(reqwest::Method::PATCH, &format!("/instances/{share_id}"))
            .json(&serde_json::json!({ "name": name }))
            .send()
            .await?;
        success(response, "edit_instance").await.map(drop)
    }

    /// Deletes the share. One already gone counts as deleted.
    pub async fn delete_share(&self, share_id: &str) -> Result<(), PushError> {
        let response = self
            .request(reqwest::Method::DELETE, &format!("/instances/{share_id}"))
            .send()
            .await?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(());
        }
        success(response, "delete_instance").await.map(drop)
    }

    /// The latest version members can install, or `None` while there is none.
    pub async fn latest_ready(
        &self,
        share_id: &str,
    ) -> Result<Option<LatestVersion>, PushError> {
        let response = self
            .request(
                reqwest::Method::GET,
                &format!("/instances/{share_id}/versions"),
            )
            .send()
            .await?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let latest: LatestVersion = success(response, "get_instance_version")
            .await?
            .json()
            .await?;
        Ok(latest.ready.then_some(latest))
    }
}

/// A version's upstream content fields and files, as a writer compares its next push to them.
#[derive(Debug, Deserialize)]
pub struct LatestVersion {
    pub version: i32,
    pub ready: bool,
    #[serde(default)]
    pub modrinth_ids: Vec<String>,
    #[serde(default)]
    pub modpack_id: Option<String>,
    #[serde(default)]
    pub removed_files: Vec<RemovedFile>,
    pub game_version: String,
    pub loader: String,
    #[serde(default)]
    pub loader_version: String,
    #[serde(default)]
    pub external_files: Vec<LatestFile>,
}

/// A file of a version's modpack that members do not get, by its Modrinth version or, for a
/// mod Modrinth does not know, its path. As `SharedInstanceRemovedFile` in `app-lib`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RemovedFile {
    Version { version_id: String },
    Path { parent: String, filename: String },
}

#[derive(Debug, Deserialize)]
pub struct LatestFile {
    pub file_name: String,
    pub file_type: String,
    #[serde(default)]
    pub sha1: Option<String>,
}

fn unavailable(status: StatusCode) -> Option<Unavailable> {
    match status {
        StatusCode::NOT_FOUND => Some(Unavailable::Deleted),
        StatusCode::UNAUTHORIZED => Some(Unavailable::AccessRevoked),
        _ => None,
    }
}

async fn success(
    response: reqwest::Response,
    operation: &'static str,
) -> Result<reqwest::Response, PushError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    tracing::warn!(
        operation,
        status = status.as_u16(),
        "Shared instances API request failed"
    );
    Err(PushError::Backend {
        operation,
        status: status.as_u16(),
    })
}

/// Every refusal over a limit (413) carries a description naming the limit, which is shown as is.
fn refusal(status: StatusCode, body: &str) -> Option<PushError> {
    #[derive(Deserialize)]
    struct Body {
        #[serde(default)]
        code: Option<String>,
        #[serde(default)]
        description: String,
    }
    let body = serde_json::from_str::<Body>(body).ok()?;
    Some(match body.code.as_deref() {
        Some("no_storage") => PushError::NoStorage,
        Some("push_in_flight") => PushError::PushInFlight,
        _ if status == StatusCode::PAYLOAD_TOO_LARGE
            && !body.description.is_empty() =>
        {
            PushError::Refused(body.description)
        }
        _ => return None,
    })
}

/// Why the backend would not give the share.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Unavailable {
    /// 404: the share is gone, or belongs to another backend.
    #[error("The shared instance was deleted.")]
    Deleted,
    /// 401: the token no longer reaches the share.
    #[error("You no longer have access to the shared instance.")]
    AccessRevoked,
}

#[derive(Debug, thiserror::Error)]
pub enum PushError {
    #[error("{0} is an executable, which a share cannot carry")]
    Executable(String),
    #[error("a share can hold at most {MAX_BYTES} bytes of files")]
    TooLarge,
    #[error(
        "a share can hold at most {MAX_FILES} files, and this push has {0}"
    )]
    TooManyFiles(usize),
    #[error("{0} changed while it was being pushed")]
    FileChanged(String),
    /// The Core lost the object of a config carried from an earlier version, which this push
    /// cannot send because it does not have the file.
    #[error("the Core no longer has a config file shared earlier")]
    CarriedFileMissing,
    #[error("no Core offers storage for this share")]
    NoStorage,
    #[error("another push of this share is still uploading")]
    PushInFlight,
    /// A limit the backend names in its own words.
    #[error("{0}")]
    Refused(String),
    #[error(transparent)]
    Unavailable(Unavailable),
    #[error(
        "shared instances API request {operation} failed with status {status}"
    )]
    Backend {
        operation: &'static str,
        status: u16,
    },
    #[error("could not reach the shared instances API: {0}")]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("the Core did not accept every shared file; push again")]
    NotAccepted,
    #[error("the Core did not confirm the shared files in time")]
    Unconfirmed,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// A file to push: its manifest name and type, and where its bytes are now.
pub struct PushFile {
    pub file_name: String,
    pub file_type: String,
    pub path: PathBuf,
}

/// A file as hashed for the push.
pub struct HashedFile {
    pub file_name: String,
    pub file_type: String,
    pub size: u64,
    /// What a member's install compares to skip a file it already has.
    pub sha1: String,
    /// P.
    pub sha256: [u8; 32],
    pub executable: bool,
    path: PathBuf,
}

/// How a push is going, for the caller's progress and logs.
pub enum PushEvent<'a> {
    Hashed(&'a [HashedFile]),
    Encrypted(usize),
    Created(i32),
    /// The objects sent to the Core, and the names of the files they hold.
    Stored {
        objects: usize,
        files: HashSet<String>,
    },
    /// Uploaded and total bytes of the objects the Core asked for.
    Progress(u64, u64),
    Confirmed,
}

/// A pushed file's manifest entry, as the Protocol in `docs/internals/pool.md` names it. The
/// upstream fields come first.
#[derive(Clone, Debug, Serialize)]
struct ManifestFile {
    file_name: String,
    file_type: String,
    file_size: u64,
    sha1: String,
    /// P.
    sha256: String,
    /// C.
    object: String,
    object_size: u64,
    /// Only non-content files carry a key.
    #[serde(skip_serializing_if = "Option::is_none")]
    key: Option<String>,
}

#[derive(Serialize)]
struct NewVersion<'a, C> {
    #[serde(flatten)]
    content: &'a C,
    external_files: &'a [ManifestFile],
    root: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    server_core: Option<&'a str>,
}

/// The parts of a version a push reads.
#[derive(Deserialize)]
struct Version {
    version: i32,
    ready: bool,
    #[serde(default)]
    external_files: Vec<KeptFile>,
    #[serde(default)]
    store: Option<StoreGrant>,
}

/// A manifest entry as the backend gives it back. Read leniently: an entry missing a field is
/// not carried forward rather than failing the push.
#[derive(Deserialize)]
struct KeptFile {
    file_name: String,
    file_type: String,
    #[serde(default)]
    file_size: Option<i64>,
    #[serde(default)]
    sha1: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    object: Option<String>,
    #[serde(default)]
    object_size: Option<u64>,
    #[serde(default)]
    key: Option<String>,
}

/// Where the owner stores the files while the version is unconfirmed.
#[derive(Deserialize)]
struct StoreGrant {
    core: PoolCore,
    ticket: String,
}

/// An encrypted object of the push.
struct Object {
    object: [u8; 32],
    size: u64,
    /// `None` for a file carried from the previous version, which the Core already holds.
    path: Option<PathBuf>,
    /// P of the plaintext that was encrypted, which the push compares to the P it hashed.
    plain: [u8; 32],
}

/// Pushes a new version of the share `share_id` holding `files`, plus the configs of the
/// latest version this push does not send again, and returns its number once members can
/// install it. `content` holds the version's upstream fields and is sent as they serialize.
/// `server_core` is a Core of the owner running a server of this share, which the backend makes
/// its home. Encrypted files wait in a folder of their own under `scratch` until the Core has
/// them.
#[allow(clippy::too_many_arguments)]
pub async fn push<C: Serialize + Sync>(
    backend: &Backend,
    store: &StoreClient,
    share_id: &str,
    content: &C,
    server_core: Option<&str>,
    files: &[PushFile],
    scratch: &Path,
    on_event: &mut (dyn FnMut(PushEvent<'_>) + Send),
) -> Result<i32, PushError> {
    let hashed = hash_files(files).await?;
    on_event(PushEvent::Hashed(&hashed));
    if let Some(file) = hashed.iter().find(|file| file.executable) {
        return Err(PushError::Executable(file.file_name.clone()));
    }
    let total_size = hashed
        .iter()
        .fold(0_u64, |total, file| total.saturating_add(file.size));
    if total_size > MAX_BYTES {
        return Err(PushError::TooLarge);
    }

    let folder = scratch.join(hex::encode(rand::random::<[u8; 16]>()));
    let result = push_hashed(
        backend,
        store,
        share_id,
        content,
        server_core,
        &hashed,
        &folder,
        on_event,
    )
    .await;
    if let Err(error) = tokio::fs::remove_dir_all(&folder).await
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(
            folder = %folder.display(),
            error = %error,
            "Could not remove a push's encrypted files"
        );
    }
    result
}

#[allow(clippy::too_many_arguments)]
async fn push_hashed<C: Serialize + Sync>(
    backend: &Backend,
    store: &StoreClient,
    share_id: &str,
    content: &C,
    server_core: Option<&str>,
    hashed: &[HashedFile],
    folder: &Path,
    on_event: &mut (dyn FnMut(PushEvent<'_>) + Send),
) -> Result<i32, PushError> {
    let previous = previous_versions(backend, share_id).await?;
    let carried = carried_config_files(&previous, hashed);
    let file_count = hashed.len() + carried.len();
    if file_count > MAX_FILES {
        return Err(PushError::TooManyFiles(file_count));
    }
    let keys = file_keys(&previous, hashed);
    tokio::fs::create_dir_all(folder).await?;
    // Collected first so the stream holds plain futures, not a borrowing closure, which keeps
    // the push future Send for Tauri commands.
    let encryptions = hashed
        .iter()
        .zip(&keys)
        .map(|(file, key)| encrypt_file(key, &file.path, folder))
        .collect::<Vec<_>>();
    let mut objects = futures::stream::iter(encryptions)
        .buffered(CONCURRENCY)
        .try_collect::<Vec<_>>()
        .await?;
    // A file written to between hashing and encrypting would reach the Core with a P that its
    // bytes no longer match, and every member's install would fail on it.
    if let Some(file) =
        hashed.iter().zip(&objects).find_map(|(file, object)| {
            (object.plain != file.sha256).then_some(file)
        })
    {
        return Err(PushError::FileChanged(file.file_name.clone()));
    }
    on_event(PushEvent::Encrypted(objects.len()));

    let mut external_files = hashed
        .iter()
        .zip(&keys)
        .zip(&objects)
        .map(|((file, key), object)| ManifestFile {
            file_name: file.file_name.clone(),
            file_type: file.file_type.clone(),
            file_size: file.size,
            sha1: file.sha1.clone(),
            sha256: hex::encode(file.sha256),
            object: hex::encode(object.object),
            object_size: object.size,
            key: (!crypto::is_content_type(&file.file_type))
                .then(|| hex::encode(key)),
        })
        .collect::<Vec<_>>();
    for (file, object) in carried {
        external_files.push(file);
        objects.push(object);
    }
    let root = hex::encode(crypto::root(objects.iter().map(|o| o.object)));

    let created = backend
        .create_version(
            share_id,
            &NewVersion {
                content,
                external_files: &external_files,
                root: &root,
                server_core,
            },
        )
        .await?;
    on_event(PushEvent::Created(created.version));
    if created.ready {
        return Ok(created.version);
    }

    if let Some(grant) = &created.store {
        let sent = store_version(
            store,
            &grant.core,
            &grant.ticket,
            &root,
            &objects,
            on_event,
        )
        .await?;
        on_event(PushEvent::Stored {
            objects: sent.len(),
            files: external_files
                .iter()
                .filter(|file| sent.contains(&file.object))
                .map(|file| file.file_name.clone())
                .collect(),
        });
    }

    for _ in 0..CONFIRM_ATTEMPTS {
        tokio::time::sleep(CONFIRM_INTERVAL).await;
        if backend
            .version::<Version>(share_id, Some(created.version))
            .await?
            .is_some_and(|version| version.ready)
        {
            on_event(PushEvent::Confirmed);
            return Ok(created.version);
        }
    }
    Err(PushError::Unconfirmed)
}

async fn hash_files(files: &[PushFile]) -> Result<Vec<HashedFile>, PushError> {
    let hashes = files.iter().map(hash_file).collect::<Vec<_>>();
    futures::stream::iter(hashes)
        .buffered(CONCURRENCY)
        .try_collect()
        .await
}

async fn hash_file(file: &PushFile) -> Result<HashedFile, PushError> {
    let mut reader = tokio::fs::File::open(&file.path).await?;
    let mut sha1 = sha1_smol::Sha1::new();
    let mut sha256 = sha2::Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut head = Vec::with_capacity(SCAN_HEAD_SIZE);
    let mut size = 0_u64;
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        sha1.update(&buffer[..read]);
        sha256.update(&buffer[..read]);
        let remaining = SCAN_HEAD_SIZE - head.len();
        head.extend_from_slice(&buffer[..read.min(remaining)]);
        size += read as u64;
    }
    Ok(HashedFile {
        file_name: file.file_name.clone(),
        file_type: file.file_type.clone(),
        size,
        sha1: sha1.digest().to_string(),
        sha256: sha256.finalize().into(),
        executable: is_executable_content(&head),
        path: file.path.clone(),
    })
}

/// The versions a push builds on: the latest one, then the version a crashed push left
/// unconfirmed one past it, if there is one.
async fn previous_versions(
    backend: &Backend,
    share_id: &str,
) -> Result<Vec<Version>, PushError> {
    let latest = match backend.version::<Version>(share_id, None).await {
        Ok(Some(latest)) => latest,
        Ok(None) | Err(PushError::Unavailable(_)) => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let pending = if latest.ready {
        backend.version(share_id, Some(latest.version + 1)).await?
    } else {
        None
    };
    Ok([latest].into_iter().chain(pending).collect())
}

/// The config files of the latest ready version that this push does not send again. Configs
/// are picked per push, so each version carries the earlier ones forward and lists every config
/// shared so far: any version stands alone, however many older ones the backend has dropped.
/// The Core still holds their objects, since it keeps the latest ready version.
fn carried_config_files(
    previous: &[Version],
    pushed: &[HashedFile],
) -> Vec<(ManifestFile, Object)> {
    let Some(latest) = previous.first().filter(|version| version.ready) else {
        return Vec::new();
    };
    let pushed = pushed
        .iter()
        .map(|file| (file.file_type.as_str(), file.file_name.as_str()))
        .collect::<HashSet<_>>();
    latest
        .external_files
        .iter()
        .filter(|file| {
            matches!(
                file.file_type.as_str(),
                CONFIG_FILE_TYPE | CONFIG_BUNDLE_FILE_TYPE
            ) && !pushed
                .contains(&(file.file_type.as_str(), file.file_name.as_str()))
        })
        .filter_map(|file| {
            let carried = carried_config_file(file);
            if carried.is_none() {
                tracing::warn!(
                    file_name = %file.file_name,
                    "A shared config file has an incomplete manifest entry and is not carried forward"
                );
            }
            carried
        })
        .collect()
}

fn carried_config_file(file: &KeptFile) -> Option<(ManifestFile, Object)> {
    let object = file.object.as_deref().and_then(parse_hash)?;
    let object_size = file.object_size?;
    let sha256 = file.sha256.as_deref().and_then(parse_hash)?;
    Some((
        ManifestFile {
            file_name: file.file_name.clone(),
            file_type: file.file_type.clone(),
            file_size: file
                .file_size
                .and_then(|size| u64::try_from(size).ok())?,
            sha1: file.sha1.clone()?,
            sha256: hex::encode(sha256),
            object: hex::encode(object),
            object_size,
            key: Some(file.key.clone()?),
        },
        Object {
            object,
            size: object_size,
            path: None,
            plain: sha256,
        },
    ))
}

/// Each file's key. Content files derive theirs from P. Other files keep the key an earlier
/// version gave the same P, so an unchanged file stays the same object, and get a new random
/// key otherwise. A push that crashed after creating its version left it unconfirmed one past
/// the latest ready one; its keys win, so pushing the same files again gives the same root and
/// resumes that upload instead of waiting for it to expire.
fn file_keys(previous: &[Version], files: &[HashedFile]) -> Vec<[u8; 32]> {
    let mut keys = HashMap::new();
    for file in previous
        .iter()
        .rev()
        .flat_map(|version| &version.external_files)
    {
        if let (Some(p), Some(key)) = (
            file.sha256.as_deref().and_then(parse_hash),
            file.key.as_deref().and_then(parse_hash),
        ) {
            keys.entry(p).or_insert(key);
        }
    }

    files
        .iter()
        .map(|file| {
            if crypto::is_content_type(&file.file_type) {
                return crypto::content_key(&file.sha256);
            }
            *keys.entry(file.sha256).or_insert_with(rand::random)
        })
        .collect()
}

/// Encrypts `source` into `folder`, named by its C once that is known.
async fn encrypt_file(
    key: &[u8; 32],
    source: &Path,
    folder: &Path,
) -> Result<Object, PushError> {
    let part = folder
        .join(format!("{}.part", hex::encode(rand::random::<[u8; 16]>())));
    let reader = tokio::fs::File::open(source).await?;
    let writer = tokio::fs::File::create(&part).await?;
    let encrypted = crypto::encrypt(key, reader, writer).await?;
    let path = folder.join(hex::encode(encrypted.object));
    tokio::fs::rename(&part, &path).await?;
    Ok(Object {
        object: encrypted.object,
        size: encrypted.size,
        path: Some(path),
        plain: encrypted.plain,
    })
}

/// Stores a version's objects on `core`, sending only those it lacks, and returns the C of each
/// object it sent.
async fn store_version(
    store: &StoreClient,
    core: &PoolCore,
    ticket: &str,
    root: &str,
    objects: &[Object],
    on_event: &mut (dyn FnMut(PushEvent<'_>) + Send),
) -> Result<HashSet<String>, PushError> {
    let paths = objects
        .iter()
        .filter_map(|object| {
            Some((hex::encode(object.object), object.path.clone()?))
        })
        .collect::<HashMap<_, _>>();
    let mut list = objects
        .iter()
        .map(|object| ListedObject {
            hash: hex::encode(object.object),
            size: object.size,
        })
        .collect::<Vec<_>>();
    list.sort_by(|left, right| left.hash.cmp(&right.hash));
    list.dedup_by(|left, right| left.hash == right.hash);
    let hashes = list
        .iter()
        .map(|object| object.hash.clone())
        .collect::<Vec<_>>();

    let have = store.have(core, ticket, root, &hashes).await?;
    if have.complete {
        return Ok(HashSet::new());
    }
    let mut missing = have.missing;
    if missing.iter().any(|hash| !paths.contains_key(hash)) {
        return Err(PushError::CarriedFileMissing);
    }
    let requested = missing.iter().cloned().collect::<HashSet<_>>();
    let total = list
        .iter()
        .filter(|object| requested.contains(&object.hash))
        .map(|object| object.size)
        .sum();
    let uploaded = Arc::new(AtomicU64::new(0));
    on_event(PushEvent::Progress(0, total));
    for _ in 0..UPLOAD_ROUNDS {
        let sending = missing
            .iter()
            .filter_map(|hash| Some((hash.clone(), paths.get(hash)?.clone())))
            .collect::<Vec<_>>();
        let upload = store.upload(
            core,
            ticket,
            root,
            &list,
            sending,
            Arc::clone(&uploaded),
        );
        tokio::pin!(upload);
        let mut ticker = tokio::time::interval(UPLOAD_PROGRESS_INTERVAL);
        let outcome = loop {
            tokio::select! {
                outcome = &mut upload => break outcome,
                _ = ticker.tick() => on_event(PushEvent::Progress(
                    uploaded.load(Ordering::Relaxed).min(total),
                    total,
                )),
            }
        }?;
        if outcome.complete {
            on_event(PushEvent::Progress(total, total));
            return Ok(requested);
        }
        missing = outcome.missing;
    }
    Err(PushError::NotAccepted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hashed(file_type: &str, file_name: &str, p: u8) -> HashedFile {
        HashedFile {
            file_name: file_name.into(),
            file_type: file_type.into(),
            size: 1,
            sha1: "sha1".into(),
            sha256: [p; 32],
            executable: false,
            path: PathBuf::new(),
        }
    }

    fn kept(file_type: &str, file_name: &str, p: u8, key: u8) -> KeptFile {
        KeptFile {
            file_name: file_name.into(),
            file_type: file_type.into(),
            file_size: Some(1),
            sha1: Some("sha1".into()),
            sha256: Some(hex::encode([p; 32])),
            object: Some(hex::encode([p ^ 0xff; 32])),
            object_size: Some(17),
            key: Some(hex::encode([key; 32])),
        }
    }

    fn version(version: i32, ready: bool, files: Vec<KeptFile>) -> Version {
        Version {
            version,
            ready,
            external_files: files,
            store: None,
        }
    }

    #[test]
    fn content_files_derive_their_key_and_configs_keep_theirs() {
        let previous = [version(1, true, vec![kept("config", "a.json", 1, 9)])];
        let files = [
            hashed("mod", "a.jar", 2),
            hashed("config", "a.json", 1),
            hashed("config", "b.json", 3),
        ];
        let keys = file_keys(&previous, &files);
        assert_eq!(keys[0], crypto::content_key(&[2; 32]));
        assert_eq!(keys[1], [9; 32]);
        assert_ne!(keys[2], [9; 32]);
    }

    #[test]
    fn an_unconfirmed_version_s_keys_win() {
        let previous = [
            version(1, true, vec![kept("config", "a.json", 1, 9)]),
            version(2, false, vec![kept("config", "a.json", 1, 7)]),
        ];
        let keys = file_keys(&previous, &[hashed("config", "a.json", 1)]);
        assert_eq!(keys[0], [7; 32]);
    }

    #[test]
    fn configs_not_pushed_again_are_carried_when_complete() {
        let mut incomplete = kept("config", "c.json", 3, 9);
        incomplete.sha1 = None;
        let previous = [version(
            1,
            true,
            vec![
                kept("config", "a.json", 1, 9),
                kept("config", "b.json", 2, 9),
                incomplete,
                kept("mod", "a.jar", 4, 9),
            ],
        )];
        let carried =
            carried_config_files(&previous, &[hashed("config", "b.json", 5)]);
        let names = carried
            .iter()
            .map(|(file, _)| file.file_name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["a.json"]);
        assert!(carried[0].1.path.is_none());
    }

    #[test]
    fn nothing_is_carried_from_an_unready_latest_version() {
        let previous =
            [version(1, false, vec![kept("config", "a.json", 1, 9)])];
        assert!(carried_config_files(&previous, &[]).is_empty());
    }
}
