//! HTTPS for the Core API with a certificate the Core makes itself, and the limits of the one
//! listener the internet can reach.
//!
//! There is no domain and no certificate authority. The App learns the certificate's SHA-256
//! fingerprint through the backend and trusts only that. There is no plain HTTP listener; the CLI
//! reaches Core over the local socket (see `access::local`).

use std::{net::IpAddr, path::Path, sync::Arc, time::Duration};

use axum::{
    Extension, Router,
    extract::Request,
    http::header::{HOST, ORIGIN},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use color_eyre::eyre::{Result, WrapErr, eyre};
use rustls::{
    ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
};
use sha2::{Digest, Sha256};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Semaphore,
};
use tokio_rustls::TlsAcceptor;
use tracing::debug;

use crate::{
    access::Peer,
    api::error::{ApiError, ErrorCode},
    daemon::paths::Paths,
};

/// How long a new connection may take to finish the TLS handshake.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// Open connections at once. More wait to be accepted.
const MAX_CONNECTIONS: usize = 256;

/// This Core's certificate: self-signed, made once on first start and never renewed.
pub struct Identity {
    config: Arc<ServerConfig>,
    /// SHA-256 of the certificate, lowercase hex. This is what the App pins.
    pub fingerprint: String,
}

impl Identity {
    /// Load the certificate from `db/`, making it only when neither file exists. Every App pins
    /// this certificate, so anything else that goes wrong is an error, never a new identity.
    pub fn load_or_create(paths: &Paths) -> Result<Self> {
        let cert_path = paths.tls_cert();
        let key_path = paths.tls_key();
        let (cert, key) = match (read(&cert_path)?, read(&key_path)?) {
            (Some(cert), Some(key)) => (cert, key),
            (None, None) => {
                let made = rcgen::generate_simple_self_signed(vec![
                    "amberite-core".to_string(),
                ])
                .wrap_err("could not make the Core certificate")?;
                let cert = made.cert.der().to_vec();
                let key = made.signing_key.serialize_der();
                // Key first: a crash between the writes leaves no certificate without its key.
                write_private(&key_path, &key).wrap_err_with(|| {
                    format!("could not write {}", key_path.display())
                })?;
                std::fs::write(&cert_path, &cert).wrap_err_with(|| {
                    format!("could not write {}", cert_path.display())
                })?;
                (cert, key)
            }
            (Some(_), None) => return Err(half_identity(&key_path)),
            (None, Some(_)) => return Err(half_identity(&cert_path)),
        };
        let fingerprint = hex::encode(Sha256::digest(&cert));
        let config = ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|error| eyre!("TLS setup failed: {error}"))?
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(cert)],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key)),
        )
        .map_err(|error| {
            eyre!("the stored Core certificate is unusable: {error}")
        })?;
        Ok(Self {
            config: Arc::new(config),
            fingerprint,
        })
    }
}

/// The file's contents, or `None` when it does not exist.
fn read(path: &Path) -> Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error)
            .wrap_err_with(|| format!("could not read {}", path.display())),
    }
}

fn half_identity(missing: &Path) -> color_eyre::Report {
    eyre!(
        "{} is missing but the rest of Core's TLS identity is there. Restore it from a backup; \
         making a new identity would break every App that trusts this Core",
        missing.display()
    )
}

/// Write a file only its owner may read. Windows keeps the data directory's own permissions.
fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    std::io::Write::write_all(&mut options.open(path)?, contents)
}

/// Serve `router` over HTTPS on `listener` until the process ends.
pub async fn serve(listener: TcpListener, router: Router, identity: &Identity) {
    let acceptor = TlsAcceptor::from(Arc::clone(&identity.config));
    let router = router.layer(middleware::from_fn(guard));
    let open = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    loop {
        let permit = Arc::clone(&open)
            .acquire_owned()
            .await
            .expect("never closed");
        let Ok((stream, peer)) = listener.accept().await else {
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        };
        let acceptor = acceptor.clone();
        let router = router.clone().layer(Extension(Peer(peer.ip())));
        tokio::spawn(async move {
            if let Err(error) = connection(stream, acceptor, router).await {
                debug!(%peer, %error, "connection closed");
            }
            drop(permit);
        });
    }
}

