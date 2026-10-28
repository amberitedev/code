//! Copying a version from another Core, as a pull ticket asks. The store reaches only the Core
//! the backend named, at a public address, over TLS pinned to that Core's certificate, so it
//! cannot be turned into a general downloader.

use std::{net::IpAddr, sync::Arc, time::Duration};

use futures::TryStreamExt;
use rustls::{
    ClientConfig, DigitallySignedStruct, Error, SignatureScheme,
    client::danger::{
        HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
    },
    crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature},
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use sha2::{Digest, Sha256};
use tokio_util::io::StreamReader;
use tracing::info;

use super::{
    Code, Store, StoreError,
    objects::{self, Hash, VersionList, WriteError},
    tickets::Source,
};

/// The most a version list may weigh.
const MAX_LIST: usize = 4 * 1024 * 1024;

fn failed(description: impl std::fmt::Display) -> StoreError {
    StoreError::new(
        Code::Conflict,
        format!("Could not copy from the other Core: {description}"),
    )
}

/// Copy `root` from `source`. True when the version is now stored here whole.
pub async fn pull(
    store: &Store,
    root: &Hash,
    source: &Source,
) -> Result<bool, StoreError> {
    let ticket = source.ticket().expect("verify requires a source ticket");
    let base = address(source.address(), store.allow_private_sources)?;
    let client = pinned(&source.core.cert_fingerprint)?;
    let get = |path: String| {
        client
            .get(base.join(&path).expect("a valid path"))
            .bearer_auth(ticket)
            .send()
    };

    let response = get(format!("/store/versions/{root}"))
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(failed)?;
    let mut body = Vec::new();
    let mut chunks = response.bytes_stream();
    while let Some(chunk) = chunks.try_next().await.map_err(failed)? {
        body.extend_from_slice(&chunk);
        if body.len() > MAX_LIST {
            return Err(failed("its version list is too large"));
        }
    }
    let list: VersionList = serde_json::from_slice(&body)
        .map_err(|_| failed("its version list is unreadable"))?;
    let missing = store.begin(root, list.clone(), None).await?;
    info!(%root, objects = missing.len(), "pulling a version");
    for hash in missing {
        let size = list.size_of(&hash).expect("missing objects are listed");
        let response = get(format!("/store/versions/{root}/objects/{hash}"))
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(failed)?;
        let reader = StreamReader::new(
            response.bytes_stream().map_err(std::io::Error::other),
        );
        objects::write_object(&store.layout, &hash, size, reader)
            .await
            .map_err(|error| match error {
                WriteError::Mismatch(description) => failed(description),
                WriteError::Io(error) => failed(error),
            })?;
    }
    Ok(store.finish(root).await?.is_empty())
}

/// `https://ip:port`, refused unless the IP is public or `allow_private` is set.
pub(crate) fn address(
    address: &str,
    allow_private: bool,
) -> Result<url::Url, StoreError> {
    let invalid = |description: &str| {
        StoreError::new(Code::InvalidRequest, description.to_owned())
    };
    let url = url::Url::parse(address)
        .map_err(|_| invalid("The source Core's address is unreadable."))?;
    let ip: IpAddr = match url.host() {
        Some(url::Host::Ipv4(ip)) => ip.into(),
        Some(url::Host::Ipv6(ip)) => ip.into(),
        _ => return Err(invalid("The source Core's address must be an IP.")),
    };
    if url.scheme() != "https" {
        return Err(invalid("The source Core's address must be https."));
    }
    if !allow_private && !crate::content::hosts::is_public(ip) {
        return Err(invalid("The source Core's address is not public."));
    }
    Ok(url)
}

