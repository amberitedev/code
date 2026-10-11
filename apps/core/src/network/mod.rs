//! Making this Core reachable from the internet without the owner opening a router page.
//!
//! One task runs for Core's lifetime. It keeps one forwarded port for every running Minecraft
//! server and one for Core's own API, opens this computer's firewall, asks the backend to test
//! each port from outside, and prints a diagnosis whenever the answer changes. Mappings are
//! renewed while Core runs, removed when a server stops, and removed when Core exits.

pub mod diagnosis;
pub mod firewall;
pub mod router;
pub mod tls;

use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::{broadcast, watch, Notify};
use tokio_util::sync::CancellationToken;

use crate::{
    application::state::AppState,
    domain::{
        event::Event,
        instance::{InstanceId, InstanceStatus},
    },
    infrastructure::minecraft::java::required_java_version,
};
use diagnosis::{diagnose, Case, Facts, Outside, RouterFacts};
use router::{Mapping, Router};

/// How often Core looks for a changed address, a restarted router, or a new network.
const TICK: Duration = Duration::from_secs(60);
/// How often every mapping is renewed. Far inside the one hour lease, so a router that restarted
/// and forgot its mappings has them back within this time.
const RENEW_EVERY: Duration = Duration::from_secs(5 * 60);
/// How often a port that passed the outside test is tested again.
const RECHECK_REACHABLE: Duration = Duration::from_secs(30 * 60);
/// How often a port that failed, or could not be tested, is tried again.
const RECHECK_FAILED: Duration = Duration::from_secs(5 * 60);
/// Different outside ports tried for one port before giving up until the next retry.
const OUTSIDE_PORTS_TRIED: u8 = 3;

/// Loopback, private network, and link-local addresses: not reachable from the internet.
pub fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback() || v4.is_private() || v4.is_link_local()
        }
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_private(IpAddr::V4(v4)),
            None => {
                v6.is_loopback()
                    || (v6.segments()[0] & 0xfe00) == 0xfc00
                    || (v6.segments()[0] & 0xffc0) == 0xfe80
            }
        },
    }
}

/// What the rest of Core may know about one port.
#[derive(Debug, Clone)]
pub struct PortState {
    /// Where the port is on the internet, when it is usable.
    pub outside: Option<SocketAddr>,
    pub case: Case,
    /// Why it is not reachable, in one line. Empty when it is.
    pub cause: String,
}

/// Shared between the forwarding task and the API handlers.
pub struct Network {
    servers: Mutex<HashMap<InstanceId, PortState>>,
    api: watch::Sender<Option<SocketAddr>>,
    wake: Notify,
}

impl Default for Network {
    fn default() -> Self {
        Self {
            servers: Mutex::new(HashMap::new()),
            api: watch::channel(None).0,
            wake: Notify::new(),
        }
    }
}

impl Network {
    /// The state of a running server's port. `None` until it has been looked at.
    pub fn server(&self, id: &InstanceId) -> Option<PortState> {
        self.servers.lock().unwrap().get(id).cloned()
    }

    /// Core's own API on the internet. `None` while it is not reachable from outside.
    pub fn api_address(&self) -> watch::Receiver<Option<SocketAddr>> {
        self.api.subscribe()
    }

