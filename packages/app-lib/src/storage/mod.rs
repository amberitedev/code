//! Private sharing storage, independent of launcher state and Minecraft processes.
//! The coordinator grants single-file capabilities; account sessions never reach a node.

use actix_web::{App, HttpRequest, HttpResponse, HttpServer, error, web};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

const MAX_FILE_SIZE: u64 = 500 * 1024 * 1024;

pub struct Configuration {
    pub id: String,
    pub directory: PathBuf,
    pub address: SocketAddr,
    pub secret: String,
    pub backend_url: String,
}

impl Configuration {
    pub fn from_env() -> eyre::Result<Self> {
        let config = Self {
            id: std::env::var("SHARING_STORAGE_ID")?,
            directory: std::env::var("SHARING_STORAGE_DIR")?.into(),
            address: std::env::var("SHARING_STORAGE_ADDR")?.parse()?,
            secret: std::env::var("SHARING_STORAGE_SECRET")?,
            backend_url: std::env::var("SHARING_BACKEND_URL")?
                .trim_end_matches('/')
                .to_owned(),
        };
        eyre::ensure!(
            config.address.ip().is_loopback(),
            "Storage currently supports localhost only"
        );
        eyre::ensure!(
            config.secret.len() >= 32,
            "Storage secret must be at least 32 characters"
        );
        eyre::ensure!(!config.id.is_empty(), "Storage node ID is required");
        let backend = url::Url::parse(&config.backend_url)?;
        eyre::ensure!(
            is_loopback_url(&backend),
            "Backend must use localhost for this milestone"
        );
        Ok(config)
    }
}

struct Service {
    config: Configuration,
    client: reqwest::Client,
    transfers: tokio::sync::Semaphore,
}

#[derive(Clone, Deserialize, Serialize)]
struct Transfer {
    op: String,
    node_id: String,
    file_id: String,
    sha256: Option<String>,
    size: u64,
    expires: u64,
    receipt_url: Option<String>,
    receipt_token: Option<String>,
}

#[derive(Deserialize)]
struct CapabilityQuery {
    cap: String,
}

#[derive(Deserialize)]
struct Jobs {
    jobs: Vec<PullJob>,
}

#[derive(Deserialize)]
struct PullJob {
    source_url: String,
    upload_url: String,
    sha256: String,
    size: u64,
}

fn is_loopback_url(url: &url::Url) -> bool {
    url.scheme() == "http"
        && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn verify(
    config: &Configuration,
    token: &str,
    operation: &str,
) -> eyre::Result<Transfer> {
    let (payload, signature) = token
        .split_once('.')
        .ok_or_else(|| eyre::eyre!("Invalid capability"))?;
    let mut mac = Hmac::<Sha256>::new_from_slice(config.secret.as_bytes())?;
    mac.update(payload.as_bytes());
    mac.verify_slice(&URL_SAFE_NO_PAD.decode(signature)?)?;
    let transfer: Transfer =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload)?)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis();
    eyre::ensure!(
        transfer.op == operation
            && transfer.node_id == config.id
            && u128::from(transfer.expires) > now,
        "Capability is expired or has the wrong scope"
    );
    eyre::ensure!(
        transfer.size <= MAX_FILE_SIZE,
        "File exceeds the native sharing limit"
    );
    if let Some(hash) = transfer.sha256.as_deref() {
        eyre::ensure!(valid_hash(hash), "Invalid content digest");
    }
    Ok(transfer)
}

/// Hash the committed object before serving it, so disk corruption cannot pass as a successful download.
async fn inspect(
    service: &Service,
    hash: &str,
    size: u64,
) -> eyre::Result<tokio::fs::File> {
    eyre::ensure!(valid_hash(hash), "Invalid digest");
    let mut file = tokio::fs::File::open(
        service.config.directory.join("objects").join(hash),
    )
    .await?;
    eyre::ensure!(
        file.metadata().await?.len() == size,
        "Stored content has the wrong length"
    );
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let length = file.read(&mut buffer).await?;
        if length == 0 {
            break;
        }
        digest.update(&buffer[..length]);
    }
    eyre::ensure!(
        format!("{:x}", digest.finalize()) == hash,
        "Stored content failed verification"
    );
    file.rewind().await?;
    Ok(file)
}

