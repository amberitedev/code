//! The client for a Core's store routes ([`contracts::store`]),
//! used by an owner's App to store a version, by members' Apps and Cores to fetch its files, and
//! by a store to pull a version from another Core.
//! A Core serves HTTPS with a certificate it made itself, so each Core gets a client that trusts
//! only the certificate whose SHA-256 the backend handed out. Only `https://ip:port` addresses are
//! used, so a ticket never leaves over plain HTTP.

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use contracts::client::{CallError, Remote, call, call_limited, call_stream};
use contracts::store::{
    GetObject, ListVersion, MissingObjects, ObjectHashes, UploadStatus,
    UploadVersion, UploadedVersion, VersionHash, VersionList, VersionObject,
};
use contracts::tickets::StoreHost;
use contracts::{Empty, ErrorCode, Hash, Json};
use futures::future::BoxFuture;
use futures::stream::BoxStream;
use futures::{Stream, StreamExt, TryStreamExt};
use tokio::io::AsyncReadExt;

use crate::crypto::{DecryptError, Decryptor};
use crate::net::{CoreAddress, tls};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("the Core's storage is full")]
    DiskFull,
    #[error("the Core has used up today's transfer allowance")]
    EgressSpent,
    #[error("the Core is limiting requests")]
    RateLimited,
    #[error("the Core refused the ticket ({0:?})")]
    Ticket(ErrorCode),
    #[error("the Core does not have this")]
    NotFound,
    #[error("the Core answered {0}: {1}")]
    Other(u16, String),
    #[error("could not reach the Core: {0}")]
    Http(#[from] reqwest::Error),
}

impl From<CallError> for StoreError {
    fn from(error: CallError) -> Self {
        match error {
            CallError::Http(error) => Self::Http(error),
            CallError::Api { status, error } => match error.error {
                ErrorCode::DiskFull => Self::DiskFull,
                ErrorCode::EgressSpent => Self::EgressSpent,
                ErrorCode::RateLimited => Self::RateLimited,
                ErrorCode::NotFound => Self::NotFound,
                code @ (ErrorCode::TicketRequired
                | ErrorCode::TicketInvalid
                | ErrorCode::TicketReused
                | ErrorCode::TicketExpired
                | ErrorCode::TicketWrongCore
                | ErrorCode::TicketWrongKind) => Self::Ticket(code),
                _ => Self::Other(status, error.description),
            },
            error @ (CallError::Decode { status, .. }
            | CallError::RangeIgnored { status }) => {
                Self::Other(status, error.to_string())
            }
            error @ CallError::Params { .. } => {
                Self::Other(0, error.to_string())
            }
        }
    }
}

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a [`StoreClient::direct`] client waits for the next bytes of an answer.
const DIRECT_READ_TIMEOUT: Duration = Duration::from_secs(60);
const PIPE_SIZE: usize = 256 * 1024;
/// Further tries at one holder after a dropped connection, each from where the last stopped.
const RESUME_ATTEMPTS: usize = 2;
/// Waits at one holder that answered it is busy (every fetch slot taken), with the wait doubling
/// up to [`RATE_LIMIT_MAX_WAIT`]: about three minutes in all.
const RATE_LIMIT_WAITS: u32 = 8;
const RATE_LIMIT_MAX_WAIT: Duration = Duration::from_secs(30);

/// Reaches Cores' stores. Keep one for the life of the process (the App and Copal each hold one,
/// and so does the store): it reuses a connection per Core and remembers which of a Core's
/// addresses last connected.
pub struct StoreClient {
    user_agent: String,
    /// Uses no proxy and gives up on a stalled read, for a store pulling from a Core it was told
    /// about.
    direct: bool,
    /// One client per pinned certificate.
    clients: Mutex<HashMap<[u8; 32], reqwest::Client>>,
    /// The address that last connected, per Core id.
    addresses: Mutex<HashMap<String, String>>,
}

/// One file of a version, as a member fetches it: where the version is, and the names and key
/// that check and decrypt the object.
pub struct PoolObject<'a> {
    /// R.
    pub root: Hash,
    pub holders: &'a [StoreHost],
    /// The fetch ticket for `root`.
    pub ticket: &'a str,
    /// C.
    pub object: [u8; 32],
    pub object_size: u64,
    /// P.
    pub plain: [u8; 32],
    pub key: [u8; 32],
}

