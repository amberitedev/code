//! The writer of a share's versions (`docs/internals/pool.md`, Push): hash the files, keep the
//! configs the owner still shares, reuse keys from the previous version, encrypt, create the
//! version on the backend, store the objects on the Core it picks, and wait until that Core
//! confirms them.
//!
//! The App pushes an instance's share with [`push`], and a Core pushes a server's share with
//! the same call. What goes into a version is the caller's: it picks the files and passes the
//! upstream content fields (Modrinth ids, loader, and so on) as they are.
//!
//! Configs are picked per push, and a version lists every config still shared: one the latest
//! version carries and this push does not send again is kept while [`KeepConfig`] finds it, so
//! the owner's local copy is sent again. A config the owner stopped sharing, or no longer has, is
//! left out of the version and reported with [`PushEvent::Dropped`].

mod hash;
mod upload;

pub use hash::HashedFile;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use bedrock::limits::MAX_SHARE_BYTES;
use futures::future::BoxFuture;
use futures::{StreamExt, TryStreamExt};
use reqwest::StatusCode;

use crate::backend::{Backend, BackendError, NewVersion, Unavailable};
use crate::crypto;
use crate::pool::{StoreClient, StoreError};
use crate::versions::{ManifestFile, ShareVersion, VersionContent};

/// Files hashed or encrypted at once.
const CONCURRENCY: usize = 4;
/// The Core confirms a stored version to the backend on its own, so a push waits up to a minute.
const CONFIRM_INTERVAL: Duration = Duration::from_secs(2);
const CONFIRM_ATTEMPTS: usize = 30;