/// A temporary file is never visible to downloads. The verified file is fsynced before atomic rename.
async fn commit<S, E>(
    service: &Service,
    grant: &Transfer,
    mut body: S,
) -> eyre::Result<String>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
    E: std::fmt::Display,
{
    let temporary = tempfile::NamedTempFile::new_in(
        service.config.directory.join("incoming"),
    )?;
    let (file, path) = temporary.into_parts();
    let mut file = tokio::fs::File::from_std(file);
    let mut size = 0_u64;
    let mut digest = Sha256::new();
    while let Some(bytes) = body.next().await {
        let bytes =
            bytes.map_err(|_| eyre::eyre!("Transfer was interrupted"))?;
        size = size
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| eyre::eyre!("File length overflow"))?;
        eyre::ensure!(
            size <= grant.size,
            "Transfer exceeded its declared size"
        );
        digest.update(&bytes);
        file.write_all(&bytes).await?;
    }
    eyre::ensure!(size == grant.size, "Transfer was incomplete");
    let hash = format!("{:x}", digest.finalize());
    eyre::ensure!(
        grant
            .sha256
            .as_ref()
            .is_none_or(|expected| expected == &hash),
        "Transfer failed SHA256 verification"
    );
    file.sync_all().await?;
    drop(file);
    let target = service.config.directory.join("objects").join(&hash);
    path.persist(&target).map_err(|error| error.error)?;
    #[cfg(unix)]
    std::fs::File::open(service.config.directory.join("objects"))?
        .sync_all()?;
    Ok(hash)
}

async fn receipt(
    service: &Service,
    grant: &Transfer,
    hash: &str,
) -> eyre::Result<()> {
    let target = grant
        .receipt_url
        .as_ref()
        .ok_or_else(|| eyre::eyre!("Missing receipt endpoint"))?;
    let target_url = url::Url::parse(target)?;
    let backend = url::Url::parse(&service.config.backend_url)?;
    eyre::ensure!(
        target_url.origin() == backend.origin()
            && target_url.path() == "/v1/storage/receipt",
        "Receipt endpoint has the wrong origin"
    );
    let response = service
        .client
        .post(target)
        .json(&serde_json::json!({
            "node_id": service.config.id, "file_id": grant.file_id,
            "sha256": hash, "size": grant.size, "receipt": grant.receipt_token,
        }))
        .send()
        .await
        .map_err(reqwest::Error::without_url)?;
    eyre::ensure!(
        response.status().is_success(),
        "Coordinator did not accept the storage receipt ({})",
        response.status()
    );
    Ok(())
}

async fn upload(
    service: web::Data<Arc<Service>>,
    request: HttpRequest,
    query: web::Query<CapabilityQuery>,
    body: web::Payload,
) -> actix_web::Result<HttpResponse> {
    let grant = verify(&service.config, &query.cap, "upload")
        .map_err(|_| error::ErrorUnauthorized("Invalid upload capability"))?;
    if request.match_info().get("id") != Some(grant.file_id.as_str()) {
        return Err(error::ErrorUnauthorized(
            "Capability has the wrong file scope",
        ));
    }
    let _permit = service
        .transfers
        .acquire()
        .await
        .map_err(error::ErrorInternalServerError)?;
    let hash = commit(&service, &grant, body)
        .await
        .map_err(|cause| error::ErrorBadRequest(cause.to_string()))?;
    receipt(&service, &grant, &hash).await.map_err(|_| {
        error::ErrorServiceUnavailable(
            "Bytes are saved; retry to confirm with the coordinator",
        )
    })?;
    Ok(HttpResponse::NoContent()
        .insert_header(("X-Content-SHA256", hash))
        .finish())
}

fn byte_range(value: &str, size: u64) -> Option<(u64, u64)> {
    let (start, end) = value.strip_prefix("bytes=")?.split_once('-')?;
    if start.is_empty() {
        let suffix: u64 = end.parse().ok()?;
        return (suffix > 0 && size > 0)
            .then_some((size.saturating_sub(suffix), size - 1));
    }
    let start: u64 = start.parse().ok()?;
    let end = if end.is_empty() {
        size.checked_sub(1)?
    } else {
        end.parse::<u64>().ok()?.min(size.checked_sub(1)?)
    };
    (start <= end && start < size).then_some((start, end))
}

