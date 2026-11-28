//! `copal doctor`'s questions to the Core: pairing, the backend, the router, its ports, Java and
//! the disk, each checked when asked. The CLI adds the service and the firewall, which it reads
//! on the machine itself.

use std::{path::Path, sync::Arc, time::Duration};

use axum::extract::State;
use contracts::{
    ApiError, Endpoint, Json,
    backend::CheckHealth,
    core::{CheckDetail, CheckState, Doctor, DoctorCheck, GetDoctor},
    limits::DISK_MARGIN_BYTES,
    server::Req,
};
use sharing_client::disk::label;

use super::{CoreState, internal};
use crate::{
    access::{Caller, link::Link, pairing, require_owner},
    network::{
        diagnosis::{Case, RouterFacts, diagnose},
        forward::Checked,
        router::Transport,
    },
    servers::server_rows,
};

/// Below this much free space, the disk check warns.
const LOW_DISK: u64 = 5 * 1024 * 1024 * 1024;
/// How long an unpaired Core waits for the backend to answer.
const BACKEND_WAIT: Duration = Duration::from_secs(5);

pub async fn get(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    _: Req<GetDoctor>,
) -> Result<Json<Doctor>, ApiError> {
    require_owner(&caller)?;
    let paired = pairing::paired(&state.db).await.map_err(internal)?;
    let owner = super::pairing::owner(&state.db).await?;
    let backend_url = state.storage.backend_url();
    let (backend, checked) = tokio::join!(
        backend(&state, backend_url, paired),
        state.network.check_now(&state.servers),
    );
    let router = router(checked.as_ref());
    let ports = ports(checked.as_ref(), router.state);
    Ok(Json(Doctor {
        port: state.network.api_port(),
        installed: state.network.installed(),
        checks: vec![
            pairing_check(backend_url.is_some(), paired, owner.as_deref()),
            backend,
            router,
            ports,
            java(&state).await?,
        ],
    }))
}

fn check(
    name: &str,
    state: CheckState,
    summary: impl Into<String>,
    details: Vec<CheckDetail>,
    fix: Vec<String>,
) -> DoctorCheck {
    DoctorCheck {
        name: name.into(),
        state,
        summary: summary.into(),
        details,
        fix,
    }
}

fn detail(label: impl Into<String>, value: impl Into<String>) -> CheckDetail {
    CheckDetail {
        label: label.into(),
        value: value.into(),
    }
}

const NO_BACKEND: &str = "no Amberite backend is set, so this Core cannot pair";

fn pairing_check(
    backend_set: bool,
    paired: bool,
    owner: Option<&str>,
) -> DoctorCheck {
    match (backend_set, paired, owner) {
        (false, ..) => {
            check("pairing", CheckState::Skip, NO_BACKEND, vec![], vec![])
        }
        (true, true, Some(name)) => check(
            "pairing",
            CheckState::Ok,
            format!("paired with {name}"),
            vec![],
            vec![],
        ),
        (true, true, None) => check(
            "pairing",
            CheckState::Ok,
            "paired with an Amberite account",
            vec![],
            vec![],
        ),
        (true, false, _) => check(
            "pairing",
            CheckState::Fail,
            "not paired with an Amberite account",
            vec![],
            vec!["Pair it with: copal pair".into()],
        ),
    }
}

/// The link while paired; before that, whether the backend answers at all.
async fn backend(
    state: &CoreState,
    url: Option<&str>,
    paired: bool,
) -> DoctorCheck {
    let Some(url) = url else {
        return check("backend", CheckState::Skip, NO_BACKEND, vec![], vec![]);
    };
    let host = reqwest::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_else(|| url.to_owned());
    let reached = if paired {
        match state.access.link.lock().expect("not poisoned").clone() {
            Link::Open => Ok("connected to Amberite"),
            Link::Opening => {
                return check(
                    "backend",
                    CheckState::Warn,
                    "still connecting to Amberite",
                    vec![detail("Backend", host)],
                    vec![
                        "Run copal doctor again in a minute. copal logs says why if it does not connect."
                            .into(),
                    ],
                );
            }
            Link::Down(problem) => Err(problem),
        }
    } else {
        reachable(url).await.map(|()| "connected to Amberite")
    };
    match reached {
        Ok(summary) => check(
            "backend",
            CheckState::Ok,
            summary,
            vec![detail("Backend", host)],
            vec![],
        ),
        Err(problem) => check(
            "backend",
            CheckState::Fail,
            format!("cannot reach Amberite: {problem}"),
            vec![detail("Backend", host)],
            vec![
                "Check this computer's internet connection.".into(),
                "See why with: copal logs".into(),
            ],
        ),
    }
}

