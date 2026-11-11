//! Copying a version from another Core, as a pull ticket asks. The store reaches only the Core
//! the backend named, at a public address, through `sharing_client`'s store client: TLS pinned
//! to that Core's certificate, no proxy and no redirect, so it cannot be turned into a general
//! downloader.

use sharing_client::net::{CoreAddress, ip};
use sharing_client::pool::PoolCore;
use tracing::info;

use crate::{
    Code, Store, StoreError,
    objects::{self, Hash, VersionList, WriteError},
    tickets::Source,
};

fn failed(description: impl std::fmt::Display) -> StoreError {
    StoreError::new(
        Code::Conflict,
        format!("Could not copy from the other Core: {description}"),
    )
}

/// Copy `root` from `source`. True when the version is now stored here whole.
pub(crate) async fn pull(
    store: &Store,
    root: &Hash,
    source: &Source,
) -> Result<bool, StoreError> {
    let ticket = source.ticket().expect("verify requires a source ticket");
    let core = PoolCore {
        // The certificate names the Core: the backend gives no id with a source.
        id: source.core.cert_fingerprint.clone(),
        address: address(source.address(), store.allow_private_sources)?,
        public_address: None,
        cert_fingerprint: source.core.cert_fingerprint.clone(),
    };
    let root_hex = root.to_string();
    let body = store
        .client
        .version_list(&core, ticket, &root_hex)
        .await
        .map_err(failed)?;
    let list: VersionList = serde_json::from_slice(&body)
        .map_err(|_| failed("its version list is unreadable"))?;
    let missing = store.begin(root, list.clone(), None).await?;
    info!(%root, objects = missing.len(), "pulling a version");
    for hash in missing {
        let size = list.size_of(&hash).expect("missing objects are listed");
        let reader = store
            .client
            .object_reader(&core, ticket, &root_hex, &hash.to_string())
            .await
            .map_err(failed)?;
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
pub fn address(
    address: &str,
    allow_private: bool,
) -> Result<String, StoreError> {
    let invalid = |description: &str| {
        StoreError::new(Code::InvalidRequest, description.to_owned())
    };
    let address = CoreAddress::parse(address).map_err(|_| {
        invalid("The source Core's address must be https://ip:port.")
    })?;
    if !allow_private && !ip::is_public(address.socket().ip()) {
        return Err(invalid("The source Core's address is not public."));
    }
    Ok(address.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{body::Body, http::Method};
    use hyper_util::{
        rt::{TokioExecutor, TokioIo},
        server::conn::auto::Builder,
        service::TowerToHyperService,
    };
    use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use tokio_rustls::TlsAcceptor;

    use super::*;
    use crate::{
        routes::tests::{Fixture, fixture},
        tickets::test_support::ticket,
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

    /// `source` served over TLS on localhost with a self-signed certificate, as a Core serves
    /// it. Returns its address and certificate fingerprint.
    async fn serve(source: &Fixture) -> (String, String) {
        let made =
            rcgen::generate_simple_self_signed(vec!["amberite-core".into()])
                .unwrap();
        let fingerprint = hex::encode(Sha256::digest(made.cert.der()));
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![made.cert.der().clone()],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
                made.signing_key.serialize_der(),
            )),
        )
        .unwrap();
        let acceptor = TlsAcceptor::from(Arc::new(config));
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("https://{}", listener.local_addr().unwrap());
        let router = source.router.clone();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let (acceptor, router) = (acceptor.clone(), router.clone());
                tokio::spawn(async move {
                    if let Ok(stream) = acceptor.accept(stream).await {
                        let _ = Builder::new(TokioExecutor::new())
                            .serve_connection(
                                TokioIo::new(stream),
                                TowerToHyperService::new(router),
                            )
                            .await;
                    }
                });
            }
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
            ticket(
                &source.signer,
                "pull",
                json!("core"),
                root.as_str(),
                crate::now(),
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
