//! This computer's own firewall. `copal setup` opens it once, as admin, for Core's port (TCP
//! for the store and UDP for Connect) and the server port range, and `copal uninstall` closes it
//! again. Core never runs a firewall tool after that, so it needs no permission to. `copal doctor`
//! checks the rules are still there, and `copal doctor --fix` opens it again.
//!
//! Windows: one inbound rule named "Amberite Core", for private networks only, so public Wi-Fi
//! stays closed. Linux: ufw and firewalld rules. macOS: the application firewall decides per
//! program and is off by default; install leaves it alone.

use std::{ops::RangeInclusive, process::Command};

use color_eyre::eyre::{Result, bail};

/// Ports Core hands to servers, lowest first. The firewall is opened for all of them at install.
pub const SERVER_PORTS: RangeInclusive<u16> = 25565..=25664;

/// What the firewall does with a port, as far as the reachability diagnosis can tell.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Firewall {
    /// No firewall is on, or it allows the port.
    #[default]
    Open,
    /// A firewall is on and does not allow the port. `fix` is the steps to open it. Core never
    /// runs a firewall tool, so it cannot tell yet; the diagnosis is ready for when it can.
    #[allow(dead_code)]
    Blocked { found: String, fix: Vec<String> },
    /// A firewall is on and Core cannot tell whether it allows the port.
    Unsure { found: String, fix: Vec<String> },
}

/// Allow Core's port and the server port range in. Returns what was changed, to print. Running
/// it again replaces the rules it made.
pub fn open(core_port: u16) -> Result<Vec<String>> {
    platform::open(core_port)
}

/// Whether this computer's firewall still has what [`open`] adds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rules {
    Open,
    /// `found` is one sentence, ready to print.
    Missing {
        found: String,
    },
    /// `why` is ready to print.
    Unknown {
        why: String,
    },
}

/// Check the firewall for what [`open`] adds: TCP for `core_port` and [`SERVER_PORTS`], and
/// UDP for `core_port`. Needs no admin on Windows; on Linux only root can read the firewall.
pub fn check(core_port: u16) -> Rules {
    platform::check(core_port)
}

/// The ports a firewall lets in, by protocol.
#[derive(Debug, Default)]
#[cfg_attr(not(any(windows, target_os = "linux", test)), allow(dead_code))]
struct Allowed {
    tcp: Vec<RangeInclusive<u16>>,
    udp: Vec<RangeInclusive<u16>>,
}

#[cfg_attr(not(any(windows, target_os = "linux", test)), allow(dead_code))]
impl Allowed {
    /// What [`open`] adds that is not allowed, ready to print: `UDP 16662`.
    fn missing(&self, core_port: u16) -> Vec<String> {
        let covers = |list: &[RangeInclusive<u16>],
                      mut ports: RangeInclusive<u16>| {
            ports.all(|port| list.iter().any(|range| range.contains(&port)))
        };
        let mut missing = Vec::new();
        if !SERVER_PORTS.contains(&core_port)
            && !covers(&self.tcp, core_port..=core_port)
        {
            missing.push(format!("TCP {core_port}"));
        }
        if !covers(&self.tcp, SERVER_PORTS) {
            missing.push(format!(
                "TCP {}-{}",
                SERVER_PORTS.start(),
                SERVER_PORTS.end()
            ));
        }
        if !covers(&self.udp, core_port..=core_port) {
            missing.push(format!("UDP {core_port}"));
        }
        missing
    }

    /// [`Rules::Open`], or what `who` misses in one sentence.
    fn rules(&self, who: &str, core_port: u16) -> Rules {
        let missing = self.missing(core_port);
        if missing.is_empty() {
            return Rules::Open;
        }
        let tcp_complete = missing.iter().all(|port| port.starts_with("UDP"));
        Rules::Missing {
            found: if tcp_complete {
                format!("{who} allows TCP but not {}", missing.join(" or "))
            } else {
                format!("{who} does not allow {}", missing.join(" or "))
            },
        }
    }
}

/// Port ranges as firewalls write them: `16662`, `25565-25664`, `25565:25664`, or a list of them
/// with commas. Words such as `RPC` are skipped.
#[cfg_attr(not(any(windows, target_os = "linux", test)), allow(dead_code))]
fn port_ranges(spec: &str) -> impl Iterator<Item = RangeInclusive<u16>> {
    spec.split(',').filter_map(|part| {
        let (low, high) = part.split_once(['-', ':']).unwrap_or((part, part));
        Some(low.trim().parse().ok()?..=high.trim().parse().ok()?)
    })
}