async fn reachable(url: &str) -> Result<(), String> {
    let http = crate::network::http()
        .timeout(BACKEND_WAIT)
        .build()
        .map_err(|error| error.to_string())?;
    match http.get(format!("{url}{}", CheckHealth::PATH)).send().await {
        Ok(response) if response.status().is_success() => Ok(()),
        Ok(response) => Err(format!("it answered {}", response.status())),
        Err(error) if error.is_timeout() => Err(format!(
            "no answer within {} seconds",
            BACKEND_WAIT.as_secs()
        )),
        Err(error) => Err(error.without_url().to_string()),
    }
}

const FORWARDING_OFF: &str = "port forwarding is switched off on this Core";

/// What kind of router the Core is behind, and so whether friends can join from the internet.
/// `checked` is `None` while forwarding is off.
fn router(checked: Option<&Checked>) -> DoctorCheck {
    let Some(checked) = checked else {
        return check(
            "router",
            CheckState::Skip,
            FORWARDING_OFF,
            vec![],
            vec![],
        );
    };
    let facts = &checked.facts;
    let diagnosis = diagnose(facts);
    let mut details = Vec::new();
    if let Some(gateway) = checked.gateway {
        details.push(detail("Router", gateway.to_string()));
    }
    let cannot = "friends cannot join from the internet";
    let (state, summary) = match &facts.router {
        RouterFacts::NoNat => {
            details.push(detail("This computer", facts.local_ip.to_string()));
            (
                CheckState::Ok,
                "this computer has a public address, friends can join from the internet"
                    .to_owned(),
            )
        }
        RouterFacts::NoProtocol { .. } => (
            CheckState::Fail,
            format!(
                "the router does not forward ports automatically, {cannot}"
            ),
        ),
        RouterFacts::Found {
            protocols, wan_ip, ..
        } => {
            details.push(detail("Protocols", protocols.clone()));
            details.push(detail(
                "Internet address",
                wan_ip.map_or("not given".into(), |ip| ip.to_string()),
            ));
            match diagnosis.case {
                Case::DoubleNat => (
                    CheckState::Fail,
                    format!("two routers in a row, {cannot}"),
                ),
                Case::SharedAddress => (
                    CheckState::Fail,
                    format!(
                        "the internet provider shares one address between customers (CGNAT), {cannot}"
                    ),
                ),
                _ => (
                    CheckState::Ok,
                    format!(
                        "answers {protocols}, friends can join from the internet"
                    ),
                ),
            }
        }
    };
    let fix = if state == CheckState::Fail {
        diagnosis.fix
    } else {
        vec![]
    };
    check("router", state, summary, details, fix)
}

/// The ports the router was asked to forward, skipped where the router cannot forward any.
fn ports(checked: Option<&Checked>, router: CheckState) -> DoctorCheck {
    let Some(checked) = checked else {
        return check(
            "ports",
            CheckState::Skip,
            "nothing is forwarded while that is off",
            vec![],
            vec![],
        );
    };
    if router == CheckState::Fail {
        return check(
            "ports",
            CheckState::Skip,
            "the router cannot forward ports from the internet",
            vec![],
            vec![],
        );
    }
    if matches!(checked.facts.router, RouterFacts::NoNat) {
        return check(
            "ports",
            CheckState::Ok,
            "this computer has a public address, nothing to forward",
            vec![],
            vec![],
        );
    }
    let details = checked
        .mappings
        .iter()
        .map(|mapped| {
            let transport = match mapped.transport {
                Transport::Tcp => "TCP",
                Transport::Udp => "UDP",
            };
            detail(
                format!("{}, {transport}", mapped.what),
                match &mapped.outside {
                    Ok(outside) => format!("{outside} -> {}", mapped.port),
                    Err(refusal) => format!("refused: {refusal}"),
                },
            )
        })
        .collect();
    let refused = checked.mappings.iter().find_map(|mapped| {
        mapped
            .outside
            .as_ref()
            .err()
            .map(|refusal| (mapped, refusal))
    });
    match refused {
        None => check(
            "ports",
            CheckState::Ok,
            format!("the router forwards {} ports", checked.mappings.len()),
            details,
            vec![],
        ),
        Some((mapped, refusal)) => {
            // The steps for the refused port, as the diagnosis gives them for Core's.
            let mut facts = checked.facts.clone();
            facts.port = mapped.port;
            if let RouterFacts::Found { mapping, .. } = &mut facts.router {
                *mapping = Err(refusal.clone());
            }
            check(
                "ports",
                CheckState::Fail,
                format!("the router refused to forward {}", mapped.what),
                details,
                diagnose(&facts).fix,
            )
        }
    }
}