#[derive(Debug, thiserror::Error)]
pub enum PushError {
    #[error("{0} is an executable, which a share cannot carry")]
    Executable(String),
    #[error("a share can hold at most {MAX_SHARE_BYTES} bytes of files")]
    TooLarge,
    #[error("{0} changed while it was being pushed")]
    FileChanged(String),
    #[error("no Core offers storage for this share")]
    NoStorage,
    #[error("another push of this share is still uploading")]
    PushInFlight,
    /// A limit the backend names in its own words.
    #[error("{0}")]
    Refused(String),
    #[error(transparent)]
    Unavailable(Unavailable),
    #[error(transparent)]
    Backend(BackendError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("the Core did not accept every shared file; push again")]
    NotAccepted,
    #[error("the Core did not confirm the shared files in time")]
    Unconfirmed,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<BackendError> for PushError {
    fn from(error: BackendError) -> Self {
        match error {
            BackendError::Unavailable { reason, .. } => {
                Self::Unavailable(reason)
            }
            error => Self::Backend(error),
        }
    }
}

/// A file to push: its manifest name and type, and where its bytes are now.
pub struct PushFile {
    pub file_name: String,
    pub file_type: String,
    pub path: PathBuf,
}

/// Where the owner's copy of a config the latest version carries is now: `Some` while the owner
/// still shares it and has it, so it is sent again from there; `None` to leave it out. The path
/// must stay readable until the push ends.
pub type KeepConfig = dyn Fn(String) -> BoxFuture<'static, std::io::Result<Option<PathBuf>>>
    + Send
    + Sync;

/// How a push is going, for the caller's progress and logs.
pub enum PushEvent<'a> {
    /// Configs of the latest version this one leaves out.
    Dropped(&'a [String]),
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

/// An encrypted object of the push.
struct Object {
    object: [u8; 32],
    size: u64,
    path: PathBuf,
    /// P of the plaintext that was encrypted, which the push compares to the P it hashed.
    plain: [u8; 32],
}

/// Pushes a new version of the share `share_id` holding `files`, plus the configs of the latest
/// version that `keep` still finds, and returns its number once members can install it.
/// `server_core` is a Core of the owner running a server of this share, which the backend makes
/// its home. Encrypted files wait in a folder of their own under `scratch` until the Core has
/// them.
#[allow(clippy::too_many_arguments)]
pub async fn push(
    backend: &Backend,
    store: &StoreClient,
    share_id: &str,
    content: &VersionContent,
    server_core: Option<&str>,
    files: Vec<PushFile>,
    keep: &KeepConfig,
    scratch: &Path,
    on_event: &mut (dyn FnMut(PushEvent<'_>) + Send),
) -> Result<i32, PushError> {
    let previous = previous_versions(backend, share_id).await?;
    let files = with_kept_configs(&previous, files, keep, on_event).await?;
    let hashed = hash::hash_files(&files).await?;
    on_event(PushEvent::Hashed(&hashed));
    if let Some(file) = hashed.iter().find(|file| file.executable) {
        return Err(PushError::Executable(file.file_name.clone()));
    }
    let total_size = hashed
        .iter()
        .fold(0_u64, |total, file| total.saturating_add(file.size));
    if total_size > MAX_SHARE_BYTES {
        return Err(PushError::TooLarge);
    }

    let folder = scratch.join(hex::encode(rand::random::<[u8; 16]>()));
    let result = push_hashed(
        backend,
        store,
        share_id,
        content,
        server_core,
        &previous,
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
async fn push_hashed(
    backend: &Backend,
    store: &StoreClient,
    share_id: &str,
    content: &VersionContent,
    server_core: Option<&str>,
    previous: &[ShareVersion],
    hashed: &[HashedFile],
    folder: &Path,
    on_event: &mut (dyn FnMut(PushEvent<'_>) + Send),
) -> Result<i32, PushError> {
    let keys = file_keys(previous, hashed);
    tokio::fs::create_dir_all(folder).await?;
    // Collected first so the stream holds plain futures, not a borrowing closure, which keeps
    // the push future Send for Tauri commands.
    let encryptions = hashed
        .iter()
        .zip(&keys)
        .map(|(file, key)| encrypt_file(key, file.path(), folder))
        .collect::<Vec<_>>();
    let objects = futures::stream::iter(encryptions)
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

    let external_files = hashed
        .iter()
        .zip(&keys)
        .zip(&objects)
        .map(|((file, key), object)| ManifestFile {
            file_name: file.file_name.clone(),
            file_type: file.file_type.clone(),
            file_size: file.size,
            sha1: file.sha1.clone(),
            sha256: file.sha256,
            object: object.object,
            object_size: object.size,
            key: (!crypto::is_content_type(&file.file_type)).then_some(*key),
        })
        .collect::<Vec<_>>();
    let root = hex::encode(crypto::root(objects.iter().map(|o| o.object)));

    let created = backend
        .create_version(
            share_id,
            &NewVersion {
                content,
                external_files: &external_files,
                root: root.clone(),
                server_core,
            },
        )
        .await
        .map_err(refusal)?;
    on_event(PushEvent::Created(created.version));
    if created.ready {
        return Ok(created.version);
    }

    if let Some(grant) = &created.store {
        let sent = upload::store_version(
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
                .filter(|file| sent.contains(&hex::encode(file.object)))
                .map(|file| file.file_name.clone())
                .collect(),
        });
    }

    for _ in 0..CONFIRM_ATTEMPTS {
        tokio::time::sleep(CONFIRM_INTERVAL).await;
        match backend.version(share_id, Some(created.version)).await {
            Ok(version) if version.ready => {
                on_event(PushEvent::Confirmed);
                return Ok(created.version);
            }
            Ok(_)
            | Err(BackendError::Unavailable {
                reason: Unavailable::Deleted,
                ..
            }) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Err(PushError::Unconfirmed)
}

/// The pool's refusals of a new version, named by the backend's `code`. Every refusal over a
/// limit (413) carries a description naming the limit, which is shown as is.
fn refusal(error: BackendError) -> PushError {
    if let BackendError::Status { status, body, .. } = &error
        && matches!(
            *status,
            StatusCode::CONFLICT
                | StatusCode::PAYLOAD_TOO_LARGE
                | StatusCode::SERVICE_UNAVAILABLE
        )
    {
        match body.code.as_deref() {
            Some("no_storage") => return PushError::NoStorage,
            Some("push_in_flight") => return PushError::PushInFlight,
            _ if *status == StatusCode::PAYLOAD_TOO_LARGE
                && !body.description.is_empty() =>
            {
                return PushError::Refused(body.description.clone());
            }
            _ => {}
        }
    }
    error.into()
}

/// The versions a push builds on: the latest one, then the version a crashed push left
/// unconfirmed one past it, if there is one. A share without a version (404) is a new share; a
/// token that no longer reaches it (401) fails the push.
async fn previous_versions(
    backend: &Backend,
    share_id: &str,
) -> Result<Vec<ShareVersion>, PushError> {
    let unless_missing =
        |result: Result<ShareVersion, BackendError>| match result {
            Ok(version) => Ok(Some(version)),
            Err(BackendError::Unavailable {
                reason: Unavailable::Deleted,
                ..
            }) => Ok(None),
            Err(error) => Err(PushError::from(error)),
        };
    let Some(latest) = unless_missing(backend.version(share_id, None).await)?
    else {
        return Ok(Vec::new());
    };
    let pending = if latest.ready {
        unless_missing(
            backend.version(share_id, Some(latest.version + 1)).await,
        )?
    } else {
        None
    };
    Ok([latest].into_iter().chain(pending).collect())
}

/// `files` plus the configs of the latest ready version that this push does not send again and
/// that `keep` still finds. Reports the ones it leaves out.
async fn with_kept_configs(
    previous: &[ShareVersion],
    mut files: Vec<PushFile>,
    keep: &KeepConfig,
    on_event: &mut (dyn FnMut(PushEvent<'_>) + Send),
) -> Result<Vec<PushFile>, PushError> {
    let mut dropped = Vec::new();
    for file_name in kept_configs(previous, &files) {
        match keep(file_name.clone()).await? {
            Some(path) => files.push(PushFile {
                file_name,
                file_type: crate::rules::CONFIG_FILE_TYPE.to_owned(),
                path,
            }),
            None => dropped.push(file_name),
        }
    }
    if !dropped.is_empty() {
        on_event(PushEvent::Dropped(&dropped));
    }
    Ok(files)
}

/// The configs of the latest ready version that `pushed` does not send again.
fn kept_configs(previous: &[ShareVersion], pushed: &[PushFile]) -> Vec<String> {
    let Some(latest) = previous.first().filter(|version| version.ready) else {
        return Vec::new();
    };
    let pushed = pushed
        .iter()
        .map(|file| (file.file_type.as_str(), file.file_name.as_str()))
        .collect::<HashSet<_>>();
    latest
        .configs()
        .filter(|file| {
            !pushed
                .contains(&(file.file_type.as_str(), file.file_name.as_str()))
        })
        .map(|file| file.file_name.clone())
        .collect()
}

/// Each file's key. Content files derive theirs from P. Other files keep the key an earlier
/// version gave the same P, so an unchanged file stays the same object and the Core already has
/// it, and get a new random key otherwise. A push that crashed after creating its version left
/// it unconfirmed one past the latest ready one; its keys win, so pushing the same files again
/// gives the same root and resumes that upload instead of waiting for it to expire.
fn file_keys(previous: &[ShareVersion], files: &[HashedFile]) -> Vec<[u8; 32]> {
    let mut keys = HashMap::new();
    for file in previous
        .iter()
        .rev()
        .flat_map(|version| &version.external_files)
    {
        if let Some(key) = file.key {
            keys.entry(file.sha256).or_insert(key);
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
        path,
        plain: encrypted.plain,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(
        file_type: &str,
        file_name: &str,
        p: u8,
        key: u8,
    ) -> ManifestFile {
        ManifestFile {
            file_name: file_name.into(),
            file_type: file_type.into(),
            file_size: 1,
            sha1: "sha1".into(),
            sha256: [p; 32],
            object: [p ^ 0xff; 32],
            object_size: 17,
            key: Some([key; 32]),
        }
    }

    fn version(
        version: i32,
        ready: bool,
        files: Vec<ManifestFile>,
    ) -> ShareVersion {
        ShareVersion {
            version,
            ready,
            content: VersionContent {
                modrinth_ids: Vec::new(),
                modpack_id: None,
                removed_files: Vec::new(),
                game_version: "1.21.1".into(),
                loader: "fabric".into(),
                loader_version: String::new(),
            },
            root: [0; 32],
            external_files: files,
            pool: None,
            store: None,
            backtrack_version: None,
        }
    }

    fn pushed(file_type: &str, file_name: &str) -> PushFile {
        PushFile {
            file_name: file_name.into(),
            file_type: file_type.into(),
            path: PathBuf::new(),
        }
    }

    #[test]
    fn content_files_derive_their_key_and_configs_keep_theirs() {
        let previous =
            [version(1, true, vec![manifest("config", "a.json", 1, 9)])];
        let files = [
            HashedFile::test("mod", "a.jar", 2),
            HashedFile::test("config", "a.json", 1),
            HashedFile::test("config", "b.json", 3),
        ];
        let keys = file_keys(&previous, &files);
        assert_eq!(keys[0], crypto::content_key(&[2; 32]));
        assert_eq!(keys[1], [9; 32]);
        assert_ne!(keys[2], [9; 32]);
    }

    #[test]
    fn an_unconfirmed_version_s_keys_win() {
        let previous = [
            version(1, true, vec![manifest("config", "a.json", 1, 9)]),
            version(2, false, vec![manifest("config", "a.json", 1, 7)]),
        ];
        let keys =
            file_keys(&previous, &[HashedFile::test("config", "a.json", 1)]);
        assert_eq!(keys[0], [7; 32]);
    }

    #[test]
    fn configs_not_pushed_again_are_kept() {
        let previous = [version(
            1,
            true,
            vec![
                manifest("config", "a.json", 1, 9),
                manifest("config", "b.json", 2, 9),
                manifest("mod", "a.jar", 4, 9),
            ],
        )];
        let kept = kept_configs(&previous, &[pushed("config", "b.json")]);
        assert_eq!(kept, ["a.json"]);
    }

    #[test]
    fn nothing_is_kept_from_an_unready_latest_version() {
        let previous =
            [version(1, false, vec![manifest("config", "a.json", 1, 9)])];
        assert!(kept_configs(&previous, &[]).is_empty());
    }

    #[tokio::test]
    async fn a_config_the_owner_no_longer_has_is_dropped_and_reported() {
        let previous = [version(
            1,
            true,
            vec![
                manifest("config", "config/kept.json", 1, 9),
                manifest("config", "config/gone.json", 2, 9),
            ],
        )];
        let keep: &KeepConfig = &|name: String| {
            Box::pin(async move {
                Ok((name == "config/kept.json").then(|| PathBuf::from(name)))
            })
        };
        let mut dropped = Vec::new();
        let files =
            with_kept_configs(&previous, Vec::new(), keep, &mut |event| {
                if let PushEvent::Dropped(names) = event {
                    dropped.extend_from_slice(names);
                }
            })
            .await
            .unwrap();
        let names = files
            .iter()
            .map(|file| file.file_name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["config/kept.json"]);
        assert_eq!(dropped, ["config/gone.json"]);
    }
}