/// The rule [`open`] adds on Windows.
#[cfg(any(windows, test))]
const WINDOWS_RULE: &str = "Amberite Core";

/// What the active inbound "Amberite Core" rules allow, from rule strings as the registry keeps
/// them: `v2.30|Action=Allow|Active=TRUE|Dir=In|Protocol=17|LPort=16662|Name=Amberite Core|`.
/// `None` when there is no such rule. The registry's form does not change with the display
/// language, unlike `netsh`'s output.
#[cfg(any(windows, test))]
fn windows_allowed(rules: &[String]) -> Option<Allowed> {
    let mut found = false;
    let mut allowed = Allowed::default();
    for rule in rules {
        let fields: Vec<(&str, &str)> = rule
            .split('|')
            .filter_map(|field| field.split_once('='))
            .collect();
        let field = |key: &str| {
            fields
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| *value)
        };
        if field("Name") != Some(WINDOWS_RULE) {
            continue;
        }
        found = true;
        if field("Action") != Some("Allow")
            || field("Active") != Some("TRUE")
            || field("Dir") != Some("In")
        {
            continue;
        }
        let list = match field("Protocol") {
            Some("6") => &mut allowed.tcp,
            Some("17") => &mut allowed.udp,
            _ => continue,
        };
        // A rule with many ports keeps them in several fields: `LPort`, `LPort2_10`, ...
        for (name, value) in &fields {
            if name.starts_with("LPort") {
                list.extend(port_ranges(value));
            }
        }
    }
    found.then_some(allowed)
}

/// What `ufw status` lists as allowed in, or `None` while ufw is inactive.
#[cfg(any(target_os = "linux", test))]
fn ufw_allowed(status: &str) -> Option<Allowed> {
    if !status.lines().any(|line| line.trim() == "Status: active") {
        return None;
    }
    let mut allowed = Allowed::default();
    for line in status.lines() {
        let mut words = line.split_whitespace();
        let (Some(to), Some(action)) = (words.next(), words.next()) else {
            continue;
        };
        if !action.starts_with("ALLOW") {
            continue;
        }
        // `ALLOW` and `ALLOW IN` let traffic in, `ALLOW OUT` and `ALLOW FWD` do not; and only a
        // rule from `Anywhere` lets everyone in.
        let from = match words.next() {
            Some("IN") => words.next(),
            from => from,
        };
        if !from.is_some_and(|from| from.starts_with("Anywhere")) {
            continue;
        }
        let (ports, protocol) = to.split_once('/').unwrap_or((to, ""));
        for range in port_ranges(ports) {
            if protocol != "udp" {
                allowed.tcp.push(range.clone());
            }
            if protocol != "tcp" {
                allowed.udp.push(range);
            }
        }
    }
    Some(allowed)
}

/// What `firewall-cmd --list-ports` lists: `16662/tcp 16662/udp 25565-25664/tcp`.
#[cfg(any(target_os = "linux", test))]
fn firewalld_allowed(ports: &str) -> Allowed {
    let mut allowed = Allowed::default();
    for entry in ports.split_whitespace() {
        let Some((ports, protocol)) = entry.split_once('/') else {
            continue;
        };
        let list = match protocol {
            "tcp" => &mut allowed.tcp,
            "udp" => &mut allowed.udp,
            _ => continue,
        };
        list.extend(port_ranges(ports));
    }
    allowed
}

/// Remove what `open` added. Rules that are already gone are skipped.
pub fn close(core_port: u16) -> Result<Vec<String>> {
    platform::close(core_port)
}

