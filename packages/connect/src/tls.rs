//! TLS between Amberite programs, which know each other by certificate. Each makes its own
//! certificate, so nothing else can vouch for it: its SHA-256 is its id. A dialer trusts only the
//! certificate whose id it learned at pairing or from the backend. A listener takes any
//! certificate and learns the dialer's id from it. The handshake signature is always checked, so
//! the other side must hold that certificate's key. Every side uses ring, which avoids
//! aws-lc-sys's CMake build on Windows.

use std::{fmt, str::FromStr, sync::Arc};

use rustls::{
    ClientConfig, DigitallySignedStruct, DistinguishedName, Error,
    ServerConfig, SignatureScheme,
    client::danger::{
        HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
    },
    crypto::{
        CryptoProvider, ring::default_provider, verify_tls12_signature,
        verify_tls13_signature,
    },
    pki_types::{
        CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime,
    },
    server::danger::{ClientCertVerified, ClientCertVerifier},
    version::TLS13,
};
use sha2::{Digest, Sha256};

/// Who a program is: the SHA-256 of its certificate.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PeerId(pub [u8; 32]);

impl PeerId {
    pub fn of(certificate: &[u8]) -> Self {
        Self(Sha256::digest(certificate).into())
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&hex::encode(self.0))
    }
}

impl fmt::Debug for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PeerId({self})")
    }
}

impl FromStr for PeerId {
    type Err = &'static str;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        parse_fingerprint(text)
            .map(Self)
            .ok_or("an id is 64 hex characters")
    }
}

/// A program's certificate and its key, both DER.
#[derive(Clone)]
pub struct Identity {
    certificate: Vec<u8>,
    /// PKCS#8.
    key: Vec<u8>,
}

impl Identity {
    pub fn new(certificate: Vec<u8>, key: Vec<u8>) -> Self {
        Self { certificate, key }
    }

    /// A new identity, for a program that keeps none between runs.
    pub fn generate() -> Result<Self, rcgen::Error> {
        let made =
            rcgen::generate_simple_self_signed(vec![SERVER_NAME.to_string()])?;
        Ok(Self {
            certificate: made.cert.der().to_vec(),
            key: made.signing_key.serialize_der(),
        })
    }

    pub fn id(&self) -> PeerId {
        PeerId::of(&self.certificate)
    }

    /// What this program signs its "I do not know this connection" answers with (QUIC's
    /// stateless reset). It comes from the key, so a program that restarted can tell a dialer
    /// its old connection is gone, and the dialer need not wait for silence to mean that.
    pub(crate) fn reset_key(&self) -> ring::hmac::Key {
        let mut seed = Sha256::new();
        seed.update(b"amberite connect reset");
        seed.update(&self.key);
        ring::hmac::Key::new(ring::hmac::HMAC_SHA256, &seed.finalize()[..])
    }

    fn chain(&self) -> Vec<CertificateDer<'static>> {
        vec![CertificateDer::from(self.certificate.clone())]
    }

    fn key(&self) -> PrivateKeyDer<'static> {
        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key.clone()))
    }
}

/// TLS 1.3 for a dialer that shows `identity` and accepts exactly `peer`'s certificate.
pub(crate) fn dialer_config(
    identity: &Identity,
    peer: PeerId,
    alpn: &[u8],
) -> Result<ClientConfig, Error> {
    let provider = Arc::new(default_provider());
    let mut config = ClientConfig::builder_with_provider(Arc::clone(&provider))
        .with_protocol_versions(&[&TLS13])?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pinned {
            fingerprint: peer.0,
            provider,
        }))
        .with_client_auth_cert(identity.chain(), identity.key())?;
    config.alpn_protocols = vec![alpn.to_vec()];
    Ok(config)
}

/// TLS 1.3 for a listener that shows `identity` and takes any dialer's certificate, which is how
/// it learns who dialed.
pub(crate) fn listener_config(
    identity: &Identity,
    alpns: &[&[u8]],
) -> Result<ServerConfig, Error> {
    let provider = Arc::new(default_provider());
    let mut config = ServerConfig::builder_with_provider(Arc::clone(&provider))
        .with_protocol_versions(&[&TLS13])?
        .with_client_cert_verifier(Arc::new(AnyDialer { provider }))
        .with_single_cert(identity.chain(), identity.key())?;
    config.alpn_protocols = alpns.iter().map(|alpn| alpn.to_vec()).collect();
    Ok(config)
}

/// The server name sent to a Core, and the name on every certificate. It is not checked: the
/// certificate is matched by fingerprint.
pub const SERVER_NAME: &str = "amberite-core";

/// A certificate's SHA-256 from its 64 hex characters.
pub fn parse_fingerprint(text: &str) -> Option<[u8; 32]> {
    let mut fingerprint = [0; 32];
    hex::decode_to_slice(text, &mut fingerprint).ok()?;
    Some(fingerprint)
}

/// A client config that accepts exactly the certificate whose SHA-256 is `fingerprint`.
pub fn pinned_config(fingerprint: [u8; 32]) -> Result<ClientConfig, Error> {
    let provider = Arc::new(default_provider());
    Ok(ClientConfig::builder_with_provider(Arc::clone(&provider))
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pinned {
            fingerprint,
            provider,
        }))
        .with_no_client_auth())
}

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
                "this is not the certificate pinned for this Core".into(),
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

/// Takes any certificate a dialer shows, as long as it shows one. Its id is read from the
/// finished handshake.
#[derive(Debug)]
struct AnyDialer {
    provider: Arc<CryptoProvider>,
}

impl ClientCertVerifier for AnyDialer {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, Error> {
        Ok(ClientCertVerified::assertion())
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
        let pinned = Pinned {
            fingerprint: Sha256::digest(&certificate).into(),
            provider: Arc::new(default_provider()),
        };
        let verify = |certificate: &CertificateDer<'_>| {
            pinned.verify_server_cert(
                certificate,
                &[],
                &ServerName::try_from(SERVER_NAME).unwrap(),
                &[],
                UnixTime::now(),
            )
        };
        assert!(verify(&certificate).is_ok());
        assert!(verify(&CertificateDer::from(b"another".to_vec())).is_err());
    }

    #[test]
    fn a_fingerprint_is_64_hex_characters() {
        assert_eq!(parse_fingerprint(&"0f".repeat(32)), Some([15u8; 32]));
        assert_eq!(parse_fingerprint("0f"), None);
        assert_eq!(parse_fingerprint(&"zz".repeat(32)), None);
    }
}
