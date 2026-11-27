//! One Core as one account reaches it: a direct connection (the `connect` crate) to the program
//! holding the certificate the App learned at pairing, with a stream per HTTP connection.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use bytes::Bytes;
use connect::{
    API, Connection, Endpoint, Identity, PeerId, Stream, ip::is_private,
};
use http_body_util::{BodyExt, Full};
use hyper::{
    Request, StatusCode, body::Body, client::conn::http1::SendRequest,
};
use hyper_util::rt::TokioIo;
use tokio::sync::OnceCell;

use super::sessions::Key;
use crate::local_api::endpoints::cores::Reached;

/// How long a call the App makes itself, such as sign-in, may take in all.
const CALL_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a Core on this computer or its network may take to answer where it last was.
const REDIAL_TIMEOUT: Duration = Duration::from_secs(2);
/// How long a Core that did not answer is left alone, so requests to a Core that is off fail at
/// once instead of each waiting for it.
const REDIAL_REST: Duration = Duration::from_secs(5);

/// This App's endpoints, each bound on first use. The one for other computers is apart because
/// Windows asks about a program the first time it binds beyond loopback, and an App whose Core
/// is on the same computer never needs to.
pub struct Dialers {
    /// Made new on every start: a Core knows a device by its session, not by this.
    identity: Identity,
    loopback: OnceCell<Endpoint>,
    wide: OnceCell<Endpoint>,
}

impl Dialers {
    pub fn new() -> Self {
        Self {
            identity: Identity::generate()
                .expect("a certificate can always be made"),
            loopback: OnceCell::new(),
            wide: OnceCell::new(),
        }
    }

    /// The endpoint that reaches this computer, or the one that reaches every other.
    pub async fn toward(&self, loopback: bool) -> std::io::Result<&Endpoint> {
        let (cell, ip) = if loopback {
            (&self.loopback, Ipv4Addr::LOCALHOST)
        } else {
            (&self.wide, Ipv4Addr::UNSPECIFIED)
        };
        cell.get_or_try_init(|| async {
            Endpoint::dialer((ip, 0).into(), self.identity.clone())
        })
        .await
    }
}

/// Why a request did not get an answer.
pub enum Unanswered {
    /// There is no connection to the Core, and nothing was sent. Sending again once there is
    /// one is safe.
    Offline,
    /// The request may have reached the Core.
    Lost(String),
}

pub struct Tunnel {
    pub key: Key,
    pub core: PeerId,
    dialers: Arc<Dialers>,
    /// The last connection made. One that closed stays, for where the Core was.
    connection: Mutex<Option<Arc<Connection>>>,
    /// Held while dialing the Core on this computer or its network, so requests that fail
    /// together dial once. Holds where and when it last did not answer there.
    dialing_here: tokio::sync::Mutex<Option<(SocketAddr, Instant)>>,
}

impl Tunnel {
    pub fn new(key: Key, core: PeerId, dialers: Arc<Dialers>) -> Self {
        Self {
            key,
            core,
            dialers,
            connection: Mutex::default(),
            dialing_here: tokio::sync::Mutex::default(),
        }
    }

    fn connection(&self) -> Option<Arc<Connection>> {
        self.connection.lock().expect("not poisoned").clone()
    }

    /// How the Core is reached now, `None` while it is not.
    pub fn reached(&self) -> Option<Reached> {
        let connection = self.connection().filter(|held| held.is_open())?;
        Some(if is_private(connection.remote().ip()) {
            Reached::Local
        } else {
            Reached::Internet
        })
    }

    /// Connect to the Core at whichever of `candidates` answers first.
    pub async fn dial(
        &self,
        candidates: &[SocketAddr],
        within: Duration,
    ) -> bool {
        // Loopback candidates come alone: only a Core on this computer has one. The loopback
        // endpoint is IPv4; the other also takes IPv6.
        let loopback = !candidates.is_empty()
            && candidates.iter().all(
                |to| matches!(to.ip(), IpAddr::V4(ip) if ip.is_loopback()),
            );
        let Ok(endpoint) = self.dialers.toward(loopback).await else {
            return false;
        };
        match endpoint.dial(self.core, candidates, API, within).await {
            Ok(connection) => {
                *self.connection.lock().expect("not poisoned") =
                    Some(Arc::new(connection));
                true
            }
            Err(_) => false,
        }
    }

    /// Connect to the Core at `address` on this computer or its network, unless it did not
    /// answer at that address within the last [`REDIAL_REST`]. Calls made together dial once.
    pub async fn dial_here(
        &self,
        address: SocketAddr,
        within: Duration,
    ) -> bool {
        let mut unanswered = self.dialing_here.lock().await;
        if self.reached().is_some() {
            return true;
        }
        if unanswered.is_some_and(|(at, when)| {
            at == address && when.elapsed() < REDIAL_REST
        }) {
            return false;
        }
        let answered = self.dial(&[address], within).await;
        *unanswered = (!answered).then(|| (address, Instant::now()));
        answered
    }

    /// A new stream to the Core. When the connection is gone, a Core on this computer or its
    /// network is dialed again where it was, since it lets everyone there in. Any other needs a
    /// new introduction, which is the frontend's to ask for.
    async fn stream(&self) -> Option<Stream> {
        let held = self.connection()?;
        if let Ok(stream) = held.open().await {
            return Some(stream);
        }
        let was = held.remote();
        if !is_private(was.ip()) || !self.dial_here(was, REDIAL_TIMEOUT).await {
            return None;
        }
        self.connection()?.open().await.ok()
    }

    /// A new HTTP/1.1 connection to the Core, on a stream of its own. It runs until the sender
    /// is dropped, and supports upgrades, which the console socket needs.
    pub async fn open<B>(&self) -> Result<SendRequest<B>, Unanswered>
    where
        B: Body + Send + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let stream = self.stream().await.ok_or(Unanswered::Offline)?;
        let (sender, connection) =
            hyper::client::conn::http1::handshake(TokioIo::new(stream))
                .await
                .map_err(|error| Unanswered::Lost(error.to_string()))?;
        tokio::spawn(async move {
            let _ = connection.with_upgrades().await;
        });
        Ok(sender)
    }

    /// Send one request with a JSON body on its own stream and read the whole answer, giving up
    /// after [`CALL_TIMEOUT`].
    pub async fn call(
        &self,
        request: Request<Full<Bytes>>,
    ) -> Result<(StatusCode, Bytes), Unanswered> {
        let lost = |error: hyper::Error| Unanswered::Lost(error.to_string());
        let call = async {
            let mut sender = self.open().await?;
            let response = sender.send_request(request).await.map_err(lost)?;
            let status = response.status();
            let body = response
                .into_body()
                .collect()
                .await
                .map_err(lost)?
                .to_bytes();
            Ok((status, body))
        };
        tokio::time::timeout(CALL_TIMEOUT, call)
            .await
            .map_err(|_| Unanswered::Lost("no answer in time".into()))?
    }
}
