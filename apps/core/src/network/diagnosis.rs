//! Turns what Core measured about one port into a verdict in plain words: which case it is, what
//! was tested, what was found, and what the owner can do about it.
//!
//! This is pure. The measuring happens in the parent module; everything here can be tested
//! without a router.

#![allow(dead_code)] // Wired in step 8: reachability reporting over the API.

use std::{
    fmt::Write as _,
    net::{IpAddr, Ipv4Addr},
};

use super::{firewall::Firewall, is_private};

/// Everything measured about one port.
#[derive(Debug, Clone)]
pub struct Facts {
    /// What the port belongs to, ready to print: `Minecraft server "SMP"`.
    pub what: String,
    /// The port on this computer.
    pub port: u16,
    pub local_ip: IpAddr,
    /// Whether something on this computer accepts connections on the port.
    pub listening: bool,
    pub firewall: Firewall,
    pub router: RouterFacts,
    pub outside: Outside,
}

#[derive(Debug, Clone)]
pub enum RouterFacts {
    /// This computer's own address is a public one. There is nothing to forward.
    NoNat,
    /// Nothing answered PCP, NAT-PMP or UPnP.
    NoProtocol { gateway: Option<Ipv4Addr> },
    Found {
        /// Protocols the router answered, ready to print: `UPnP`.
        protocols: String,
        /// The router's address on its internet side.
        wan_ip: Option<IpAddr>,
        /// The outside port the router granted, or its refusal in words.
        mapping: Result<u16, String>,
        /// How many different outside ports were tried.
        ports_tried: u8,
    },
}