/// A holder that served bad bytes during a fetch, for the caller to report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
    pub core: String,
    pub kind: FaultKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind {
    /// The bytes did not hash to C: the Core's fault.
    Object,
    /// The bytes decrypted to another file than P: the manifest's fault.
    Content,
}

impl FaultKind {
    /// As `POST /v1/instances/:id/versions/:n/reports` names it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Object => "object",
            Self::Content => "content",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError<E> {
    /// The file decrypts to another file than the manifest names. Every holder has the same
    /// bytes, so only a new push fixes it.
    #[error("the shared file does not match its manifest")]
    Broken,
    /// No holder served the file.
    #[error("no Core holding the shared file could serve it")]
    Unavailable,
    /// The sink failed on its own.
    #[error(transparent)]
    Sink(E),
}

#[derive(Debug, thiserror::Error)]
enum AttemptError {
    #[error(transparent)]
    Decrypt(#[from] DecryptError),
    #[error(transparent)]
    Http(#[from] StoreError),
}

impl StoreClient {
    pub fn new(user_agent: impl Into<String>) -> Self {
        Self {
            user_agent: user_agent.into(),
            direct: false,
            clients: Mutex::default(),
            addresses: Mutex::default(),
        }
    }

    /// A client that uses no proxy and gives up on a read stalled for [`DIRECT_READ_TIMEOUT`],
    /// so a store pulling from another Core cannot be held or redirected.
    pub fn direct(user_agent: impl Into<String>) -> Self {
        Self {
            direct: true,
            ..Self::new(user_agent)
        }
    }

    fn client(&self, core: &StoreHost) -> Result<reqwest::Client, StoreError> {
        let fingerprint = tls::parse_fingerprint(&core.cert_fingerprint)
            .ok_or_else(|| {
                StoreError::Other(0, "the Core's fingerprint is invalid".into())
            })?;
        let mut clients =
            self.clients.lock().expect("store clients lock poisoned");
        if let Some(client) = clients.get(&fingerprint) {
            return Ok(client.clone());
        }
        let config = tls::pinned_config(fingerprint)
            .map_err(|error| StoreError::Other(0, error.to_string()))?;
        // No overall timeout: an object can take minutes on a slow uplink.
        let mut builder = reqwest::Client::builder()
            .use_preconfigured_tls(config)
            .connect_timeout(CONNECT_TIMEOUT)
            .tcp_keepalive(Some(Duration::from_secs(10)))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(&self.user_agent);
        if self.direct {
            builder = builder.no_proxy().read_timeout(DIRECT_READ_TIMEOUT);
        }
        let client = builder.build()?;
        clients.insert(fingerprint, client.clone());
        Ok(client)
    }

    /// The addresses to try, the one that last worked first. Only `https://ip:port` ones.
    fn addresses(&self, core: &StoreHost) -> Vec<String> {
        let remembered = self
            .addresses
            .lock()
            .expect("store addresses lock poisoned")
            .get(&core.id)
            .cloned();
        let mut addresses = Vec::with_capacity(3);
        for address in remembered
            .into_iter()
            .chain([core.address.clone()])
            .chain(core.public_address.clone())
        {
            let Ok(address) = CoreAddress::parse(&address) else {
                continue;
            };
            let address = address.to_string();
            if !addresses.contains(&address) {
                addresses.push(address);
            }
        }
        addresses
    }

    fn remember(&self, core: &StoreHost, address: &str) {
        self.addresses
            .lock()
            .expect("store addresses lock poisoned")
            .insert(core.id.clone(), address.to_string());
    }

    /// Makes a call that can be repeated, moving to the next address when one does not connect.
    async fn send<'a, T>(
        &self,
        core: &StoreHost,
        call: impl Fn(
            reqwest::Client,
            String,
        ) -> BoxFuture<'a, Result<T, CallError>>,
    ) -> Result<T, StoreError> {
        let client = self.client(core)?;
        let mut unreachable = None;
        for address in self.addresses(core) {
            match call(client.clone(), address.clone()).await {
                Err(CallError::Http(error)) if error.is_connect() => {
                    unreachable = Some(error);
                }
                Err(CallError::Http(error)) => return Err(error.into()),
                result => {
                    self.remember(core, &address);
                    return Ok(result?);
                }
            }
        }
        Err(unreachable.map_or_else(
            || StoreError::Other(0, "the Core has no usable address".into()),
            StoreError::Http,
        ))
    }

    /// Which of `objects` the Core lacks for the version `root`.
    pub async fn have(
        &self,
        core: &StoreHost,
        ticket: &str,
        root: Hash,
        objects: Vec<Hash>,
    ) -> Result<UploadStatus, StoreError> {
        let body = Json(ObjectHashes { objects });
        self.send(core, |http, address| {
            let body = body.clone();
            Box::pin(async move {
                let remote = remote(&http, &address, ticket);
                call::<MissingObjects>(
                    &remote,
                    &VersionHash { root },
                    &(),
                    body,
                )
                .await
            })
        })
        .await
    }

    /// Streams the version's list and the given objects to the Core as one tar. `objects` are
    /// `(C, object file)` pairs; their sizes come from `list`. `uploaded` counts object bytes
    /// sent.
    pub async fn upload(
        &self,
        core: &StoreHost,
        ticket: &str,
        root: Hash,
        list: &VersionList,
        objects: impl IntoIterator<Item = (Hash, PathBuf)>,
        uploaded: Arc<AtomicU64>,
    ) -> Result<UploadedVersion, StoreError> {
        let sizes = list
            .objects
            .iter()
            .map(|object| (object.hash, object.size))
            .collect::<HashMap<_, _>>();
        let entries = objects
            .into_iter()
            .filter_map(|(hash, path)| {
                let size = *sizes.get(&hash)?;
                Some((hash.to_string(), path, size))
            })
            .collect::<Vec<_>>();
        let list = serde_json::to_vec(list)
            .map_err(|error| StoreError::Other(0, error.to_string()))?;
        let body = tar_body(list, entries, uploaded);
        // The body streams once, so it goes to the address `have` just reached.
        let Some(address) = self.addresses(core).into_iter().next() else {
            return Err(StoreError::Other(
                0,
                "the Core has no usable address".into(),
            ));
        };
        Ok(call::<UploadVersion>(
            &remote(&self.client(core)?, &address, ticket),
            &VersionHash { root },
            &(),
            reqwest::Body::wrap_stream(body),
        )
        .await?)
    }

    /// The list of version `root` as the Core stored it, refused past
    /// [`MAX_VERSION_LIST_BYTES`](contracts::limits::MAX_VERSION_LIST_BYTES).
    pub async fn version_list(
        &self,
        core: &StoreHost,
        ticket: &str,
        root: Hash,
    ) -> Result<VersionList, StoreError> {
        self.send(core, |http, address| {
            Box::pin(async move {
                call_limited::<ListVersion, _>(
                    &remote(&http, &address, ticket),
                    &VersionHash { root },
                    &(),
                    Empty,
                    contracts::limits::MAX_VERSION_LIST_BYTES as usize,
                )
                .await
            })
        })
        .await
    }

    /// The bytes of `object` of version `root`, unchecked: the caller checks them against C.
    pub async fn object_reader(
        &self,
        core: &StoreHost,
        ticket: &str,
        root: Hash,
        object: Hash,
    ) -> Result<impl tokio::io::AsyncRead + Send + Unpin + use<>, StoreError>
    {
        let response = self.object(core, ticket, root, object, 0).await?;
        Ok(tokio_util::io::StreamReader::new(
            response.bytes_stream().map_err(std::io::Error::other),
        ))
    }

    /// `object` of version `root`, from byte `from` on.
    async fn object(
        &self,
        core: &StoreHost,
        ticket: &str,
        root: Hash,
        object: Hash,
        from: u64,
    ) -> Result<reqwest::Response, StoreError> {
        self.send(core, |http, address| {
            Box::pin(async move {
                let remote = remote(&http, &address, ticket);
                let params = VersionObject { root, object };
                call_stream::<GetObject>(&remote, &params, &(), from).await
            })
        })
        .await
    }

    /// Fetches one file from the first holder that serves it correctly, checked against C and
    /// then P. `sink` gets the plain bytes and is called afresh for each holder; only the
    /// result of a sink whose holder served the whole file correctly is returned, and the last
    /// chunk reaches it only once the file checks out. A dropped connection resumes from the
    /// last whole chunk, and a busy holder is waited for. Holders that served bad bytes are
    /// added to `faults`.
    pub async fn fetch_file<T, E, Fut>(
        &self,
        file: &PoolObject<'_>,
        downloaded: &(dyn Fn(u64) + Sync),
        faults: &mut Vec<Fault>,
        mut sink: impl FnMut(tokio::io::DuplexStream) -> Fut,
    ) -> Result<T, FetchError<E>>
    where
        Fut: Future<Output = Result<T, E>>,
    {
        let object = Hash(file.object);
        for core in file.holders {
            let (writer, reader) = tokio::io::duplex(PIPE_SIZE);
            let decrypt = async {
                let mut writer = writer;
                let mut decryptor =
                    Decryptor::new(&file.key, file.object, file.plain);
                let mut attempt = 0;
                let mut waits = 0;
                loop {
                    match self
                        .fetch_once(
                            core,
                            file,
                            object,
                            &mut decryptor,
                            &mut writer,
                            downloaded,
                        )
                        .await
                    {
                        Err(AttemptError::Http(StoreError::RateLimited))
                            if waits < RATE_LIMIT_WAITS =>
                        {
                            tokio::time::sleep(
                                Duration::from_secs(1 << waits)
                                    .min(RATE_LIMIT_MAX_WAIT),
                            )
                            .await;
                            waits += 1;
                        }
                        Err(error)
                            if attempt < RESUME_ATTEMPTS
                                && is_transient(&error) =>
                        {
                            attempt += 1;
                        }
                        result => break result,
                    }
                }
            };
            let (decrypted, sunk) = tokio::join!(decrypt, sink(reader));
            match decrypted {
                Ok(()) => return sunk.map_err(FetchError::Sink),
                // The sink stopped reading on its own; its error is the real one.
                Err(AttemptError::Decrypt(DecryptError::Io(error)))
                    if error.kind() == std::io::ErrorKind::BrokenPipe =>
                {
                    return sunk.map_err(FetchError::Sink);
                }
                Err(AttemptError::Decrypt(DecryptError::WrongContent)) => {
                    // Every holder has the same bytes, so trying another cannot help.
                    faults.push(Fault {
                        core: core.id.clone(),
                        kind: FaultKind::Content,
                    });
                    return Err(FetchError::Broken);
                }
                Err(error) => {
                    tracing::warn!(
                        core = %core.id,
                        %object,
                        error = %error,
                        "Could not fetch a shared file from this Core"
                    );
                    if matches!(
                        error,
                        AttemptError::Decrypt(DecryptError::WrongObject)
                    ) {
                        faults.push(Fault {
                            core: core.id.clone(),
                            kind: FaultKind::Object,
                        });
                    }
                }
            }
        }
        Err(FetchError::Unavailable)
    }

    async fn fetch_once(
        &self,
        core: &StoreHost,
        file: &PoolObject<'_>,
        object: Hash,
        decryptor: &mut Decryptor,
        writer: &mut tokio::io::DuplexStream,
        downloaded: &(dyn Fn(u64) + Sync),
    ) -> Result<(), AttemptError> {
        let from = decryptor.offset();
        let response = self
            .object(core, file.ticket, file.root, object, from)
            .await?;
        if response.content_length().is_some_and(|length| {
            length != file.object_size.saturating_sub(from)
        }) {
            return Err(DecryptError::WrongObject.into());
        }
        let body = tokio_util::io::StreamReader::new(
            response
                .bytes_stream()
                .map_err(std::io::Error::other)
                .inspect_ok(|chunk| downloaded(chunk.len() as u64)),
        );
        Ok(decryptor.run(body, writer).await?)
    }
}

fn is_transient(error: &AttemptError) -> bool {
    match error {
        AttemptError::Decrypt(DecryptError::Io(error)) => {
            error.kind() != std::io::ErrorKind::BrokenPipe
        }
        AttemptError::Http(StoreError::Http(_)) => true,
        _ => false,
    }
}

fn remote<'a>(
    http: &'a reqwest::Client,
    address: &'a str,
    ticket: &'a str,
) -> Remote<'a> {
    Remote {
        http,
        base: address,
        bearer: None,
        read_ticket: Some(ticket),
        timeout: None,
    }
}

