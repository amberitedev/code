//! TLS to a Core, trusting only the certificate whose SHA-256 the App learned at pairing or the
//! backend handed out. A Core serves HTTPS with a certificate it made itself, so nothing else can
//! vouch for it. The handshake signature is still checked, so the other side must hold that
//! certificate's key. Every side uses ring, which avoids aws-lc-sys's CMake build on Windows.

use std::sync::Arc;

use rustls::{
    ClientConfig, DigitallySignedStruct, Error, SignatureScheme,
    client::danger::{
        HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
    },
    crypto::{
        CryptoProvider, ring::default_provider, verify_tls12_signature,
        verify_tls13_signature,
    },
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use sha2::{Digest, Sha256};

/// The server name sent to a Core. It is not checked: the certificate is matched by fingerprint.
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