/// The result of asking the backend to connect from the internet.
#[derive(Debug, Clone)]
pub enum Outside {
    Reachable {
        ip: IpAddr,
        port: u16,
    },
    Failed {
        ip: IpAddr,
        port: u16,
        how: String,
    },
    /// The test could not be run. The reason is ready to print.
    NotChecked(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Case {
    Reachable,
    /// Forwarded, but nobody could test it from the internet.
    Unverified,
    NotListening,
    Firewall,
    /// The router supports no forwarding protocol, or has it switched off.
    NoProtocol,
    /// The router refused the mapping.
    Refused,
    /// Two routers in a row.
    DoubleNat,
    /// The internet provider shares one address between customers.
    SharedAddress,
    /// The router forwards, but connections from the internet do not arrive.
    ProviderBlocks,
    /// A public address with something in front of it that blocks the port.
    PublicBlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Pass,
    Fail,
    /// The test was not run or could not tell.
    Skip,
}

#[derive(Debug, Clone)]
pub struct Check {
    pub mark: Mark,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct Diagnosis {
    pub case: Case,
    /// The cause in one line. Also what the App shows on the server's status indicator.
    pub title: String,
    /// Each test that was run, in order, with what it found.
    pub checks: Vec<Check>,
    pub found: Vec<String>,
    pub fix: Vec<String>,
}

impl Diagnosis {
    /// Whether the port can be used from the internet as far as Core can tell.
    pub fn usable(&self) -> bool {
        matches!(self.case, Case::Reachable | Case::Unverified)
    }

    /// The full report for the Core's terminal.
    pub fn render(&self, facts: &Facts) -> String {
        let heading = match self.case {
            Case::Reachable => "Reachable from the internet",
            Case::Unverified => "Forwarded, not yet proven",
            _ => "NOT reachable from the internet",
        };
        let mut out = format!(
            "\n==== {heading}: {} (port {}) ====\n\n  {}\n",
            facts.what, facts.port, self.title
        );
        out.push_str("\n  What was tested\n");
        for check in &self.checks {
            let mark = match check.mark {
                Mark::Pass => "ok  ",
                Mark::Fail => "FAIL",
                Mark::Skip => "--  ",
            };
            let _ = writeln!(out, "    [{mark}] {}", check.text);
        }
        if !self.found.is_empty() {
            out.push_str("\n  What was found\n");
            for line in &self.found {
                let _ = writeln!(out, "    {line}");
            }
        }
        if !self.fix.is_empty() {
            out.push_str("\n  How to fix it\n");
            for (index, step) in self.fix.iter().enumerate() {
                let _ = writeln!(out, "    {}. {step}", index + 1);
            }
        }
        out
    }
}

/// 100.64.0.0/10, the range internet providers use when customers share an address.
fn is_shared_range(ip: IpAddr) -> bool {
    matches!(ip, IpAddr::V4(v4) if v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1]))
}

/// Decide which case `facts` describe.
pub fn diagnose(facts: &Facts) -> Diagnosis {
    let port = facts.port;
    let local = facts.local_ip;
    let mut checks = Vec::new();
    let mut check = |mark, text: String| checks.push(Check { mark, text });

    check(
        if facts.listening {
            Mark::Pass
        } else {
            Mark::Fail
        },
        format!(
            "Connect to port {port} on this computer: {}",
            if facts.listening {
                "something answers"
            } else {
                "nothing answers"
            }
        ),
    );
    match &facts.firewall {
        Firewall::Open => check(
            Mark::Pass,
            "This computer's firewall: allows the port".into(),
        ),
        Firewall::Blocked { found, .. } => {
            check(Mark::Fail, format!("This computer's firewall: {found}"))
        }
        Firewall::Unsure { found, .. } => {
            check(Mark::Skip, format!("This computer's firewall: {found}"))
        }
    }
    let manual_forward = format!(
        "Or forward the port by hand in the router's settings: protocol TCP, outside port {port}, to address {local}, inside port {port}."
    );
    match &facts.router {
        RouterFacts::NoNat => check(
            Mark::Pass,
            format!(
                "This computer's address {local} is a public one: no router to ask"
            ),
        ),
        RouterFacts::NoProtocol { gateway } => check(
            Mark::Fail,
            format!(
                "Ask the router{} to forward ports, over PCP, NAT-PMP and UPnP: no answer to any of them",
                gateway.map(|ip| format!(" at {ip}")).unwrap_or_default()
            ),
        ),
        RouterFacts::Found {
            protocols,
            wan_ip,
            mapping,
            ports_tried,
        } => {
            check(
                Mark::Pass,
                format!(
                    "Ask the router to forward ports: it answers over {protocols}"
                ),
            );
            match wan_ip {
                Some(ip) => check(
                    if is_private(*ip) || is_shared_range(*ip) {
                        Mark::Fail
                    } else {
                        Mark::Pass
                    },
                    format!("Ask the router for its internet address: {ip}"),
                ),
                None => check(
                    Mark::Skip,
                    "Ask the router for its internet address: it did not say"
                        .into(),
                ),
            }
            match mapping {
                Ok(external) => check(
                    Mark::Pass,
                    format!(
                        "Ask the router to forward outside port {external} to {local}:{port}: granted"
                    ),
                ),
                Err(refusal) => check(
                    Mark::Fail,
                    format!(
                        "Ask the router to forward a port to {local}:{port}: refused after {ports_tried} different outside port(s) ({refusal})"
                    ),
                ),
            }
        }
    }
    match &facts.outside {
        Outside::Reachable { ip, port } => check(
            Mark::Pass,
            format!(
                "Amberite connects to {ip}:{port} from the internet: connected"
            ),
        ),
        Outside::Failed { ip, port, how } => check(
            Mark::Fail,
            format!(
                "Amberite connects to {ip}:{port} from the internet: {how}"
            ),
        ),
        Outside::NotChecked(why) => check(
            Mark::Skip,
            format!("Amberite connects from the internet: not run, {why}"),
        ),
    }

    let (case, title, found, fix): (Case, String, Vec<String>, Vec<String>) =
        decide(facts, &manual_forward);
    Diagnosis {
        case,
        title,
        checks,
        found,
        fix,
    }
}

type Verdict = (Case, String, Vec<String>, Vec<String>);

fn decide(facts: &Facts, manual_forward: &str) -> Verdict {
    let port = facts.port;
    if !facts.listening {
        return (
            Case::NotListening,
            "The server is running but is not listening on its port.".into(),
            vec![format!(
                "Nothing accepted a connection on port {port} on this computer, so nobody can join, from inside or outside."
            )],
            vec![
                "Look at the server's console for an error about the port (\"failed to bind\"). Another program may be using it.".into(),
                format!("Check that server-port in the server's server.properties is {port}."),
                "Restart the server. Core checks again when it is running.".into(),
            ],
        );
    }
    if let Outside::Reachable { ip, port } = &facts.outside {
        return (
            Case::Reachable,
            format!("Reachable at {ip}:{port}."),
            vec![],
            vec![],
        );
    }
    if let Firewall::Blocked { found, fix } = &facts.firewall {
        return (
            Case::Firewall,
            "This computer's firewall is blocking the port.".into(),
            vec![
                found.clone(),
                "Connections are stopped on this computer before they reach the server.".into(),
            ],
            fix.clone(),
        );
    }
    let outside_ip = match &facts.outside {
        Outside::Failed { ip, .. } => Some(*ip),
        _ => None,
    };
    let firewall_note = match &facts.firewall {
        Firewall::Unsure { fix, .. } => fix.clone(),
        _ => vec![],
    };
    match &facts.router {
        RouterFacts::NoNat => match &facts.outside {
            Outside::Failed { how, .. } => (
                Case::PublicBlocked,
                "This computer has a public address, but something in front of it blocks the port.".into(),
                vec![format!(
                    "This computer needs no port forwarding, yet a connection from the internet to port {port} failed ({how}). A firewall run by the hosting or internet provider is the usual cause."
                )],
                [
                    vec![format!(
                        "In the provider's control panel, allow incoming TCP on port {port} (often called a firewall rule or security group)."
                    )],
                    firewall_note,
                ]
                .concat(),
            ),
            _ => unverified(facts),
        },
        RouterFacts::NoProtocol { gateway } => {
            let page = gateway
                .map(|ip| format!(" (usually http://{ip})"))
                .unwrap_or_default();
            (
                Case::NoProtocol,
                "The router does not allow automatic port forwarding.".into(),
                vec![
                    "The router answered none of the three protocols a program can use to open a port (PCP, NAT-PMP, UPnP). Either it does not support them or the setting is switched off.".into(),
                ],
                vec![
                    format!(
                        "Open the router's settings page{page} and switch on UPnP. Some routers call it NAT-PMP or port mapping. Core notices within a few minutes and opens the port by itself."
                    ),
                    manual_forward.into(),
                    "If you cannot change the router (a shared building, a mobile hotspot, a router run by the provider), nothing on this computer can fix it. Ask whoever runs the router to do one of the two steps above.".into(),
                ],
            )
        }
        RouterFacts::Found {
            wan_ip, mapping, ..
        } => {
            if let Some(wan) = wan_ip.filter(|ip| is_shared_range(*ip)) {
                return shared_address(format!(
                    "The router's internet address is {wan}, which is in the range providers use when many customers share one public address."
                ));
            }
            if let Some(wan) = wan_ip.filter(|ip| is_private(*ip)) {
                return (
                    Case::DoubleNat,
                    "There are two routers in a row, and only the inner one can be opened automatically.".into(),
                    vec![format!(
                        "The router Core talks to has the internet address {wan}. That is a private address, so a second router (often the provider's box) sits between it and the internet and still blocks the port."
                    )],
                    vec![
                        "Set the provider's box to bridge mode (also called modem mode), so only one router is left. Core then opens the port by itself.".into(),
                        format!(
                            "Or, in the provider's box, forward TCP port {port} to {wan}. Core keeps the inner router open."
                        ),
                        "If the outer box is not yours to change, nothing on this computer can fix it.".into(),
                    ],
                );
            }
            if let (Some(wan), Some(seen)) = (wan_ip, outside_ip)
                && *wan != seen {
                    return shared_address(format!(
                        "The router says its internet address is {wan}, but the internet sees this computer as {seen}. Something between them rewrites the address: usually the provider sharing addresses, sometimes a VPN on this computer."
                    ));
                }
            match (mapping, &facts.outside) {
                (Err(refusal), _) => (
                    Case::Refused,
                    "The router refused to open the port.".into(),
                    vec![
                        format!("The router supports port forwarding but answered: {refusal}"),
                        "Usual causes: forwarding is set to read-only or \"secure mode\", the router only accepts requests from some devices, or its table of forwarded ports is full.".into(),
                    ],
                    vec![
                        "In the router's UPnP settings, allow devices to add port mappings (switch off \"secure mode\" or \"read-only\" if present), then wait a few minutes.".into(),
                        "Restart the router. That empties its list of forwarded ports.".into(),
                        manual_forward.into(),
                    ],
                ),
                (Ok(external), Outside::Failed { how, .. }) => (
                    Case::ProviderBlocks,
                    "The router opened the port, but connections from the internet do not arrive.".into(),
                    vec![
                        format!(
                            "The router granted outside port {external}, and the address matches what the internet sees, yet the connection failed ({how})."
                        ),
                        "Usual causes: the internet provider blocks incoming connections on this plan, or the router reports success without applying it.".into(),
                    ],
                    [
                        firewall_note,
                        vec![
                            "Restart the router, then wait a few minutes for Core to try again.".into(),
                            "Check that the router's own firewall is not set to block all incoming connections.".into(),
                            "Ask the internet provider whether incoming connections are blocked on your plan. If they are, nothing on your side can fix it.".into(),
                        ],
                    ]
                    .concat(),
                ),
                _ => unverified(facts),
            }
        }
    }
}

fn shared_address(found: String) -> Verdict {
    (
        Case::SharedAddress,
        "The internet provider shares one public address between customers, so no port can be opened.".into(),
        vec![
            found,
            "Incoming connections stop at the provider's equipment and never reach the router.".into(),
        ],
        vec![
            "Nothing on this computer or the router can fix it.".into(),
            "Ask the internet provider for a public IPv4 address. Some call it a static IP or opting out of CGNAT; it is often free or a small monthly fee.".into(),
            "If a VPN is running on this computer, turn it off: it has the same effect.".into(),
        ],
    )
}

fn unverified(facts: &Facts) -> Verdict {
    let why = match &facts.outside {
        Outside::NotChecked(why) => why.clone(),
        _ => String::new(),
    };
    (
        Case::Unverified,
        "The port is open as far as Core can tell, but it was not tested from the internet.".into(),
        vec![format!("The outside test did not run: {why}.")],
        vec![],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    fn found(wan: &str, mapping: Result<u16, String>) -> RouterFacts {
        RouterFacts::Found {
            protocols: "UPnP".into(),
            wan_ip: Some(ip(wan)),
            mapping,
            ports_tried: 1,
        }
    }

    fn failed(seen: &str) -> Outside {
        Outside::Failed {
            ip: ip(seen),
            port: 25565,
            how: "timed out".into(),
        }
    }

    fn facts(router: RouterFacts, outside: Outside) -> Facts {
        Facts {
            what: "Minecraft server \"SMP\"".into(),
            port: 25565,
            local_ip: ip("192.168.1.20"),
            listening: true,
            firewall: Firewall::Open,
            router,
            outside,
        }
    }

    fn case(router: RouterFacts, outside: Outside) -> Case {
        diagnose(&facts(router, outside)).case
    }

    #[test]
    fn a_server_that_does_not_listen_is_reported_before_anything_else() {
        let mut facts = facts(
            RouterFacts::NoProtocol { gateway: None },
            failed("203.0.113.9"),
        );
        facts.listening = false;
        assert_eq!(diagnose(&facts).case, Case::NotListening);
    }

    #[test]
    fn a_passing_outside_test_wins_over_every_other_finding() {
        let mut facts = facts(
            RouterFacts::NoProtocol { gateway: None },
            Outside::Reachable {
                ip: ip("203.0.113.9"),
                port: 25565,
            },
        );
        facts.firewall = Firewall::Blocked {
            found: "blocked".into(),
            fix: vec![],
        };
        assert_eq!(diagnose(&facts).case, Case::Reachable);
    }

    #[test]
    fn a_blocking_local_firewall_is_named_before_the_router() {
        let mut facts =
            facts(found("203.0.113.9", Ok(25565)), failed("203.0.113.9"));
        facts.firewall = Firewall::Blocked {
            found: "Windows Firewall has no rule for port 25565".into(),
            fix: vec!["run this".into()],
        };
        let diagnosis = diagnose(&facts);
        assert_eq!(diagnosis.case, Case::Firewall);
        assert_eq!(diagnosis.fix, vec!["run this".to_string()]);
    }

    #[test]
    fn a_router_that_answers_no_protocol() {
        assert_eq!(
            case(
                RouterFacts::NoProtocol {
                    gateway: Some(Ipv4Addr::new(192, 168, 1, 1))
                },
                Outside::NotChecked("this Core is not signed in".into())
            ),
            Case::NoProtocol
        );
    }

    #[test]
    fn a_router_that_refuses_the_mapping() {
        assert_eq!(
            case(
                found("203.0.113.9", Err("UPnP: not authorized".into())),
                failed("203.0.113.9")
            ),
            Case::Refused
        );
    }

    #[test]
    fn a_private_router_address_means_two_routers() {
        assert_eq!(
            case(found("192.168.0.14", Ok(25565)), failed("203.0.113.9")),
            Case::DoubleNat
        );
        // Still two routers when the inner one refused: the outer one is the real blocker.
        assert_eq!(
            case(
                found("10.0.0.2", Err("refused".into())),
                Outside::NotChecked("not signed in".into())
            ),
            Case::DoubleNat
        );
    }

    #[test]
    fn a_shared_range_or_a_mismatched_address_means_a_shared_address() {
        assert_eq!(
            case(found("100.72.3.4", Ok(25565)), failed("203.0.113.9")),
            Case::SharedAddress
        );
        assert_eq!(
            case(found("198.51.100.7", Ok(25565)), failed("203.0.113.9")),
            Case::SharedAddress
        );
    }

    #[test]
    fn a_granted_mapping_that_cannot_be_reached_blames_the_provider() {
        assert_eq!(
            case(found("203.0.113.9", Ok(25565)), failed("203.0.113.9")),
            Case::ProviderBlocks
        );
    }

    #[test]
    fn a_granted_mapping_nobody_tested_is_unverified_and_usable() {
        let diagnosis = diagnose(&facts(
            found("203.0.113.9", Ok(25565)),
            Outside::NotChecked("this Core is not signed in".into()),
        ));
        assert_eq!(diagnosis.case, Case::Unverified);
        assert!(diagnosis.usable());
    }

    #[test]
    fn a_public_address_that_is_blocked() {
        assert_eq!(
            case(RouterFacts::NoNat, failed("203.0.113.9")),
            Case::PublicBlocked
        );
    }

    #[test]
    fn the_report_names_what_was_tested_found_and_how_to_fix_it() {
        let facts = facts(
            RouterFacts::NoProtocol {
                gateway: Some(Ipv4Addr::new(192, 168, 1, 1)),
            },
            Outside::NotChecked("this Core is not signed in".into()),
        );
        let report = diagnose(&facts).render(&facts);
        assert!(report.contains("NOT reachable from the internet: Minecraft server \"SMP\" (port 25565)"));
        assert!(report.contains("[FAIL] Ask the router at 192.168.1.1"));
        assert!(report.contains("What was found"));
        assert!(report.contains(
            "1. Open the router's settings page (usually http://192.168.1.1)"
        ));
        assert!(report.contains("to address 192.168.1.20, inside port 25565"));
    }
}
