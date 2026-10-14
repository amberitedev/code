//! HTTPS for the Core API with a certificate the Core makes itself.
//!
//! There is no domain and no certificate authority. The App learns the certificate's SHA-256
//! fingerprint through the backend and trusts only that. There is no plain HTTP listener; the CLI
//! reaches Core over the local socket (step 2).

use std::{path::Path, sync::Arc, time::Duration};

use axum::Router;
use color_eyre::eyre::{Result, WrapErr, eyre};
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
    service::TowerToHyperService,
};
use rustls::{
    ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
};
use sha2::{Digest, Sha256};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;
use tracing::debug;

use crate::daemon::paths::Paths;

/// How long a new connection may take to finish the TLS handshake.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// This Core's certificate: self-signed, made once on first start and never renewed.
pub struct Identity {
    config: Arc<ServerConfig>,
    /// SHA-256 of the certificate, lowercase hex. This is what the App pins.
    pub fingerprint: String,
}

impl Identity {
    /// Load the certificate from `db/`, making it on first start.
    pub fn load_or_create(paths: &Paths) -> Result<Self> {
        let cert_path = paths.tls_cert();
        let key_path = paths.tls_key();
        let (cert, key) =
            match (std::fs::read(&cert_path), std::fs::read(&key_path)) {
                (Ok(cert), Ok(key)) => (cert, key),
                _ => {
                    let made = rcgen::generate_simple_self_signed(vec![
                        "amberite-core".to_string(),
                    ])
                    .wrap_err("could not make the Core certificate")?;
                    let cert = made.cert.der().to_vec();
                    let key = made.signing_key.serialize_der();
                    write_private(&key_path, &key)?;
                    std::fs::write(&cert_path, &cert)?;
                    (cert, key)
                }
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
    loop {
        let Ok((stream, peer)) = listener.accept().await else {
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        };
        let acceptor = acceptor.clone();
        let router = router.clone();
        tokio::spawn(async move {
            if let Err(error) = connection(stream, acceptor, router).await {
                debug!(%peer, %error, "connection closed");
            }
        });
    }
}

async fn connection(
    stream: TcpStream,
    acceptor: TlsAcceptor,
    router: Router,
) -> Result<()> {
    let stream =
        tokio::time::timeout(HANDSHAKE_TIMEOUT, acceptor.accept(stream))
            .await??;
    Builder::new(TokioExecutor::new())
        .serve_connection_with_upgrades(
            TokioIo::new(stream),
            TowerToHyperService::new(router),
        )
        .await
        .map_err(|error| eyre!("{error}"))
}
