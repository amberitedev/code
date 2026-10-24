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
/// Requests in a row the router may fail before Core looks for it again.
const FAILURES_BEFORE_SEARCH: u32 = 3;
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
    /// This machine's address and gateway when the router was found. Another one means
    /// another network.
    found_on: Option<(Ipv4Addr, Option<Ipv4Addr>)>,
    /// Requests the router failed in a row.
    failures: u32,
    /// Every mapping Core made and has not seen go: those of running servers, and those of
    /// stopped servers the router could not be asked to remove yet.
    mappings: HashMap<String, Mapping>,
    /// Keys whose renewal failed. Their mapping is kept, nonce and all, and renewed again at
    /// the next reconcile.
    renew: HashSet<String>,
}

impl Routing {
    /// Look for the router when there is none, when this machine moved to another network, or
    /// after it failed [`FAILURES_BEFORE_SEARCH`] requests in a row.
    async fn find(&mut self, local: Ipv4Addr) {
        let here = (local, router::default_gateway());
        if self.router.is_some()
            && (self.found_on != Some(here)
                || self.failures >= FAILURES_BEFORE_SEARCH)
        {
            info!("looking for the router again");
            self.router = None;
            self.searched = None;
        }
        let search = self.router.is_none()
            && self
                .searched
                .is_none_or(|searched| searched.elapsed() > SEARCH_AGAIN);
        if search {
            self.router = Router::discover(local).await;
            self.searched = Some(Instant::now());
            self.found_on = Some(here);
            self.failures = 0;
        }
    }
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
        // A mapping whose renewal failed counts as not forwarded yet.
        let mapped: HashSet<String> = {
            let routing = self.routing.lock().await;
            routing
                .mappings
                .keys()
                .filter(|key| !routing.renew.contains(*key))
                .cloned()
                .collect()
        };
        let pending: Vec<String> =
            self.routing.lock().await.mappings.keys().cloned().collect();
        let reported: Vec<String> = self
            .reports
            .lock()
            .expect("not poisoned")
            .keys()
            .cloned()
            .collect();
        for key in pending.iter().chain(&reported) {
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
        // Checks repeat every few minutes; the log says what changed, not the same advice again.
        let previous = reports.get(key).map(|report| &report.diagnosis);
        if changed(previous, &diagnosis) {
            if !diagnosis.usable() {
                info!("{}", diagnosis.render(&facts));
            } else if previous.is_some() {
                info!("{}: {}", facts.what, diagnosis.title);
            }
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
        routing.find(local).await;
        let Routing {
            router,
            mappings,
            renew,
            failures,
            ..
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
        // Renewing restores a mapping the router forgot. A failed renewal may still have
        // renewed it with only the answer lost, and a new mapping would clash with that one,
        // so the mapping is kept to renew again; only one made through another router is
        // replaced.
        let (result, kept) = match mappings.remove(key) {
            Some(mut mapping) => match router.renew(&mut mapping).await {
                Ok(()) => (Ok(mapping), None),
                Err(error) if router.serves(&mapping) => {
                    (Err(error), Some(mapping))
                }
                Err(_) => (router.map(port, port).await, None),
            },
            None => (router.map(port, port).await, None),
        };
        let granted = result
            .as_ref()
            .map(|mapping| mapping.external)
            .map_err(Clone::clone);
        match (result, kept) {
            (Ok(mapping), _) => {
                *failures = 0;
                renew.remove(key);
                save(&self.db, key, &mapping).await;
                mappings.insert(key.into(), mapping);
            }
            (Err(error), Some(mapping)) => {
                *failures += 1;
                renew.insert(key.into());
                mappings.insert(key.into(), mapping);
                warn!(%key, %error, "could not renew a port mapping; trying again soon");
            }
            (Err(refusal), None) => {
                *failures += 1;
                renew.remove(key);
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

    /// A server stopped: its report goes, and its mapping once the router removed it. A
    /// mapping only the router can remove stays, saved, until it can be asked: each reconcile
    /// tries again.
    async fn close(&self, key: &str) {
        self.reports.lock().expect("not poisoned").remove(key);
        let mut routing = self.routing.lock().await;
        routing.renew.remove(key);
        let Some(mapping) = routing.mappings.remove(key) else {
            forget(&self.db, key).await;
            return;
        };
        // After a restart Core may know the mapping before it found the router.
        if mapping.needs_router()
            && self.on
            && let Some(local) = public_ip::local_ipv4()
        {
            routing.find(local).await;
        }
        if Router::remove(routing.router.as_ref(), &mapping).await {
            forget(&self.db, key).await;
        } else {
            routing.mappings.insert(key.into(), mapping);
        }
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

/// Whether a check found something other than the one before it: another case or another
/// cause.
fn changed(previous: Option<&Diagnosis>, now: &Diagnosis) -> bool {
    previous.is_none_or(|previous| {
        previous.case != now.case || previous.title != now.title
    })
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

    #[test]
    fn the_same_result_is_logged_once() {
        use crate::network::diagnosis::Case;

        let result = |case, title: &str| Diagnosis {
            case,
            title: title.into(),
            checks: Vec::new(),
            found: Vec::new(),
            fix: Vec::new(),
        };
        let no_upnp =
            result(Case::NoProtocol, "The router does not forward ports");
        assert!(changed(None, &no_upnp), "the first check is logged");
        // The same advice again, with other checks behind it, is not.
        let mut again = no_upnp.clone();
        again.found.push("tried again".into());
        assert!(!changed(Some(&no_upnp), &again));
        assert!(changed(
            Some(&no_upnp),
            &result(Case::NoProtocol, "The router refused the port")
        ));
        assert!(changed(
            Some(&no_upnp),
            &result(Case::Unverified, "The router does not forward ports")
        ));
    }

    #[tokio::test]
    async fn a_mapping_only_the_router_can_remove_waits_for_it() {
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
        // Its server stopped, but no router was found to ask. A UPnP mapping may be permanent,
        // so it is kept, saved, for the next try.
        network.close("server").await;
        assert_eq!(
            network.routing.lock().await.mappings.get("server"),
            Some(&mapping)
        );
        let left: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM port_mappings")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(left, 1);
    }
}