/// Refuse what only a browser or a DNS rebinding attack sends: an `Origin` header, or a `Host`
/// that is a name instead of this Core's address. The App's Rust side sends neither.
async fn guard(request: Request, next: Next) -> Response {
    if request.headers().contains_key(ORIGIN) {
        return ApiError::new(
            ErrorCode::Forbidden,
            "Browsers cannot call a Core.",
        )
        .into_response();
    }
    let host = request
        .headers()
        .get(HOST)
        .and_then(|host| host.to_str().ok())
        .or_else(|| request.uri().host());
    if !host.is_some_and(host_allowed) {
        return ApiError::new(
            ErrorCode::InvalidRequest,
            "Reach this Core by its IP address.",
        )
        .into_response();
    }
    next.run(request).await
}

/// An IP address or `localhost`, with or without a port.
fn host_allowed(host: &str) -> bool {
    let name = match host.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or_default(),
        None => host.rsplit_once(':').map_or(host, |(name, _)| name),
    };
    name.eq_ignore_ascii_case("localhost") || name.parse::<IpAddr>().is_ok()
}

async fn connection(
    stream: TcpStream,
    acceptor: TlsAcceptor,
    router: Router,
) -> Result<()> {
    let stream =
        tokio::time::timeout(HANDSHAKE_TIMEOUT, acceptor.accept(stream))
            .await??;
    crate::api::serve_connection(stream, router).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_addresses_are_allowed_hosts() {
        for host in [
            "192.168.1.2:16662",
            "203.0.113.5",
            "[::1]:16662",
            "localhost:1",
        ] {
            assert!(host_allowed(host), "{host}");
        }
        for host in [
            "evil.example:16662",
            "evil.example",
            "192.168.1.2.sslip.io",
            "",
        ] {
            assert!(!host_allowed(host), "{host}");
        }
    }

    #[tokio::test]
    async fn browsers_and_names_are_refused() {
        use axum::{body::Body, routing::get};
        use tower::ServiceExt;

        let router = Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(middleware::from_fn(guard));
        let status = |headers: &[(&str, &str)]| {
            let mut request = Request::builder().uri("/");
            for (name, value) in headers {
                request = request.header(*name, *value);
            }
            let router = router.clone();
            async move {
                router
                    .oneshot(request.body(Body::empty()).unwrap())
                    .await
                    .unwrap()
                    .status()
                    .as_u16()
            }
        };
        assert_eq!(status(&[("host", "192.168.1.2:16662")]).await, 200);
        assert_eq!(status(&[("host", "evil.example")]).await, 400);
        assert_eq!(
            status(&[
                ("host", "192.168.1.2"),
                ("origin", "https://evil.example")
            ])
            .await,
            403
        );
    }

    fn paths() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        std::fs::create_dir_all(paths.tls_cert().parent().unwrap()).unwrap();
        (dir, paths)
    }

    #[test]
    fn the_identity_is_made_once_and_kept() {
        let (_dir, paths) = paths();
        let first = Identity::load_or_create(&paths).unwrap();
        let again = Identity::load_or_create(&paths).unwrap();
        assert_eq!(first.fingerprint, again.fingerprint);
    }

    #[test]
    fn half_an_identity_is_an_error_not_a_new_one() {
        let (_dir, paths) = paths();
        Identity::load_or_create(&paths).unwrap();
        let cert = std::fs::read(paths.tls_cert()).unwrap();
        std::fs::remove_file(paths.tls_key()).unwrap();
        let error = Identity::load_or_create(&paths).err().unwrap();
        assert!(error.to_string().contains("tls-key.der"));
        assert_eq!(std::fs::read(paths.tls_cert()).unwrap(), cert);
        assert!(!paths.tls_key().exists());
    }
}