const TAR_BLOCK: u64 = 512;
static TAR_END: [u8; 1024] = [0; 1024];

fn tar_header(name: &str, size: u64) -> std::io::Result<Bytes> {
    let mut header = tar::Header::new_ustar();
    header.set_path(name)?;
    header.set_size(size);
    header.set_mode(0o644);
    header.set_mtime(0);
    header.set_entry_type(tar::EntryType::Regular);
    header.set_cksum();
    Ok(Bytes::copy_from_slice(header.as_bytes()))
}

fn tar_padding(size: u64) -> Bytes {
    Bytes::from(vec![
        0;
        ((TAR_BLOCK - size % TAR_BLOCK) % TAR_BLOCK) as usize
    ])
}

/// An uncompressed ustar stream: `version.json` first, then each object named by its C.
fn tar_body(
    list: Vec<u8>,
    entries: Vec<(String, PathBuf, u64)>,
    uploaded: Arc<AtomicU64>,
) -> impl Stream<Item = std::io::Result<Bytes>> + Send + 'static {
    fn once(
        bytes: std::io::Result<Bytes>,
    ) -> BoxStream<'static, std::io::Result<Bytes>> {
        futures::stream::once(futures::future::ready(bytes)).boxed()
    }

    let list_size = list.len() as u64;
    let mut parts = vec![
        once(tar_header("version.json", list_size)),
        once(Ok(Bytes::from(list))),
        once(Ok(tar_padding(list_size))),
    ];
    for (hash, path, size) in entries {
        let uploaded = Arc::clone(&uploaded);
        parts.push(once(tar_header(&hash, size)));
        parts.push(
            futures::stream::once(tokio::fs::File::open(path))
                .map_ok(move |file| {
                    tokio_util::io::ReaderStream::with_capacity(
                        file.take(size),
                        64 * 1024,
                    )
                })
                .try_flatten()
                .inspect_ok(move |chunk| {
                    uploaded.fetch_add(chunk.len() as u64, Ordering::Relaxed);
                })
                .boxed(),
        );
        parts.push(once(Ok(tar_padding(size))));
    }
    parts.push(once(Ok(Bytes::from_static(&TAR_END))));
    futures::stream::iter(parts).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[tokio::test]
    async fn tar_body_round_trips_names_and_sizes() {
        let directory = tempfile::tempdir().unwrap();
        let objects = [("a".repeat(64), 0_usize), ("b".repeat(64), 513)];
        let mut entries = Vec::new();
        for (hash, size) in &objects {
            let path = directory.path().join(hash);
            tokio::fs::write(&path, vec![1; *size]).await.unwrap();
            entries.push((hash.clone(), path, *size as u64));
        }
        let list = br#"{"objects":[]}"#.to_vec();
        let uploaded = Arc::new(AtomicU64::new(0));
        let body = tar_body(list.clone(), entries, Arc::clone(&uploaded))
            .try_fold(Vec::new(), |mut body, chunk| async move {
                body.extend_from_slice(&chunk);
                Ok(body)
            })
            .await
            .unwrap();
        assert_eq!(uploaded.load(Ordering::Relaxed), 513);

        let mut archive = tar::Archive::new(&body[..]);
        let mut read = Vec::new();
        for entry in archive.entries().unwrap() {
            let mut entry = entry.unwrap();
            let name = entry.path().unwrap().to_string_lossy().into_owned();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            read.push((name, bytes));
        }
        assert_eq!(read.len(), 3);
        assert_eq!(read[0], ("version.json".to_string(), list));
        assert_eq!(read[1], (objects[0].0.clone(), Vec::new()));
        assert_eq!(read[2], (objects[1].0.clone(), vec![1; 513]));
    }
}
