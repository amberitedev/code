//! Core's end of Connect (the `connect` crate): one UDP socket, on the API's port number, where
//! the owner's devices reach the API with no port forwarded.
//!
//! The socket answers nobody the backend did not introduce (`access::link`), except this
//! computer and its own networks, so the owner reaches a Core at home with the internet down. A
//! connection carries the same API as the local socket, one request per stream, and every route
//! still checks its caller. Core also asks the router to forward this port (`forward`): a
//! forwarded port is reached from any network, while a path opened by both sides sending at once
//! needs the dialer's router to cooperate.

use std::{net::SocketAddr, sync::Arc};

use axum::{Extension, Router};
use color_eyre::eyre::{Result, WrapErr};
use connect::{Endpoint, Gate, Identity};
use tokio::sync::Semaphore;
use tracing::debug;

use super::{
    forward::Network,
    tls::{HANDSHAKE_TIMEOUT, MAX_CONNECTIONS},
};
use crate::access::Peer;

pub struct Connect {
    endpoint: Endpoint,
    network: Arc<Network>,
}

impl Connect {
    /// Listen on `address`, as the Core `identity` names.
    pub fn open(
        address: SocketAddr,
        identity: Identity,
        network: Arc<Network>,
    ) -> Result<Self> {
        let endpoint = Endpoint::listener(
            address,
            identity,
            &[connect::API],
            Gate::IntroducedOrLocal,
        )
        .wrap_err_with(|| format!("could not listen on UDP {address}"))?;
        Ok(Self { endpoint, network })
    }

    /// A device at `dialer` was introduced: let it in and send toward it, then say where this
    /// Core might be reached, the port the router forwards first.
    pub async fn introduce(&self, dialer: &[SocketAddr]) -> Vec<SocketAddr> {
        self.endpoint.expect(dialer);
        let forwarded: Vec<u16> =
            self.network.connect_port().into_iter().collect();
        self.endpoint.candidates(&forwarded).await
    }

    /// Core is stopping: tell the connected Apps, so they need not wait to notice.
    pub async fn close(&self) {
        self.endpoint.close().await;
    }

    /// Serve `router` to every connection until the process ends.
    pub async fn serve(&self, router: Router) {
        let open = Arc::new(Semaphore::new(MAX_CONNECTIONS));
        while let Some(incoming) = self.endpoint.accept().await {
            let Ok(permit) = Arc::clone(&open).try_acquire_owned() else {
                incoming.refuse();
                continue;
            };
            let router = router.clone();
            tokio::spawn(async move {
                let _permit = permit;
                let Ok(Ok(connection)) =
                    tokio::time::timeout(HANDSHAKE_TIMEOUT, incoming.accept())
                        .await
                else {
                    return;
                };
                let peer = connection.remote();
                let router = router.layer(Extension(Peer(peer.ip())));
                while let Some(stream) = connection.accept().await {
                    let router = router.clone();
                    tokio::spawn(async move {
                        if let Err(error) =
                            crate::api::serve_connection(stream, router).await
                        {
                            debug!(%peer, %error, "stream closed");
                        }
                    });
                }
            });
        }
    }
}
