//! The client for a Core's store routes (`/store/` in the Protocol of
//! `docs/internals/pool.md`). A Core serves HTTPS with a certificate it made itself, so each Core
//! gets a client that trusts only the certificate whose SHA-256 the backend handed out.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use bytes::Bytes;
use futures::stream::BoxStream;
use futures::{Stream, StreamExt, TryStreamExt};
use reqwest::StatusCode;
use rustls::client::danger::{
    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
};
use rustls::crypto::{
    CryptoProvider, verify_tls12_signature, verify_tls13_signature,
};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;

/// A Core as the backend names it. `address` is tried first and `public_address` when the first
/// does not connect, so a Core on the same network is reached directly.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoolCore {
    pub id: String,
    pub address: String,
    #[serde(default)]
    pub public_address: Option<String>,
    pub cert_fingerprint: String,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("the Core's storage is full")]
    DiskFull,
    #[error("the Core has used up today's transfer allowance")]
    EgressSpent,
    #[error("the Core is limiting requests")]
    RateLimited,
    #[error("the Core refused the ticket ({0})")]
    Ticket(String),
    #[error("the Core does not have this")]
    NotFound,
    #[error("the Core answered {0}: {1}")]
    Other(u16, String),
    #[error("could not reach the Core: {0}")]
    Http(#[from] reqwest::Error),
}

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// One client per pinned certificate, so connections are reused.
static CLIENTS: LazyLock<Mutex<HashMap<[u8; 32], reqwest::Client>>> =
    LazyLock::new(Default::default);
/// The address that last connected, per Core id, for the life of the process.
static ADDRESSES: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(Default::default);

fn client(core: &PoolCore) -> Result<reqwest::Client, StoreError> {
    let mut fingerprint = [0; 32];
    hex::decode_to_slice(&core.cert_fingerprint, &mut fingerprint).map_err(
        |_| StoreError::Other(0, "the Core's fingerprint is invalid".into()),
    )?;
    let mut clients = CLIENTS.lock().expect("store clients lock poisoned");
    if let Some(client) = clients.get(&fingerprint) {
        return Ok(client.clone());
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config =
        rustls::ClientConfig::builder_with_provider(Arc::clone(&provider))
            .with_safe_default_protocol_versions()
            .map_err(|error| StoreError::Other(0, error.to_string()))?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(Pinned {
                fingerprint,
                provider,
            }))
            .with_no_client_auth();
    // No overall timeout: an object can take minutes on a slow uplink.
    let client = reqwest::Client::builder()
        .use_preconfigured_tls(config)
        .connect_timeout(CONNECT_TIMEOUT)
        .tcp_keepalive(Some(Duration::from_secs(10)))
        .user_agent(crate::launcher_user_agent())
        .build()?;
    clients.insert(fingerprint, client.clone());
    Ok(client)
}

/// The addresses to try, the one that last worked first.
fn addresses(core: &PoolCore) -> Vec<String> {
    let remembered = ADDRESSES
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
        let address = address.trim_end_matches('/').to_string();
        if !address.is_empty() && !addresses.contains(&address) {
            addresses.push(address);
        }
    }
    addresses
}

fn remember(core: &PoolCore, address: &str) {
    ADDRESSES
        .lock()
        .expect("store addresses lock poisoned")
        .insert(core.id.clone(), address.to_string());
}

/// Sends a request that can be rebuilt, moving to the next address when one does not connect.
async fn send(
    core: &PoolCore,
    request: impl Fn(&reqwest::Client, &str) -> reqwest::RequestBuilder,
) -> Result<reqwest::Response, StoreError> {
    let client = client(core)?;
    let mut unreachable = None;
    for address in addresses(core) {
        match request(&client, &address).send().await {
            Ok(response) => {
                remember(core, &address);
                return check(response).await;
            }
            Err(error) if error.is_connect() => unreachable = Some(error),
            Err(error) => return Err(error.into()),
        }
    }
    Err(unreachable.map_or_else(
        || StoreError::Other(0, "the Core has no address".into()),
        StoreError::Http,
    ))
}

async fn check(
    response: reqwest::Response,
) -> Result<reqwest::Response, StoreError> {
    if response.status().is_success() {
        return Ok(response);
    }
    #[derive(Deserialize)]
    struct Body {
        error: String,
        #[serde(default)]
        description: String,
    }
    let status = response.status().as_u16();
    let text = response.text().await.unwrap_or_default();
    let Ok(body) = serde_json::from_str::<Body>(&text) else {
        return Err(StoreError::Other(status, text));
    };
    Err(match body.error.as_str() {
        "disk_full" => StoreError::DiskFull,
        "egress_spent" => StoreError::EgressSpent,
        "rate_limited" => StoreError::RateLimited,
        "not_found" => StoreError::NotFound,
        error if error.starts_with("ticket_") => StoreError::Ticket(body.error),
        _ => StoreError::Other(status, body.description),
    })
}

