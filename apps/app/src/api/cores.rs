//! Pinned connections to paired Cores.
//!
//! A Core serves HTTPS with a certificate it made itself, so the WebView cannot talk to it
//! directly. For each Core the App opens a port on this computer and carries every connection to
//! the Core over TLS, accepting only the certificate whose fingerprint the App learned at
//! pairing. Requests, uploads, and WebSockets all go through that one local address.

use std::{collections::HashMap, sync::Arc};

use rustls::{
    client::danger::{
        HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
    },
    crypto::{
        aws_lc_rs::default_provider, verify_tls12_signature,
        verify_tls13_signature, CryptoProvider,
    },
    pki_types::{CertificateDer, ServerName, UnixTime},
    ClientConfig, DigitallySignedStruct, Error, SignatureScheme,
};
use sha2::{Digest, Sha256};
use tauri::{plugin::TauriPlugin, State};
use tokio::{
    io::copy_bidirectional,
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
use tokio_rustls::TlsConnector;

pub fn init<R: tauri::Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("cores")
        .setup(|app, _| {
            use tauri::Manager;
            app.manage(Tunnels::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![core_connect])
        .build()
}

/// Open local addresses, keyed by Core address and fingerprint.
#[derive(Default)]
struct Tunnels(Mutex<HashMap<(String, String), String>>);

/// Returns the local address (`http://127.0.0.1:<port>`) that reaches the Core at `address`
/// (`https://host:port`), trusting only the certificate with this SHA-256 `fingerprint`.
#[tauri::command]
async fn core_connect(
    address: String,
    fingerprint: String,
    tunnels: State<'_, Tunnels>,
) -> Result<String, String> {
    let mut open = tunnels.0.lock().await;
    let key = (address, fingerprint);
    if let Some(local) = open.get(&key) {
        return Ok(local.clone());
    }
    let url = url::Url::parse(&key.0).map_err(|error| error.to_string())?;
    let target = match (url.scheme(), url.host_str(), url.port_or_known_default())
    {
        ("https", Some(host), Some(port)) => {
            (host.trim_matches(['[', ']']).to_string(), port)
        }
        _ => return Err("a Core address must be https://host:port".into()),
    };
    let fingerprint: [u8; 32] = hex(&key.1)
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
    let connector = TlsConnector::from(Arc::new(config));

    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|error| error.to_string())?;
    let port = listener
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    tokio::spawn(async move {
        while let Ok((mut local, _)) = listener.accept().await {
            let connector = connector.clone();
            let target = target.clone();
            tokio::spawn(async move {
                let Ok(remote) = TcpStream::connect(target).await else {
                    return;
                };
                // The name is not checked: the certificate is matched by fingerprint.
                let name = ServerName::try_from("amberite-core")
                    .expect("a valid DNS name");
                if let Ok(mut remote) = connector.connect(name, remote).await {
                    let _ = copy_bidirectional(&mut local, &mut remote).await;
                }
            });
        }
    });

    let local = format!("http://127.0.0.1:{port}");
    open.insert(key, local.clone());
    Ok(local)
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
        if Sha256::digest(end_entity).as_slice() == self.fingerprint {
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
