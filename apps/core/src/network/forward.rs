//! Port mappings that belong to servers, and how reachable each port is.
//!
//! Core asks the router to forward its API port when it starts, and a server's game port when the
//! server starts running or Core adopts it after a restart. A server's mapping goes when the
//! server stops, never when Core exits: servers outlive Core, and a mapping lasts its lease (an
//! hour) by itself. Core renews every lease while it runs. After forwarding a port, Core checks it
//! and keeps the diagnosis, which the API reports with its single next step.

use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};

use sqlx::SqlitePool;
use tokio::sync::broadcast::error::RecvError;
use tracing::{info, warn};

use super::{
    diagnosis::{Diagnosis, Facts, Outside, RouterFacts, diagnose},
    firewall::Firewall,
    is_private, public_ip,
    router::{self, Mapping, Router},
};
use crate::{
    access::now,
    servers::{Lifecycle, Servers, records},
};

/// The key of Core's own API mapping and report.
pub const CORE: &str = "core";
const RENEW_EVERY: Duration = Duration::from_secs(20 * 60);
/// How long Core waits before looking for a router again after none answered.
const SEARCH_AGAIN: Duration = Duration::from_secs(5 * 60);
/// How long a server may take to open its port before it is checked anyway.
const LISTEN_WAIT: Duration = Duration::from_secs(120);

/// What Core found about one port.
#[derive(Debug, Clone)]
pub struct Report {
    pub diagnosis: Diagnosis,
    /// Unix seconds.
    pub at: i64,
}

/// Forwarding and reachability for one Core.
pub struct Network {
    db: SqlitePool,
    api_port: u16,
    /// `copal install` opened the firewall.
    installed: bool,
    /// Off with `forward_ports` off, and in tests, which must not talk to the router.
    on: bool,
    routing: tokio::sync::Mutex<Routing>,
    reports: std::sync::Mutex<HashMap<String, Report>>,
}

#[derive(Default)]
struct Routing {
    router: Option<Router>,
    searched: Option<Instant>,
    mappings: HashMap<String, Mapping>,
}

impl Network {
    /// `on` is the `forward_ports` setting.
    pub fn new(
        db: SqlitePool,
        api_port: u16,
        installed: bool,
        on: bool,
    ) -> Self {
        Self {
            db,
            api_port,
            installed,
            on,
            routing: tokio::sync::Mutex::default(),
            reports: std::sync::Mutex::default(),
        }
    }

    /// A Network that never forwards or checks anything.
    #[cfg(test)]
    pub fn off(db: SqlitePool) -> Self {
        Self::new(db, 0, false, false)
    }

    /// The last check of a server's port, or of Core's with [`CORE`].
    pub fn report(&self, key: &str) -> Option<Report> {
        self.reports.lock().expect("not poisoned").get(key).cloned()
    }

    /// Forward and check Core's API and every running server, then follow servers starting
    /// and stopping, and renew the leases. Runs as long as Core does.
    pub async fn run(self: Arc<Self>, servers: Arc<Servers>) {
        if !self.on {
            return;
        }
        let mut events = servers.events.subscribe();
        let me = Arc::clone(&self);
        tokio::spawn(async move {
            me.open(CORE, "Core's API".into(), me.api_port).await;
        });
        for id in servers.running() {
            tokio::spawn(Arc::clone(&self).check(id));
        }
        let mut renew = tokio::time::interval(RENEW_EVERY);
        renew.tick().await;
        loop {
            tokio::select! {
                event = events.recv() => match event {
                    Ok(Lifecycle::Running(id)) => {
                        tokio::spawn(Arc::clone(&self).check(id));
                    }
                    Ok(Lifecycle::Ended(id)) => self.close(&id).await,
                    Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => return,
                },
                _ = renew.tick() => self.renew().await,
            }
        }
    }

    /// Forward a server's port if it is not yet, and check it again.
    pub async fn check(self: Arc<Self>, server_id: String) {
        if !self.on {
            return;
        }
        let Ok(Some(server)) = records::get(&self.db, &server_id).await else {
            return;
        };
        let Ok(port) = u16::try_from(server.port) else {
            return;
        };
        self.open(
            &server_id,
            format!("Minecraft server \"{}\"", server.name),
            port,
        )
        .await;
    }