/// A client that trusts exactly the certificate whose SHA-256 is `fingerprint`, follows no
/// redirect, and uses no proxy.
pub(crate) fn pinned(fingerprint: &str) -> Result<reqwest::Client, StoreError> {
    let fingerprint: [u8; 32] = hex::decode(fingerprint)
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| {
            StoreError::new(
                Code::InvalidRequest,
                "The source Core's fingerprint is unreadable.",
            )
        })?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ClientConfig::builder_with_provider(Arc::clone(&provider))
        .with_safe_default_protocol_versions()
        .map_err(failed)?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pinned {
            fingerprint,
            provider,
        }))
        .with_no_client_auth();
    reqwest::Client::builder()
        .use_preconfigured_tls(config)
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(60))
        .user_agent(concat!("copal-store/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(failed)
}

/// Accepts exactly one certificate: the one whose SHA-256 is `fingerprint`. The handshake
/// signature is still checked, so the other side must hold that certificate's key.
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
    ) -> Result<ServerCertVerified, Error> {
        if Sha256::digest(end_entity)[..] == self.fingerprint {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(Error::General(
                "this is not the certificate of the Core the ticket names"
                    .into(),
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
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
    ) -> Result<HandshakeSignatureValid, Error> {
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
    use axum::{body::Body, http::Method};
    use serde_json::json;

    use super::*;
    use crate::{
        daemon::paths::Paths,
        network::tls::Identity,
        store::routes::tests::{Fixture, fixture},
    };

    #[test]
    fn only_public_https_addresses_are_sources() {
        assert!(
            address("https://203.0.113.1:16662", false).is_err(),
            "documentation range"
        );
        assert!(address("https://8.8.8.8:16662", false).is_ok());
        for refused in [
            "https://127.0.0.1:16662",
            "https://192.168.1.2:16662",
            "https://169.254.1.1:16662",
            "https://[::1]:16662",
            "https://example.com:16662",
            "http://8.8.8.8:16662",
        ] {
            assert!(address(refused, false).is_err(), "{refused}");
        }
        assert!(address("https://127.0.0.1:16662", true).is_ok());
    }

    /// `source` served over TLS on localhost, as a Core would. Returns its address and
    /// certificate fingerprint.
    async fn serve(source: &Fixture) -> (String, String) {
        let identity_dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(identity_dir.path());
        std::fs::create_dir_all(paths.tls_cert().parent().unwrap()).unwrap();
        let identity = Identity::load_or_create(&paths).unwrap();
        let fingerprint = identity.fingerprint.clone();
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("https://{}", listener.local_addr().unwrap());
        let router = source.router.clone();
        tokio::spawn(async move {
            let _keep = identity_dir;
            crate::network::tls::serve(listener, router, &identity).await;
        });
        (address, fingerprint)
    }

    #[tokio::test]
    async fn a_pull_copies_from_the_pinned_source_only() {
        let source = fixture("source", 1 << 30, 1 << 30).await;
        let list = objects::list_of(&[b"one", b"two"]);
        let (_, answer) = source.upload(&list, &[b"one", b"two"], 100).await;
        assert_eq!(answer["complete"], true);
        let root = list.root();
        let (address, fingerprint) = serve(&source).await;
        let fetch = source.ticket("fetch", &root, json!({}));
        // The replica trusts the same backend as the source.
        let replica = fixture("core", 1 << 30, 1 << 30).await;
        replica.store.keys.set(&source.signer.public_keys()).await;
        let pull = |fingerprint: &str| {
            crate::store::tickets::test_support::ticket(
                &source.signer,
                "pull",
                json!("core"),
                root.as_str(),
                crate::store::now(),
                json!({ "source": {
                    "core": { "id": "source", "address": address, "public_address": null, "cert_fingerprint": fingerprint },
                    "ticket": fetch,
                } }),
            )
        };
        let uri = format!("/store/versions/{root}/pull");

        let (status, error) = replica
            .json(Method::POST, &uri, &pull(&"0".repeat(64)), Body::empty())
            .await;
        assert_eq!(status, axum::http::StatusCode::CONFLICT, "{error}");
        assert!(!replica.store.is_stored(&root));

        let (status, answer) = replica
            .json(Method::POST, &uri, &pull(&fingerprint), Body::empty())
            .await;
        assert_eq!(status, axum::http::StatusCode::OK, "{answer}");
        assert_eq!(answer, json!({ "complete": true }));
        assert!(replica.store.is_stored(&root));
        for entry in &list.objects {
            assert!(replica.store.layout.object(&entry.hash).exists());
        }
    }
}
