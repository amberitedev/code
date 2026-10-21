//! Port mappings that belong to servers, and how reachable each port is.
//!
//! Core asks the router to forward its API port, and each running server's game port. A server's
//! mapping goes when the server stops, never when Core exits: servers outlive Core, and a mapping
//! lasts its lease (an hour) by itself. Mappings are kept in the database, so a Core that
//! restarts renews the ones it had instead of asking for new ones.
//!
//! Every few minutes Core reconciles: it forwards what should be forwarded and is not, looking for
//! the router again when none answered, and drops what belongs to servers no longer running.
//! Every 20 minutes it renews every lease and checks every port again. After forwarding a port,
//! Core checks it and keeps the diagnosis, which the API reports with its single next step. A
//! check started for one run of a server maps and reports nothing once that run is over.

use std::{
    collections::{HashMap, HashSet},
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
/// How often Core forwards what is not forwarded yet, and drops what is no longer running.
const RECONCILE_EVERY: Duration = Duration::from_secs(5 * 60);
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

/// Whether the run a check started for still runs. Core's own API always does.
type Current<'a> = &'a (dyn Fn() -> bool + Sync);

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
    /// and stopping, reconcile, and renew the leases. Runs as long as Core does.
    pub async fn run(self: Arc<Self>, servers: Arc<Servers>) {
        if !self.on {
            return;
        }
        self.load().await;
        let mut events = servers.events.subscribe();
        // The runs Core adopted at start were announced before this subscribed.
        self.reconcile(&servers, true).await;
        let mut reconcile = tokio::time::interval(RECONCILE_EVERY);
        let mut renew = tokio::time::interval(RENEW_EVERY);
        reconcile.tick().await;
        renew.tick().await;
        loop {
            tokio::select! {
                event = events.recv() => match event {
                    Ok(Lifecycle::Running(id)) => {
                        tokio::spawn(Arc::clone(&self).check(Arc::clone(&servers), id));
                    }
                    Ok(Lifecycle::Ended(id)) => self.close(&id).await,
                    // The next reconcile catches up.
                    Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => return,
                },
                _ = reconcile.tick() => self.reconcile(&servers, false).await,
                _ = renew.tick() => self.reconcile(&servers, true).await,
            }
        }
    }

    /// Bring the mappings in line with what runs: drop those of servers that stopped, and
    /// forward and check the ports not forwarded yet. With `all`, renew and check every port.
    async fn reconcile(self: &Arc<Self>, servers: &Arc<Servers>, all: bool) {
        let running: HashSet<String> = servers.running().into_iter().collect();
        let mapped: HashSet<String> =
            self.routing.lock().await.mappings.keys().cloned().collect();
        let reported: Vec<String> = self
            .reports
            .lock()
            .expect("not poisoned")
            .keys()
            .cloned()
            .collect();
        for key in mapped.iter().chain(&reported) {
            if key != CORE && !running.contains(key) {
                self.close(key).await;
            }
        }
        if all || !mapped.contains(CORE) {
            let me = Arc::clone(self);
            tokio::spawn(async move {
                me.open(CORE, "Core's API".into(), me.api_port, &|| true)
                    .await;
            });
        }
        for id in running {
            if all || !mapped.contains(&id) {
                tokio::spawn(Arc::clone(self).check(Arc::clone(servers), id));
            }
        }
    }

    /// Forward a running server's port if it is not yet, and check it again.
    pub async fn check(
        self: Arc<Self>,
        servers: Arc<Servers>,
        server_id: String,
    ) {
        if !self.on {
            return;
        }
        let Some(run) = servers.run_of(&server_id) else {
            return;
        };
        let Ok(Some(server)) = records::get(&self.db, &server_id).await else {
            return;
        };
        let Ok(port) = u16::try_from(server.port) else {
            return;
        };
        let current = || servers.run_of(&server_id) == Some(run);
        self.open(
            &server_id,
            format!("Minecraft server \"{}\"", server.name),
            port,
            &current,
        )
        .await;
    }

    async fn open(
        &self,
        key: &str,
        what: String,
        port: u16,
        current: Current<'_>,
    ) {
        let local = public_ip::local_ipv4();
        let router = match local {
            None => RouterFacts::NoProtocol {
                gateway: router::default_gateway(),
            },
            Some(ip) if !is_private(IpAddr::V4(ip)) => RouterFacts::NoNat,
            Some(ip) => match self.forward(key, ip, port, current).await {
                Some(router) => router,
                None => return,
            },
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
        let mut reports = self.reports.lock().expect("not poisoned");
        // Under the lock `close` takes too, so a stopped server's report stays gone.
        if !current() {
            return;
        }
        if !diagnosis.usable() {
            info!("{}", diagnosis.render(&facts));
        }
        reports.insert(
            key.into(),
            Report {
                diagnosis,
                at: now(),
            },
        );
    }

    /// Ask the router to forward `port` for `key`, renewing a mapping it already has, also one
    /// an earlier Core made. `None` when the run it is for ended.
    async fn forward(
        &self,
        key: &str,
        local: Ipv4Addr,
        port: u16,
        current: Current<'_>,
    ) -> Option<RouterFacts> {
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
            return Some(RouterFacts::NoProtocol {
                gateway: router::default_gateway(),
            });
        };
        // Under the lock `close` takes too, so a stopped server gets no new mapping.
        if !current() {
            return None;
        }
        let mapping = match mappings.remove(key) {
            Some(mut mapping) => match router.renew(&mut mapping).await {
                Ok(()) => Ok(mapping),
                // The router forgot it, or this is another router: ask again.
                Err(error) => {
                    warn!(%key, %error, "could not renew a port mapping");
                    router.map(port, port).await
                }
            },
            None => router.map(port, port).await,
        };
        let granted = mapping
            .as_ref()
            .map(|mapping| mapping.external)
            .map_err(Clone::clone);
        match mapping {
            Ok(mapping) => {
                save(&self.db, key, &mapping).await;
                mappings.insert(key.into(), mapping);
            }
            Err(refusal) => {
                forget(&self.db, key).await;
                warn!(port, %refusal, "the router refused to forward");
            }
        }
        Some(RouterFacts::Found {
            protocols: router.protocols(),
            wan_ip: router.wan_ip().await,
            mapping: granted,
            ports_tried: 1,
        })
    }

    /// A server stopped: its mapping and report go.
    async fn close(&self, key: &str) {
        self.reports.lock().expect("not poisoned").remove(key);
        let mut routing = self.routing.lock().await;
        if let Some(mapping) = routing.mappings.remove(key) {
            Router::remove(routing.router.as_ref(), mapping).await;
        }
        forget(&self.db, key).await;
    }

    /// The mappings an earlier Core made, so they are renewed instead of made again.
    async fn load(&self) {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT key, mapping FROM port_mappings")
                .fetch_all(&self.db)
                .await
                .unwrap_or_default();
        let mut routing = self.routing.lock().await;
        for (key, mapping) in rows {
            match serde_json::from_str(&mapping) {
                Ok(mapping) => {
                    routing.mappings.insert(key, mapping);
                }
                Err(error) => {
                    warn!(%key, %error, "forgot a port mapping Core cannot read")
                }
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

async fn save(db: &SqlitePool, key: &str, mapping: &Mapping) {
    let result = sqlx::query(
        "INSERT INTO port_mappings (key, mapping) VALUES (?, ?) \
         ON CONFLICT (key) DO UPDATE SET mapping = excluded.mapping",
    )
    .bind(key)
    .bind(serde_json::to_string(mapping).expect("serializes"))
    .execute(db)
    .await;
    if let Err(error) = result {
        warn!(%key, %error, "could not save a port mapping");
    }
}

async fn forget(db: &SqlitePool, key: &str) {
    let _ = sqlx::query("DELETE FROM port_mappings WHERE key = ?")
        .bind(key)
        .execute(db)
        .await;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[tokio::test]
    async fn a_mapping_outlives_core_until_its_server_stops() {
        let db = db::memory().await;
        let mapping: Mapping = serde_json::from_str(
            r#"{"internal":25565,"external":25565,"via":{"protocol":"upnp"}}"#,
        )
        .unwrap();
        save(&db, "server", &mapping).await;
        // The next Core finds it and renews it rather than asking for a new one.
        let network = Network::off(db.clone());
        network.load().await;
        assert_eq!(
            network.routing.lock().await.mappings.get("server"),
            Some(&mapping)
        );
        // With no router found, UPnP cannot be asked; the lease ends by itself.
        network.close("server").await;
        assert!(network.routing.lock().await.mappings.is_empty());
        let left: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM port_mappings")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(left, 0);
    }
}