    async fn open(&self, key: &str, what: String, port: u16) {
        let local = public_ip::local_ipv4();
        let router = match local {
            None => RouterFacts::NoProtocol {
                gateway: router::default_gateway(),
            },
            Some(ip) if !is_private(IpAddr::V4(ip)) => RouterFacts::NoNat,
            Some(ip) => self.forward(key, ip, port).await,
        };
        let listening = listening(port).await;
        let facts = Facts {
            what,
            port,
            local_ip: local
                .map_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED), IpAddr::V4),
            listening,
            firewall: self.firewall(),
            router,
            outside: Outside::NotChecked(
                "this Core has no test from the internet yet".into(),
            ),
        };
        let diagnosis = diagnose(&facts);
        if !diagnosis.usable() {
            info!("{}", diagnosis.render(&facts));
        }
        self.reports.lock().expect("not poisoned").insert(
            key.into(),
            Report {
                diagnosis,
                at: now(),
            },
        );
    }

    /// Ask the router to forward `port` for `key`, renewing a mapping it already has.
    async fn forward(
        &self,
        key: &str,
        local: Ipv4Addr,
        port: u16,
    ) -> RouterFacts {
        let mut routing = self.routing.lock().await;
        let search = routing.router.is_none()
            && routing
                .searched
                .is_none_or(|searched| searched.elapsed() > SEARCH_AGAIN);
        if search {
            routing.router = Router::discover(local).await;
            routing.searched = Some(Instant::now());
        }
        let Routing {
            router, mappings, ..
        } = &mut *routing;
        let Some(router) = router.as_ref() else {
            return RouterFacts::NoProtocol {
                gateway: router::default_gateway(),
            };
        };
        let mapping = match mappings.remove(key) {
            Some(mut mapping) => {
                router.renew(&mut mapping).await.map(|()| mapping)
            }
            None => router.map(port, port).await,
        };
        let granted = mapping
            .as_ref()
            .map(|mapping| mapping.external)
            .map_err(Clone::clone);
        match mapping {
            Ok(mapping) => {
                mappings.insert(key.into(), mapping);
            }
            Err(refusal) => {
                warn!(port, %refusal, "the router refused to forward")
            }
        }
        RouterFacts::Found {
            protocols: router.protocols(),
            wan_ip: router.wan_ip().await,
            mapping: granted,
            ports_tried: 1,
        }
    }

    /// A server stopped: its mapping and report go.
    async fn close(&self, key: &str) {
        self.reports.lock().expect("not poisoned").remove(key);
        let mut routing = self.routing.lock().await;
        if let Some(mapping) = routing.mappings.remove(key)
            && let Some(router) = &routing.router
        {
            router.remove(mapping).await;
        }
    }

    async fn renew(&self) {
        let mut routing = self.routing.lock().await;
        let Routing {
            router, mappings, ..
        } = &mut *routing;
        let Some(router) = router.as_ref() else {
            return;
        };
        for (key, mapping) in mappings.iter_mut() {
            if let Err(error) = router.renew(mapping).await {
                warn!(%key, %error, "could not renew a port mapping");
            }
        }
    }

    /// `copal install` opens the firewall for Core's port and the server ports, and Core never
    /// changes it, so after install it is open as far as Core knows.
    fn firewall(&self) -> Firewall {
        if self.installed {
            Firewall::Open
        } else {
            Firewall::Unsure {
                found: "Copal runs without copal install, which opens it".into(),
                fix: vec![
                    "Run copal install as admin. It opens the firewall for Core's port and the server ports."
                        .into(),
                ],
            }
        }
    }
}

/// Whether something on this computer accepts connections on `port`, waiting up to
/// [`LISTEN_WAIT`] for a server that is still starting.
async fn listening(port: u16) -> bool {
    let deadline = Instant::now() + LISTEN_WAIT;
    loop {
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
        let connect = tokio::net::TcpStream::connect(address);
        if let Ok(Ok(_)) =
            tokio::time::timeout(Duration::from_secs(2), connect).await
        {
            return true;
        }
        if Instant::now() > deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