#[derive(Debug, Deserialize)]
pub struct Have {
    #[serde(default)]
    pub missing: Vec<String>,
    /// Whether the version's list is already stored, which means nothing is left to send.
    #[serde(default)]
    pub complete: bool,
}

/// Which of `objects` the Core lacks for the version `root`.
pub async fn have(
    core: &PoolCore,
    ticket: &str,
    root: &str,
    objects: &[String],
) -> Result<Have, StoreError> {
    let body = serde_json::json!({ "objects": objects });
    Ok(send(core, |client, address| {
        client
            .post(format!("{address}/store/versions/{root}/have"))
            .bearer_auth(ticket)
            .json(&body)
    })
    .await?
    .json()
    .await?)
}

/// One entry of a version's list, sorted by hash.
#[derive(Clone, Debug, Serialize)]
pub struct ListedObject {
    pub hash: String,
    pub size: u64,
}

#[derive(Debug, Deserialize)]
pub struct UploadOutcome {
    pub complete: bool,
    #[serde(default)]
    pub missing: Vec<String>,
}

/// Streams the version's list and the given objects to the Core as one tar. `objects` are
/// `(C, object file)` pairs; their sizes come from `list`. `uploaded` counts object bytes sent.
pub async fn upload(
    core: &PoolCore,
    ticket: &str,
    root: &str,
    list: &[ListedObject],
    objects: impl IntoIterator<Item = (String, PathBuf)>,
    uploaded: Arc<AtomicU64>,
) -> Result<UploadOutcome, StoreError> {
    let sizes = list
        .iter()
        .map(|object| (object.hash.as_str(), object.size))
        .collect::<HashMap<_, _>>();
    let entries = objects
        .into_iter()
        .filter_map(|(hash, path)| {
            let size = *sizes.get(hash.as_str())?;
            Some((hash, path, size))
        })
        .collect::<Vec<_>>();
    let list = serde_json::to_vec(&serde_json::json!({ "objects": list }))
        .map_err(|error| StoreError::Other(0, error.to_string()))?;
    let body = tar_body(list, entries, uploaded);
    // The body streams once, so it goes to the address `have` just reached.
    let address = addresses(core).into_iter().next().unwrap_or_default();
    let response = client(core)?
        .post(format!("{address}/store/versions/{root}"))
        .bearer_auth(ticket)
        .header(reqwest::header::CONTENT_TYPE, "application/x-tar")
        .body(reqwest::Body::wrap_stream(body))
        .send()
        .await?;
    Ok(check(response).await?.json().await?)
}

/// The object `c` of version `root`, from byte `from` on.
pub async fn fetch(
    core: &PoolCore,
    ticket: &str,
    root: &str,
    c: &str,
    from: u64,
) -> Result<reqwest::Response, StoreError> {
    let response = send(core, |client, address| {
        let request = client
            .get(format!("{address}/store/versions/{root}/objects/{c}"))
            .bearer_auth(ticket);
        if from > 0 {
            request.header(reqwest::header::RANGE, format!("bytes={from}-"))
        } else {
            request
        }
    })
    .await?;
    if from > 0 && response.status() != StatusCode::PARTIAL_CONTENT {
        return Err(StoreError::Other(
            response.status().as_u16(),
            "the Core ignored the requested range".into(),
        ));
    }
    Ok(response)
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

/// Accepts exactly one certificate: the one whose SHA-256 is `fingerprint`. The handshake
/// signature is still checked, so the Core must hold that certificate's key.
#[derive(Debug)]
struct Pinned {
    fingerprint: [u8; 32],
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if Sha256::digest(end_entity)[..] == self.fingerprint {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General(
                "this is not the certificate the backend named for this Core"
                    .into(),
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
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

    #[test]
    fn only_the_pinned_certificate_is_accepted() {
        let certificate = CertificateDer::from(b"the Core".to_vec());
        let pinned = Pinned {
            fingerprint: Sha256::digest(&certificate).into(),
            provider: Arc::new(rustls::crypto::ring::default_provider()),
        };
        let verify = |certificate: &CertificateDer<'_>| {
            pinned.verify_server_cert(
                certificate,
                &[],
                &ServerName::try_from("core").unwrap(),
                &[],
                UnixTime::now(),
            )
        };
        assert!(verify(&certificate).is_ok());
        assert!(verify(&CertificateDer::from(b"another".to_vec())).is_err());
    }
}
