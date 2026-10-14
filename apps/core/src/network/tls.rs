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

#[cfg(test)]
mod tests {
    use super::*;

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
