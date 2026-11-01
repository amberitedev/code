//! TLS to a Core, trusting only the certificate whose fingerprint the App learned at pairing
//! (`bedrock::net::tls`).

use std::{sync::Arc, time::Duration};

use bedrock::net::{CoreAddress, tls};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{
    Request, StatusCode, body::Body, client::conn::http1::SendRequest,
};
use hyper_util::rt::TokioIo;
use rustls::pki_types::ServerName;
use tokio::net::TcpStream;
use tokio_rustls::{TlsConnector, client::TlsStream};

/// How long a call the App makes itself, such as sign-in, may take in all.
const CALL_TIMEOUT: Duration = Duration::from_secs(10);

/// One Core's address and the certificate it must present there.
pub struct Target {
    address: CoreAddress,
    connector: TlsConnector,
}

impl Target {
    /// `address` is `https://ip:port`; `fingerprint` is the certificate's SHA-256 in hex.
    pub fn new(address: &str, fingerprint: &str) -> Result<Self, String> {
        let address =
            CoreAddress::parse(address).map_err(|error| error.to_string())?;
        let fingerprint = tls::parse_fingerprint(fingerprint)
            .ok_or("a Core fingerprint must be 64 hex characters")?;
        let config = tls::pinned_config(fingerprint)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            address,
            connector: TlsConnector::from(Arc::new(config)),
        })
    }

    pub async fn connect(&self) -> std::io::Result<TlsStream<TcpStream>> {
        let stream = TcpStream::connect(self.address.socket()).await?;
        let name =
            ServerName::try_from(tls::SERVER_NAME).expect("a valid DNS name");
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

    /// `ip:port`, as a `Host` header names it.
    pub fn authority(&self) -> String {
        self.address.authority()
    }
}