async fn download(
    service: web::Data<Arc<Service>>,
    request: HttpRequest,
    query: web::Query<CapabilityQuery>,
) -> actix_web::Result<HttpResponse> {
    let grant = verify(&service.config, &query.cap, "read")
        .map_err(|_| error::ErrorUnauthorized("Invalid download capability"))?;
    let hash = grant
        .sha256
        .as_deref()
        .ok_or_else(|| error::ErrorUnauthorized("Missing content digest"))?;
    if request.match_info().get("hash") != Some(hash) {
        return Err(error::ErrorUnauthorized(
            "Capability has the wrong file scope",
        ));
    }
    let mut file = match inspect(&service, hash, grant.size).await {
        Ok(file) => file,
        Err(_) => return Ok(HttpResponse::NotFound().finish()),
    };
    let etag = format!("\"{hash}\"");
    let range = request
        .headers()
        .get("range")
        .and_then(|value| value.to_str().ok());
    let allow_range = request
        .headers()
        .get("if-range")
        .is_none_or(|value| value.to_str().ok() == Some(&etag));
    let (start, end, partial) =
        if let Some(range) = range.filter(|_| allow_range) {
            let Some((start, end)) = byte_range(range, grant.size) else {
                return Ok(HttpResponse::RangeNotSatisfiable()
                    .insert_header((
                        "Content-Range",
                        format!("bytes */{}", grant.size),
                    ))
                    .finish());
            };
            (start, end, true)
        } else {
            (0, grant.size.saturating_sub(1), false)
        };
    let size = if grant.size == 0 { 0 } else { end - start + 1 };
    let mut response = if partial {
        HttpResponse::PartialContent()
    } else {
        HttpResponse::Ok()
    };
    response
        .insert_header(("Content-Type", "application/octet-stream"))
        .insert_header(("Content-Length", size.to_string()))
        .insert_header(("Accept-Ranges", "bytes"))
        .insert_header(("ETag", etag))
        .insert_header(("X-Content-SHA256", hash))
        .insert_header(("Cache-Control", "private, no-store"));
    if partial {
        response.insert_header((
            "Content-Range",
            format!("bytes {start}-{end}/{}", grant.size),
        ));
    }
    if request.method() == actix_web::http::Method::HEAD {
        // Actix derives Content-Length from the body size, even for HEAD.
        // A sized body preserves object length while the HTTP encoder omits bytes.
        return Ok(response.body(actix_web::body::SizedStream::new(
            size,
            futures::stream::empty::<Result<Bytes, std::io::Error>>(),
        )));
    }
    file.seek(std::io::SeekFrom::Start(start))
        .await
        .map_err(error::ErrorInternalServerError)?;
    Ok(response.streaming(tokio_util::io::ReaderStream::new(file.take(size))))
}

async fn pull(service: &Service, job: PullJob) -> eyre::Result<()> {
    let destination = url::Url::parse(&job.upload_url)?;
    let token = destination
        .query_pairs()
        .find(|(key, _)| key == "cap")
        .map(|(_, value)| value.into_owned())
        .ok_or_else(|| eyre::eyre!("Missing pull capability"))?;
    let grant = verify(&service.config, &token, "upload")?;
    eyre::ensure!(
        grant.sha256.as_deref() == Some(job.sha256.as_str())
            && grant.size == job.size,
        "Pull job has conflicting content metadata"
    );
    if inspect(service, &job.sha256, job.size).await.is_ok() {
        return receipt(service, &grant, &job.sha256).await;
    }
    let source = url::Url::parse(&job.source_url)?;
    eyre::ensure!(
        is_loopback_url(&source),
        "Only local storage sources are supported"
    );
    let response = service
        .client
        .get(source)
        .send()
        .await
        .map_err(reqwest::Error::without_url)?;
    eyre::ensure!(response.status().is_success(), "Source is unavailable");
    let hash = commit(service, &grant, response.bytes_stream()).await?;
    receipt(service, &grant, &hash).await
}