/// The managed Java versions, and any server set to a Java that is gone.
async fn java(state: &CoreState) -> Result<DoctorCheck, ApiError> {
    let missing: Vec<_> = server_rows::list(&state.db, &state.engine)
        .await
        .map_err(internal)?
        .into_iter()
        .filter_map(|server| {
            let path = server.java_path?;
            (!Path::new(&path).is_file()).then_some(detail(server.name, path))
        })
        .collect();
    if let [server] = missing.as_slice() {
        return Ok(check(
            "java",
            CheckState::Fail,
            format!(
                "server \"{}\" runs a Java that no longer exists",
                server.label
            ),
            missing,
            vec!["Choose another Java for that server in Amberite.".into()],
        ));
    }
    if !missing.is_empty() {
        return Ok(check(
            "java",
            CheckState::Fail,
            format!(
                "{} servers run a Java that no longer exists",
                missing.len()
            ),
            missing,
            vec!["Choose another Java for those servers in Amberite.".into()],
        ));
    }
    let mut installed: Vec<_> = engine::jre::get_java_versions(&state.engine)
        .await
        .map_err(|error| internal(format!("{error:#}")))?
        .into_iter()
        .collect();
    installed.sort_by_key(|(major, _)| *major);
    Ok(java_installed(
        installed
            .into_iter()
            .map(|(major, java)| (major, java.version))
            .collect(),
    ))
}

fn java_installed(installed: Vec<(u32, String)>) -> DoctorCheck {
    if installed.is_empty() {
        return check(
            "java",
            CheckState::Ok,
            "downloads when a server first starts",
            vec![],
            vec![],
        );
    }
    let majors: Vec<String> = installed
        .iter()
        .map(|(major, _)| major.to_string())
        .collect();
    check(
        "java",
        CheckState::Ok,
        format!("Java {} installed", majors.join(", ")),
        installed
            .into_iter()
            .filter(|(major, version)| *version != major.to_string())
            .map(|(major, version)| detail(format!("Java {major}"), version))
            .collect(),
        vec![],
    )
}

/// Free space on the disk that holds `root`, the data folder. The CLI checks it, so it is
/// checked while Core is down too: a full disk is one reason a Core does not start.
pub fn disk_check(root: &Path) -> DoctorCheck {
    // Through its parts, so the path is shown with this OS's own separators.
    let shown: std::path::PathBuf = root.components().collect();
    disk(&shown, super::disk::space(root))
}