/// Run a program that must succeed.
fn run(program: &str, args: &[&str]) -> Result<()> {
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() {
        bail!(
            "{program} {} failed: {}{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(windows)]
mod platform {
    use std::process::Command;

    use color_eyre::eyre::Result;

    use windows_sys::Win32::System::Registry::HKEY_LOCAL_MACHINE;

    use super::{
        Rules, SERVER_PORTS, WINDOWS_RULE as RULE, run, windows_allowed,
    };

    /// Where Windows Firewall keeps its rules. Any account may read it.
    const RULES_KEY: &str = r"SYSTEM\CurrentControlSet\Services\SharedAccess\Parameters\FirewallPolicy\FirewallRules";

    pub fn check(core_port: u16) -> Rules {
        match crate::daemon::win::registry_strings(
            HKEY_LOCAL_MACHINE,
            RULES_KEY,
        ) {
            Err(error) => Rules::Unknown {
                why: format!(
                    "could not read Windows Firewall's rules: {error}"
                ),
            },
            Ok(rules) => match windows_allowed(&rules) {
                None => Rules::Missing {
                    found: format!("Windows Firewall has no \"{RULE}\" rule"),
                },
                Some(allowed) => {
                    allowed.rules(&format!("the \"{RULE}\" rule"), core_port)
                }
            },
        }
    }

    pub fn open(core_port: u16) -> Result<Vec<String>> {
        let _ = close(core_port);
        let ports = if SERVER_PORTS.contains(&core_port) {
            format!("{}-{}", SERVER_PORTS.start(), SERVER_PORTS.end())
        } else {
            format!(
                "{core_port},{}-{}",
                SERVER_PORTS.start(),
                SERVER_PORTS.end()
            )
        };
        run(
            "netsh",
            &[
                "advfirewall",
                "firewall",
                "add",
                "rule",
                &format!("name={RULE}"),
                "dir=in",
                "action=allow",
                "protocol=TCP",
                &format!("localport={ports}"),
                "profile=private",
            ],
        )?;
        // A second rule under the same name, so `close` removes both.
        run(
            "netsh",
            &[
                "advfirewall",
                "firewall",
                "add",
                "rule",
                &format!("name={RULE}"),
                "dir=in",
                "action=allow",
                "protocol=UDP",
                &format!("localport={core_port}"),
                "profile=private",
            ],
        )?;
        Ok(vec![format!(
            "Windows Firewall: rule \"{RULE}\" allows TCP {ports} and UDP {core_port}"
        )])
    }

    pub fn close(_core_port: u16) -> Result<Vec<String>> {
        let deleted = Command::new("netsh")
            .args(["advfirewall", "firewall", "delete", "rule"])
            .arg(format!("name={RULE}"))
            .output()?
            .status
            .success();
        Ok(if deleted {
            vec![format!("Windows Firewall: removed rule \"{RULE}\"")]
        } else {
            Vec::new()
        })
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::process::Command;

    use color_eyre::eyre::Result;

    use super::{
        Allowed, Rules, SERVER_PORTS, firewalld_allowed, run, ufw_allowed,
    };

    /// What a program printed, in English, when it succeeded.
    fn stdout(program: &str, args: &[&str]) -> Option<String> {
        let output = Command::new(program)
            .args(args)
            .env("LC_ALL", "C")
            .output()
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
    }

    pub fn check(core_port: u16) -> Rules {
        if !nix::unistd::geteuid().is_root() {
            return Rules::Unknown {
                why: "only root can read the firewall".into(),
            };
        }
        let mut found: Vec<(&str, Allowed)> = Vec::new();
        if let Some(allowed) =
            stdout("ufw", &["status"]).as_deref().and_then(ufw_allowed)
        {
            found.push(("ufw", allowed));
        }
        if firewalld() {
            // A firewall that runs and will not say what it lets in is not an open one.
            let Some(ports) = stdout("firewall-cmd", &["--list-ports"]) else {
                return Rules::Unknown {
                    why: "firewalld would not list its ports".into(),
                };
            };
            found.push(("firewalld", firewalld_allowed(&ports)));
        }
        found
            .iter()
            .map(|(who, allowed)| allowed.rules(who, core_port))
            .find(|rules| *rules != Rules::Open)
            .unwrap_or(Rules::Open)
    }

    /// ufw takes rules while it is off, so they apply when someone turns it on.
    fn ufw() -> bool {
        Command::new("ufw").arg("version").output().is_ok()
    }

    /// firewalld only takes rules while it runs.
    fn firewalld() -> bool {
        Command::new("firewall-cmd")
            .arg("--state")
            .output()
            .is_ok_and(|output| output.status.success())
    }

    pub fn open(core_port: u16) -> Result<Vec<String>> {
        let mut done = Vec::new();
        let range = format!("{}:{}", SERVER_PORTS.start(), SERVER_PORTS.end());
        if ufw() {
            for ports in [core_port.to_string(), range.clone()] {
                run(
                    "ufw",
                    &["allow", &format!("{ports}/tcp"), "comment", "Copal"],
                )?;
                done.push(format!("ufw: allowed TCP {ports}"));
            }
            run(
                "ufw",
                &["allow", &format!("{core_port}/udp"), "comment", "Copal"],
            )?;
            done.push(format!("ufw: allowed UDP {core_port}"));
        }
        if firewalld() {
            let range = range.replace(':', "-");
            run(
                "firewall-cmd",
                &[
                    "--permanent",
                    &format!("--add-port={core_port}/tcp"),
                    &format!("--add-port={core_port}/udp"),
                    &format!("--add-port={range}/tcp"),
                ],
            )?;
            run("firewall-cmd", &["--reload"])?;
            done.push(format!(
                "firewalld: allowed TCP {core_port} and {range}, and UDP {core_port}"
            ));
        }
        Ok(done)
    }

    pub fn close(core_port: u16) -> Result<Vec<String>> {
        let mut done = Vec::new();
        let range = format!("{}:{}", SERVER_PORTS.start(), SERVER_PORTS.end());
        if ufw() {
            for ports in [core_port.to_string(), range.clone()] {
                if run("ufw", &["delete", "allow", &format!("{ports}/tcp")])
                    .is_ok()
                {
                    done.push(format!("ufw: removed TCP {ports}"));
                }
            }
            if run("ufw", &["delete", "allow", &format!("{core_port}/udp")])
                .is_ok()
            {
                done.push(format!("ufw: removed UDP {core_port}"));
            }
        }
        if firewalld() {
            let range = range.replace(':', "-");
            let removed = run(
                "firewall-cmd",
                &[
                    "--permanent",
                    &format!("--remove-port={core_port}/tcp"),
                    &format!("--remove-port={core_port}/udp"),
                    &format!("--remove-port={range}/tcp"),
                ],
            );
            run("firewall-cmd", &["--reload"])?;
            if removed.is_ok() {
                done.push(format!(
                    "firewalld: removed TCP {core_port} and {range}, and UDP {core_port}"
                ));
            }
        }
        Ok(done)
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod platform {
    use color_eyre::eyre::Result;

    use super::Rules;

    /// Setup leaves the application firewall alone.
    pub fn check(_core_port: u16) -> Rules {
        Rules::Open
    }

    pub fn open(_core_port: u16) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    pub fn close(_core_port: u16) -> Result<Vec<String>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(protocol: u8, ports: &str) -> String {
        format!(
            "v2.33|Action=Allow|Active=TRUE|Dir=In|Protocol={protocol}|Profile=Private|{ports}|Name=Amberite Core|"
        )
    }

    fn windows(rules: &[String]) -> Rules {
        windows_allowed(rules).map_or(
            Rules::Missing {
                found: "none".into(),
            },
            |allowed| allowed.rules("the rule", 16662),
        )
    }

    #[test]
    fn a_windows_rule_without_udp_is_found_missing() {
        // As a Core installed before Connect left it, ports split over two fields.
        let tcp = rule(6, "LPort=16662|LPort2_10=25565-25664");
        assert_eq!(
            windows(std::slice::from_ref(&tcp)),
            Rules::Missing {
                found: "the rule allows TCP but not UDP 16662".into()
            }
        );
        assert_eq!(windows(&[tcp, rule(17, "LPort=16662")]), Rules::Open);
    }

    #[test]
    fn only_active_inbound_amberite_rules_count() {
        let others = [
            rule(17, "LPort=16662").replace("Active=TRUE", "Active=FALSE"),
            rule(6, "LPort=16662,25565-25664").replace("Amberite Core", "Other"),
            "v2.33|Action=Allow|Active=TRUE|Dir=In|Protocol=6|LPort=RPC|Name=Amberite Core|"
                .into(),
        ];
        assert_eq!(
            windows(&others),
            Rules::Missing {
                found: "the rule does not allow TCP 16662 or TCP 25565-25664 or UDP 16662"
                    .into()
            }
        );
        assert!(windows_allowed(&others[1..2]).is_none());
    }

    #[test]
    fn ufw_and_firewalld_lists_are_read() {
        let status = "Status: active\n\nTo                         Action      From\n--                         ------      ----\n16662/tcp                  ALLOW       Anywhere                   # Copal\n25565:25664/tcp            ALLOW       Anywhere\n16662/tcp (v6)             ALLOW       Anywhere (v6)\n";
        assert_eq!(
            ufw_allowed(status).unwrap().missing(16662),
            vec!["UDP 16662".to_string()]
        );
        // A rule that lets traffic out lets nothing in, and neither does one for a single
        // address.
        let out = format!(
            "{status}16662/udp                  ALLOW OUT   Anywhere\n16662/udp                  ALLOW       192.0.2.10\n"
        );
        assert_eq!(
            ufw_allowed(&out).unwrap().missing(16662),
            vec!["UDP 16662".to_string()]
        );
        // A rule with no protocol allows both.
        let both = status.replace("16662/tcp ", "16662     ");
        assert!(ufw_allowed(&both).unwrap().missing(16662).is_empty());
        assert!(ufw_allowed("Status: inactive\n").is_none());
        assert!(
            firewalld_allowed("16662/tcp 16662/udp 25565-25664/tcp\n")
                .missing(16662)
                .is_empty()
        );
    }
}