async fn heartbeat(service: Arc<Service>) {
    let mut tick = tokio::time::interval(Duration::from_secs(15));
    loop {
        tick.tick().await;
        let response = service
            .client
            .post(format!(
                "{}/v1/storage/{}/heartbeat",
                service.config.backend_url, service.config.id
            ))
            .bearer_auth(&service.config.secret)
            .send()
            .await;
        let Ok(response) = response else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        let Ok(jobs) = response.json::<Jobs>().await else {
            continue;
        };
        // Background copies do not queue ahead of a user's active upload.
        let Ok(_permit) = service.transfers.try_acquire() else {
            continue;
        };
        if let Some(job) = jobs.jobs.into_iter().next() {
            if let Err(error) = pull(&service, job).await {
                tracing::warn!(%error,"Storage copy will retry on the next heartbeat");
            }
        }
    }
}

/// Run a localhost storage process without initializing a launcher installation or Minecraft server.
pub async fn run(configuration: Configuration) -> eyre::Result<()> {
    tokio::fs::create_dir_all(configuration.directory.join("objects")).await?;
    tokio::fs::create_dir_all(configuration.directory.join("incoming")).await?;
    let address = configuration.address;
    let service = Arc::new(Service {
        config: configuration,
        client: reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(600))
            .redirect(reqwest::redirect::Policy::none())
            .build()?,
        transfers: tokio::sync::Semaphore::new(2),
    });
    let server_state = service.clone();
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(server_state.clone()))
            .route(
                "/health",
                web::get().to(|| async {
                    HttpResponse::Ok().json(serde_json::json!({"status":"ok"}))
                }),
            )
            .route("/v1/uploads/{id}", web::put().to(upload))
            .route("/v1/files/{hash}", web::get().to(download))
            .route("/v1/files/{hash}", web::head().to(download))
    })
    .workers(2)
    .bind(address)?
    .run();
    let background = tokio::spawn(heartbeat(service));
    let result = server.await;
    background.abort();
    result?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn interrupted_or_corrupt_uploads_never_commit_and_retry_is_idempotent()
     {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("incoming")).unwrap();
        std::fs::create_dir(directory.path().join("objects")).unwrap();
        let service = Service {
            config: Configuration {
                id: "test".into(),
                directory: directory.path().to_owned(),
                address: "127.0.0.1:0".parse().unwrap(),
                secret: "test-secret-at-least-thirty-two-characters".into(),
                backend_url: "http://127.0.0.1:1".into(),
            },
            client: reqwest::Client::new(),
            transfers: tokio::sync::Semaphore::new(2),
        };
        let bytes = Bytes::from_static(b"resource pack bytes");
        let digest = format!("{:x}", Sha256::digest(&bytes));
        let grant = Transfer {
            op: "upload".into(),
            node_id: "test".into(),
            file_id: "file".into(),
            sha256: Some(digest.clone()),
            size: bytes.len() as u64,
            expires: u64::MAX,
            receipt_url: None,
            receipt_token: None,
        };
        let interrupted =
            futures::stream::iter([Ok(bytes.slice(..4)), Err("interrupted")]);
        assert!(commit(&service, &grant, interrupted).await.is_err());
        assert_eq!(
            std::fs::read_dir(directory.path().join("objects"))
                .unwrap()
                .count(),
            0
        );
        assert_eq!(
            std::fs::read_dir(directory.path().join("incoming"))
                .unwrap()
                .count(),
            0
        );
        let wrong = futures::stream::iter([Ok::<_, &str>(Bytes::from_static(
            b"resource pack wrong",
        ))]);
        assert!(commit(&service, &grant, wrong).await.is_err());
        for _ in 0..2 {
            assert_eq!(
                commit(
                    &service,
                    &grant,
                    futures::stream::iter([Ok::<_, &str>(bytes.clone())])
                )
                .await
                .unwrap(),
                digest
            );
            assert!(inspect(&service, &digest, grant.size).await.is_ok());
        }
        std::fs::write(
            directory.path().join("objects").join(&digest),
            b"resource pack wrong",
        )
        .unwrap();
        assert!(inspect(&service, &digest, grant.size).await.is_err());
    }

    #[test]
    fn retry_ranges_reject_invalid_or_overflowing_positions() {
        assert_eq!(byte_range("bytes=4-", 10), Some((4, 9)));
        assert_eq!(byte_range("bytes=-3", 10), Some((7, 9)));
        assert_eq!(byte_range("bytes=10-", 10), None);
        assert_eq!(byte_range("bytes=8-3", 10), None);
        assert_eq!(byte_range("bytes=0-18446744073709551616", 10), None);
        assert_eq!(byte_range("bytes=0-1,4-5", 10), None);
    }
}