/// `space` is the disk's total and free bytes.
fn disk(root: &Path, space: std::io::Result<(u64, u64)>) -> DoctorCheck {
    let details = vec![detail("Data folder", root.display().to_string())];
    let (total, free) = match space {
        Ok(space) => space,
        Err(error) => {
            return check(
                "disk",
                CheckState::Warn,
                format!("could not read the disk: {error}"),
                details,
                vec![],
            );
        }
    };
    let state = if free < DISK_MARGIN_BYTES {
        CheckState::Fail
    } else if free < LOW_DISK {
        CheckState::Warn
    } else {
        CheckState::Ok
    };
    let fix = if state == CheckState::Ok {
        vec![]
    } else {
        vec!["See what takes the space with: copal storage".into()]
    };
    check(
        "disk",
        state,
        format!("{} free of {}", label(free), label(total)),
        details,
        fix,
    )
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::*;
    use crate::network::{
        diagnosis::{Facts, Outside},
        firewall::Firewall,
        forward::Mapped,
    };

    fn checked(router: RouterFacts, mappings: Vec<Mapped>) -> Checked {
        Checked {
            facts: Facts {
                what: "Core's store".into(),
                port: 16662,
                local_ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)),
                listening: true,
                firewall: Firewall::Open,
                router,
                outside: Outside::NotChecked("not tested".into()),
            },
            gateway: Some(Ipv4Addr::new(192, 168, 1, 1)),
            mappings,
        }
    }

    fn upnp(wan: &str, mapping: Result<u16, String>) -> RouterFacts {
        RouterFacts::Found {
            protocols: "UPnP".into(),
            wan_ip: Some(wan.parse().unwrap()),
            mapping,
            ports_tried: 1,
        }
    }

    fn mapped(
        what: &str,
        transport: Transport,
        outside: Result<u16, String>,
    ) -> Mapped {
        Mapped {
            what: what.into(),
            transport,
            port: 16662,
            outside,
        }
    }

    fn both(checked: &Checked) -> (DoctorCheck, DoctorCheck) {
        let router = router(Some(checked));
        let ports = ports(Some(checked), router.state);
        (router, ports)
    }

    #[test]
    fn a_router_that_forwards_lets_friends_join() {
        let granted = checked(
            upnp("203.0.113.9", Ok(16662)),
            vec![
                mapped("Core", Transport::Tcp, Ok(16662)),
                mapped("Core", Transport::Udp, Ok(40000)),
            ],
        );
        let (router, ports) = both(&granted);
        assert_eq!(router.state, CheckState::Ok);
        assert_eq!(
            router.summary,
            "answers UPnP, friends can join from the internet"
        );
        assert_eq!(router.details[2].value, "203.0.113.9");
        assert_eq!(ports.state, CheckState::Ok);
        assert_eq!(ports.details[1], detail("Core, UDP", "40000 -> 16662"));
    }

    #[test]
    fn routers_friends_cannot_join_through_fail_and_skip_the_ports() {
        for (router, summary) in [
            (
                RouterFacts::NoProtocol { gateway: None },
                "the router does not forward ports automatically",
            ),
            (upnp("192.168.0.14", Ok(16662)), "two routers in a row"),
            (
                upnp("100.72.3.4", Ok(16662)),
                "the internet provider shares",
            ),
        ] {
            let (router, ports) = both(&checked(router, vec![]));
            assert_eq!(router.state, CheckState::Fail);
            assert!(router.summary.starts_with(summary), "{}", router.summary);
            assert!(
                router
                    .summary
                    .ends_with("friends cannot join from the internet")
            );
            assert!(!router.fix.is_empty());
            assert_eq!(ports.state, CheckState::Skip);
        }
    }

    #[test]
    fn a_refused_port_fails_with_the_steps_for_that_port() {
        let refused = checked(
            upnp("203.0.113.9", Ok(16662)),
            vec![
                mapped("Core", Transport::Tcp, Ok(16662)),
                Mapped {
                    port: 25565,
                    ..mapped(
                        "server \"SMP\"",
                        Transport::Tcp,
                        Err("not authorized".into()),
                    )
                },
            ],
        );
        let (router, ports) = both(&refused);
        assert_eq!(router.state, CheckState::Ok);
        assert_eq!(ports.state, CheckState::Fail);
        assert_eq!(
            ports.summary,
            "the router refused to forward server \"SMP\""
        );
        assert_eq!(
            ports.details[1],
            detail("server \"SMP\", TCP", "refused: not authorized")
        );
        assert!(
            ports
                .fix
                .iter()
                .any(|step| step.contains("inside port 25565"))
        );
    }

    #[test]
    fn a_public_address_or_forwarding_off() {
        let (public, nothing) = both(&checked(RouterFacts::NoNat, vec![]));
        assert_eq!(
            (public.state, nothing.state),
            (CheckState::Ok, CheckState::Ok)
        );
        assert_eq!(router(None).state, CheckState::Skip);
        assert_eq!(ports(None, CheckState::Skip).state, CheckState::Skip);
    }

    #[test]
    fn the_disk_warns_when_low_and_fails_at_the_margin() {
        let gb = 1024 * 1024 * 1024;
        let at = |free| disk(Path::new("/data"), Ok((931 * gb, free)));
        assert_eq!(at(212 * gb).summary, "212.0 GB free of 931.0 GB");
        assert_eq!(at(212 * gb).state, CheckState::Ok);
        assert_eq!(at(4 * gb).state, CheckState::Warn);
        assert_eq!(at(DISK_MARGIN_BYTES - 1).state, CheckState::Fail);
        assert_eq!(at(DISK_MARGIN_BYTES - 1).fix.len(), 1);
    }
}