    /// Look again now instead of at the next tick.
    pub fn wake(&self) {
        self.wake.notify_one();
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum Key {
    Api,
    Server(InstanceId),
}

/// One port Core wants reachable.
struct Port {
    what: String,
    port: u16,
    /// The program that listens on the port, when it is not Core itself: a server's Java.
    program: Option<String>,
    mapping: Option<Mapping>,
    /// Why the router last refused, and how many outside ports were asked for.
    refusal: Option<(String, u8)>,
    /// When to ask a router that refused again.
    retry_at: Instant,
    outside: Outside,
    /// The outside port the last outside test was for, and when it ran.
    checked: Option<(u16, Instant)>,
    /// The last report printed, so the same one is not printed twice.
    printed: String,
}

/// Keep Core's ports forwarded until `stop` is cancelled, then remove every mapping.
pub async fn run(state: Arc<AppState>, stop: CancellationToken) {
    let mut events = state.broadcaster.subscribe();
    let mut ports: HashMap<Key, Port> = HashMap::new();
    let mut router: Option<Router> = None;
    let mut local_ip: Option<Ipv4Addr> = None;
    let mut wan_ip: Option<IpAddr> = None;
    let mut firewall = firewall::Checked::default();
    let mut firewall_ports: Vec<u16> = Vec::new();
    let mut renewed_at = Instant::now();

    loop {
        let wanted = wanted(&state).await;

        // A different local address means a different network: the old router and its mappings
        // are gone.
        let ip = crate::config::local_ipv4();
        if ip != local_ip {
            local_ip = ip;
            router = None;
            for port in ports.values_mut() {
                port.mapping = None;
                port.checked = None;
            }
        }

        // Servers that stopped: remove their mappings.
        let gone: Vec<Key> = ports
            .keys()
            .filter(|key| !wanted.iter().any(|(wanted, ..)| wanted == *key))
            .cloned()
            .collect();
        for key in gone {
            let Some(port) = ports.remove(&key) else {
                continue;
            };
            if let (Some(mapping), Some(router)) = (port.mapping, &router) {
                println!(
                    "\n{} stopped: outside port {} is closed again.\n",
                    port.what, mapping.external
                );
                router.remove(mapping).await;
            }
            if let Key::Server(id) = &key {
                state.network.servers.lock().unwrap().remove(id);
            }
        }
        for (key, what, port, program) in &wanted {
            let known = ports.entry(key.clone()).or_insert_with(|| Port {
                what: what.clone(),
                port: *port,
                program: program.clone(),
                mapping: None,
                refusal: None,
                retry_at: Instant::now(),
                outside: Outside::NotChecked("not tested yet".into()),
                checked: None,
                printed: String::new(),
            });
            known.what.clone_from(what);
        }

        if let (Some(ip), false) = (local_ip, ports.is_empty()) {
            let numbers: Vec<u16> =
                ports.values().map(|port| port.port).collect();
            if numbers.iter().any(|port| !firewall_ports.contains(port)) {
                firewall =
                    firewall::ensure(&numbers, &state.config.data_dir).await;
                firewall_ports = numbers;
            }

            let nat = is_private(IpAddr::V4(ip));
            if nat && router.is_none() {
                router = Router::discover(ip).await;
            }
            let seen = match &router {
                Some(router) => router.wan_ip().await,
                None => None,
            };
            // The home address changed: every earlier outside test is about the old one.
            if seen != wan_ip {
                wan_ip = seen;
                for port in ports.values_mut() {
                    port.checked = None;
                }
            }
            let renew = renewed_at.elapsed() >= RENEW_EVERY;
            if renew {
                renewed_at = Instant::now();
            }

            for (key, port) in &mut ports {
                let listening = listening(port.port).await;
                if let Some(router) = &router {
                    if renew {
                        if let Some(mapping) = &mut port.mapping {
                            if router.renew(mapping).await.is_err() {
                                port.mapping = None;
                            }
                        }
                    }
                    if port.mapping.is_none() && port.retry_at <= Instant::now()
                    {
                        establish(&state, router, port).await;
                    }
                }
                let target = SocketAddr::new(
                    wan_ip.unwrap_or(IpAddr::V4(ip)),
                    port.mapping.as_ref().map_or(port.port, |m| m.external),
                );
                let due = match (&port.checked, &port.outside) {
                    (None, _) => true,
                    (Some((checked, _)), _) if *checked != target.port() => {
                        true
                    }
                    (Some((_, at)), Outside::Reachable { .. }) => {
                        at.elapsed() >= RECHECK_REACHABLE
                    }
                    (Some((_, at)), _) => at.elapsed() >= RECHECK_FAILED,
                };
                if due && listening {
                    port.outside = outside(&state, target.port()).await;
                    port.checked = Some((target.port(), Instant::now()));
                }

                let facts = Facts {
                    what: port.what.clone(),
                    port: port.port,
                    local_ip: IpAddr::V4(ip),
                    listening,
                    firewall: firewall.for_program(port.program.as_deref()),
                    router: match (&router, nat) {
                        (_, false) => RouterFacts::NoNat,
                        (None, true) => RouterFacts::NoProtocol {
                            gateway: router::default_gateway(),
                        },
                        (Some(router), true) => RouterFacts::Found {
                            protocols: router.protocols(),
                            wan_ip,
                            mapping: match (&port.mapping, &port.refusal) {
                                (Some(mapping), _) => Ok(mapping.external),
                                (None, Some((refusal, _))) => {
                                    Err(refusal.clone())
                                }
                                (None, None) => Err("no answer".into()),
                            },
                            ports_tried: port
                                .refusal
                                .as_ref()
                                .map_or(1, |(_, tried)| *tried),
                        },
                    },
                    outside: port.outside.clone(),
                };
                let diagnosis = diagnose(&facts);
                let address = match &facts.outside {
                    Outside::Reachable { ip, port } => {
                        Some(SocketAddr::new(*ip, *port))
                    }
                    // Not proven, so only an address the internet could actually use.
                    _ if diagnosis.usable() && !is_private(target.ip()) => {
                        Some(target)
                    }
                    _ => None,
                };
                let report = match (diagnosis.case, address) {
                    (Case::Reachable, Some(address)) => format!(
                        "\n{} is reachable from the internet at {address}.\n",
                        port.what
                    ),
                    _ => diagnosis.render(&facts),
                };
                if report != port.printed {
                    println!("{report}");
                    port.printed = report;
                }
                let usable = address.filter(|_| diagnosis.usable());
                match key {
                    Key::Api => {
                        state.network.api.send_if_modified(|current| {
                            let changed = *current != usable;
                            *current = usable;
                            changed
                        });
                    }
                    Key::Server(id) => {
                        state.network.servers.lock().unwrap().insert(
                            id.clone(),
                            PortState {
                                outside: usable,
                                case: diagnosis.case,
                                cause: if diagnosis.usable() {
                                    String::new()
                                } else {
                                    diagnosis.title.clone()
                                },
                            },
                        );
                    }
                }
            }
        }

        tokio::select! {
            () = tokio::time::sleep(TICK) => {}
            () = state.network.wake.notified() => {}
            () = server_changed(&mut events) => {}
            () = stop.cancelled() => break,
        }
    }

    if let Some(router) = &router {
        for port in ports.into_values() {
            if let Some(mapping) = port.mapping {
                router.remove(mapping).await;
            }
        }
    }
}

/// The ports Core wants reachable now: its own API unless it only listens on this computer, and
/// every server that has finished starting.
async fn wanted(state: &AppState) -> Vec<(Key, String, u16, Option<String>)> {
    let mut wanted = Vec::new();
    let api_is_local = state
        .config
        .bind_host
        .parse::<IpAddr>()
        .is_ok_and(|host| host.is_loopback());
    if !api_is_local {
        wanted.push((
            Key::Api,
            "The Core's own API".into(),
            state.config.port,
            None,
        ));
    }
    let running = state
        .instance_store
        .list_by_status(InstanceStatus::Running)
        .await
        .unwrap_or_default();
    for record in running {
        if state.instances.contains_key(&record.id) {
            wanted.push((
                Key::Server(record.id.clone()),
                format!("Minecraft server \"{}\"", record.name),
                record.port,
                state
                    .java_store
                    .find_by_version(required_java_version(
                        &record.game_version,
                    ))
                    .await
                    .map(|java| java.display().to_string()),
            ));
        }
    }
    wanted
}

/// Resolves when a server started, stopped, or was removed.
async fn server_changed(events: &mut broadcast::Receiver<Event>) {
    loop {
        match events.recv().await {
            Ok(Event::StatusChanged { .. } | Event::InstanceDeleted { .. })
            | Err(broadcast::error::RecvError::Lagged(_)) => return,
            Ok(_) => {}
            Err(broadcast::error::RecvError::Closed) => {
                std::future::pending::<()>().await;
            }
        }
    }
}

/// Whether something on this computer accepts connections on `port`.
async fn listening(port: u16) -> bool {
    tokio::time::timeout(
        Duration::from_secs(2),
        tokio::net::TcpStream::connect((Ipv4Addr::LOCALHOST, port)),
    )
    .await
    .is_ok_and(|connected| connected.is_ok())
}

/// Get `port` a mapping. The same outside port is asked for first; when the router refuses it, or
/// grants it but the outside test fails, other outside ports are tried.
async fn establish(state: &AppState, router: &Router, port: &mut Port) {
    let mut refusal = String::new();
    for attempt in 1..=OUTSIDE_PORTS_TRIED {
        let external = if attempt == 1 {
            port.port
        } else {
            // Any port from the range routers leave free for this.
            49152 + (uuid::Uuid::new_v4().as_u128() % 16000) as u16
        };
        match router.map(port.port, external).await {
            Ok(mapping) => {
                let result = outside(state, mapping.external).await;
                let failed = matches!(result, Outside::Failed { .. });
                if failed && attempt < OUTSIDE_PORTS_TRIED {
                    router.remove(mapping).await;
                    continue;
                }
                port.checked = Some((mapping.external, Instant::now()));
                port.outside = result;
                port.mapping = Some(mapping);
                port.refusal = None;
                return;
            }
            Err(error) => refusal = error,
        }
    }
    port.refusal = Some((refusal, OUTSIDE_PORTS_TRIED));
    port.retry_at = Instant::now() + RENEW_EVERY;
}

#[derive(Deserialize)]
struct Reachability {
    /// The address the backend saw this Core call from, and the only one it tests.
    address: IpAddr,
    /// False when the backend is on this Core's own network and so cannot test from outside.
    outside: bool,
    reachable: bool,
    #[serde(default)]
    detail: String,
}

/// Ask the backend to connect to `port` on this Core's address from the internet.
async fn outside(state: &AppState, port: u16) -> Outside {
    let key = match state.owner().await {
        Ok(Some(owner)) => owner.backend_key,
        _ => None,
    };
    let Some(key) = key else {
        return Outside::NotChecked(
            "this Core is not paired to an account yet".into(),
        );
    };
    let response = state
        .http
        .post(format!(
            "{}/core/reachability",
            state.config.account_api_url
        ))
        .bearer_auth(key)
        .timeout(Duration::from_secs(20))
        .json(&json!({ "port": port }))
        .send()
        .await;
    let unavailable = || {
        Outside::NotChecked("the Amberite service could not be reached".into())
    };
    let Ok(response) = response else {
        return unavailable();
    };
    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        return Outside::NotChecked(
            "Core asked for too many tests in a short time and tries again later"
                .into(),
        );
    }
    if !response.status().is_success() {
        return unavailable();
    }
    let Ok(result) = response.json::<Reachability>().await else {
        return unavailable();
    };
    if !result.outside {
        return Outside::NotChecked(format!(
            "the Amberite service this Core uses is on the same network ({}), so it cannot test from the internet",
            result.address
        ));
    }
    if result.reachable {
        Outside::Reachable {
            ip: result.address,
            port,
        }
    } else {
        Outside::Failed {
            ip: result.address,
            port,
            how: result.detail,
        }
    }
}
