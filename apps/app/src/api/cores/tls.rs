//! TLS to a Core, trusting only the certificate whose fingerprint the App learned at pairing.
//!
//! A Core serves HTTPS with a certificate it made itself, so nothing else can vouch for it. The
//! handshake signature is still checked, so the other side must hold that certificate's key.

use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{
    Request, StatusCode, body::Body, client::conn::http1::SendRequest,
};
use hyper_util::rt::TokioIo;
use rustls::{
    ClientConfig, DigitallySignedStruct, Error, SignatureScheme,
    client::danger::{
        HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
    },
    crypto::{
        CryptoProvider, aws_lc_rs::default_provider, verify_tls12_signature,
        verify_tls13_signature,
    },
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use sha2::{Digest, Sha256};
use tokio::net::TcpStream;
use tokio_rustls::{TlsConnector, client::TlsStream};

/// How long a call the App makes itself, such as sign-in, may take in all.
const CALL_TIMEOUT: Duration = Duration::from_secs(10);

/// One Core's address and the certificate it must present there.
pub struct Target {
    host: String,
    port: u16,
    connector: TlsConnector,
}

impl Target {
    /// `address` is `https://host:port`; `fingerprint` is the certificate's SHA-256 in hex.
    pub fn new(address: &str, fingerprint: &str) -> Result<Self, String> {
        let url =
            url::Url::parse(address).map_err(|error| error.to_string())?;
        let (host, port) =
            match (url.scheme(), url.host_str(), url.port_or_known_default()) {
                ("https", Some(host), Some(port)) => {
                    (host.trim_matches(['[', ']']).to_string(), port)
                }
                _ => {
                    return Err(
                        "a Core address must be https://host:port".into()
                    );
                }
            };
        let fingerprint: [u8; 32] = hex(fingerprint)
            .ok_or("a Core fingerprint must be 64 hex characters")?;
        let provider = Arc::new(default_provider());
        let config = ClientConfig::builder_with_provider(Arc::clone(&provider))
            .with_safe_default_protocol_versions()
            .map_err(|error| error.to_string())?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(Pinned {
                fingerprint,
                provider,
            }))
            .with_no_client_auth();
        Ok(Self {
            host,
            port,
            connector: TlsConnector::from(Arc::new(config)),
        })
    }

    pub async fn connect(&self) -> std::io::Result<TlsStream<TcpStream>> {
        let stream =
            TcpStream::connect((self.host.as_str(), self.port)).await?;
        // The name is not checked: the certificate is matched by fingerprint.
        let name =
            ServerName::try_from("amberite-core").expect("a valid DNS name");
        self.connector.connect(name, stream).await
    }

    /// A new HTTP/1.1 connection to the Core. The connection runs until the sender is dropped,
    /// and supports upgrades, which the console socket needs.
    pub async fn open<B>(&self) -> Result<SendRequest<B>, String>
    where
        B: Body + Send + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let stream = self.connect().await.map_err(|error| error.to_string())?;
        let (sender, connection) =
            hyper::client::conn::http1::handshake(TokioIo::new(stream))
                .await
                .map_err(|error| error.to_string())?;
        tokio::spawn(async move {
            let _ = connection.with_upgrades().await;
        });
        Ok(sender)
    }

    /// Send one request with a JSON body on its own connection and read the whole answer, giving
    /// up after [`CALL_TIMEOUT`].
    pub async fn call(
        &self,
        request: Request<Full<Bytes>>,
    ) -> Result<(StatusCode, Bytes), String> {
        let call = async {
            let mut sender = self.open().await?;
            let response = sender
                .send_request(request)
                .await
                .map_err(|error| error.to_string())?;
            let status = response.status();
            let body = response
                .into_body()
                .collect()
                .await
                .map_err(|error| error.to_string())?
                .to_bytes();
            Ok((status, body))
        };
        tokio::time::timeout(CALL_TIMEOUT, call)
            .await
            .map_err(|_| "no answer in time".to_string())?
    }

    /// `host:port`, as a `Host` header names it.
    pub fn authority(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

fn hex(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 || !text.is_ascii() {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(bytes)
}

/// Accepts exactly one certificate: the one whose SHA-256 is `fingerprint`.
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
                "this is not the certificate of the paired Core".into(),
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
    use super::*;

    #[test]
    fn only_the_pinned_certificate_is_accepted() {
        let certificate = CertificateDer::from(b"the paired Core".to_vec());
        let other = CertificateDer::from(b"somebody else".to_vec());
        let pinned = Pinned {
            fingerprint: Sha256::digest(&certificate).into(),
            provider: Arc::new(default_provider()),
        };
        let verify = |certificate: &CertificateDer<'_>| {
            pinned.verify_server_cert(
                certificate,
                &[],
                &ServerName::try_from("amberite-core").unwrap(),
                &[],
                UnixTime::now(),
            )
        };
        assert!(verify(&certificate).is_ok());
        assert!(verify(&other).is_err());
    }

    #[test]
    fn a_fingerprint_is_64_hex_characters() {
        assert_eq!(hex(&"0f".repeat(32)), Some([15u8; 32]));
        assert_eq!(hex("0f"), None);
        assert_eq!(hex(&"zz".repeat(32)), None);
    }
}
