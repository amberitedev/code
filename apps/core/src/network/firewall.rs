//! Opening this computer's own firewall for Core and the servers it runs.
//!
//! Windows: one inbound rule named "Amberite Core" covers Core's port and a range of server
//! ports, so it is added once. Adding it needs administrator rights; Core asks with the normal
//! Windows prompt a single time and never again, whatever the answer.
//! Linux: ufw and firewalld are handled. Core opens ports itself when it runs as root and
//! otherwise says which command to run. Hand-written nftables or iptables rules are not read.
//! macOS: the firewall is off by default; when it is on, Core says where to allow the server.

use std::path::Path;

#[cfg_attr(not(windows), allow(dead_code))]
/// Server ports covered by the one Windows rule. Core hands out ports upward from 25565.
const SERVER_PORTS: std::ops::RangeInclusive<u16> = 25565..=25664;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Firewall {
    /// No firewall is on, or it allows the ports.
    #[default]
    Open,
    /// A firewall is on and does not allow a port. `fix` is the steps to open it.
    Blocked { found: String, fix: Vec<String> },
    /// A firewall is on and Core cannot tell whether it allows the ports.
    Unsure { found: String, fix: Vec<String> },
}

/// Make sure the firewall lets `ports` in, changing it when Core is allowed to.
/// `data_dir` remembers that the Windows prompt was already shown.
pub async fn ensure(ports: &[u16], data_dir: &Path) -> Checked {
    if ports.is_empty() {
        return Checked::default();
    }
    #[cfg(windows)]
    let (ports, blocked_programs) = platform::ensure(ports, data_dir).await;
    #[cfg(not(windows))]
    let (ports, blocked_programs) =
        (platform::ensure(ports, data_dir).await, Vec::new());
    Checked {
        ports,
        blocked_programs,
    }
}

/// The firewall's answer for the ports, and the programs it blocks whatever the port.
#[derive(Default)]
pub struct Checked {
    pub ports: Firewall,
    /// Programs with a rule that blocks incoming connections. Windows only. Such a rule wins
    /// over Core's allow rule, so a server run by one of these programs cannot be joined.
    pub blocked_programs: Vec<String>,
}

impl Checked {
    /// The firewall's answer for a port served by `program`.
    pub fn for_program(&self, program: Option<&str>) -> Firewall {
        let blocked = program.filter(|program| {
            self.blocked_programs
                .iter()
                .any(|blocked| blocked.eq_ignore_ascii_case(program))
        });
        match blocked {
            Some(program) => Firewall::Blocked {
                found: format!(
                    "Windows Firewall has a rule that blocks {program}, the Java this server runs on. A block rule wins over Core's allow rule. Windows adds it when \"Cancel\" is pressed on its \"allow access\" window"
                ),
                fix: vec![
                    "Open the Start menu, type PowerShell, choose \"Run as administrator\", and run:".into(),
                    format!(
                        "Get-NetFirewallRule -Direction Inbound -Action Block | Where-Object {{ ($_ | Get-NetFirewallApplicationFilter).Program -eq '{program}' }} | Remove-NetFirewallRule"
                    ),
                ],
            },
            None => self.ports.clone(),
        }
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
/// Ports as a firewall rule lists them: Core's port, the server range, and any server port
/// outside that range.
fn rule_ports(ports: &[u16]) -> String {
    let mut extra: Vec<u16> = ports
        .iter()
        .copied()
        .filter(|port| !SERVER_PORTS.contains(port))
        .collect();
    extra.sort_unstable();
    extra.dedup();
    extra
        .iter()
        .map(u16::to_string)
        .chain([format!("{}-{}", SERVER_PORTS.start(), SERVER_PORTS.end())])
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg_attr(not(windows), allow(dead_code))]
/// Whether a rule's port list (`16662,25565-25664`) contains `port`.
fn rule_covers(rule: &str, port: u16) -> bool {
    rule.split(',').any(|part| {
        let part = part.trim();
        if part.eq_ignore_ascii_case("any") {
            return true;
        }
        match part.split_once('-') {
            Some((low, high)) => {
                matches!((low.parse::<u16>(), high.parse::<u16>()), (Ok(low), Ok(high)) if (low..=high).contains(&port))
            }
            None => part.parse() == Ok(port),
        }
    })
}

#[cfg(windows)]
mod platform {
    use std::{path::Path, time::Duration};

    use serde::Deserialize;
    use tokio::process::Command;

    use super::{rule_covers, rule_ports, Firewall};

    const RULE: &str = "Amberite Core";
    /// Windows closes an unanswered administrator prompt by itself after about two minutes.
    const PROMPT_TIMEOUT: Duration = Duration::from_secs(150);

    #[derive(Deserialize)]
    struct State {
        /// Whether Windows Firewall is on for any network profile.
        enabled: bool,
        /// Whether Core already runs with administrator rights.
        admin: bool,
        /// Local ports of the "Amberite Core" rule, when it exists.
        ports: Option<String>,
        /// Programs with a rule that blocks incoming connections.
        blocked: Vec<String>,
    }

    const QUERY: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$rule = Get-NetFirewallRule -DisplayName 'Amberite Core' | Where-Object { $_.Enabled -eq 'True' -and $_.Direction -eq 'Inbound' -and $_.Action -eq 'Allow' } | Select-Object -First 1
$ports = if ($rule) { (($rule | Get-NetFirewallPortFilter).LocalPort) -join ',' } else { $null }
$blocked = @(Get-NetFirewallRule -Direction Inbound -Action Block -Enabled True | Get-NetFirewallApplicationFilter | ForEach-Object { $_.Program } | Sort-Object -Unique)
[pscustomobject]@{
  enabled = [bool](Get-NetFirewallProfile | Where-Object { $_.Enabled -eq 'True' })
  admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
  ports = $ports
  blocked = $blocked
} | ConvertTo-Json -Compress
"#;

    async fn powershell(script: &str, timeout: Duration) -> Option<String> {
        let output = tokio::time::timeout(
            timeout,
            Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", script])
                .kill_on_drop(true)
                .output(),
        )
        .await
        .ok()?
        .ok()?;
        Some(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    async fn query() -> Option<State> {
        let text = powershell(QUERY, Duration::from_secs(60)).await?;
        serde_json::from_str(text.trim()).ok()
    }

    /// The answer for the ports, and every program a block rule names.
    pub async fn ensure(
        ports: &[u16],
        data_dir: &Path,
    ) -> (Firewall, Vec<String>) {
        let Some(state) = query().await else {
            return (
                Firewall::Unsure {
                    found: "Windows Firewall could not be read".into(),
                    fix: vec![],
                },
                Vec::new(),
            );
        };
        if !state.enabled {
            return (Firewall::Open, Vec::new());
        }
        let (firewall, state) = rule(ports, data_dir, state).await;
        (firewall, state.blocked)
    }

    /// Make sure the "Amberite Core" rule allows `ports`, asking for permission at most once.
    async fn rule(
        ports: &[u16],
        data_dir: &Path,
        mut state: State,
    ) -> (Firewall, State) {
        let wanted = rule_ports(ports);
        let covered = |state: &State| {
            state.ports.as_deref().is_some_and(|rule| {
                ports.iter().all(|port| rule_covers(rule, *port))
            })
        };
        let add = format!(
            "advfirewall firewall add rule name=\"{RULE}\" dir=in action=allow protocol=TCP localport={wanted} profile=any"
        );
        let delete =
            format!("advfirewall firewall delete rule name=\"{RULE}\"");

        let asked = data_dir.join("firewall-asked");
        if !covered(&state) && (state.admin || !asked.exists()) {
            // The marker is written first: the prompt is shown once, whatever happens next.
            let _ = std::fs::write(&asked, "");
            if state.admin {
                let _ = powershell(&format!("netsh {delete}"), PROMPT_TIMEOUT)
                    .await;
                let _ =
                    powershell(&format!("netsh {add}"), PROMPT_TIMEOUT).await;
            } else {
                println!(
                    "\nWindows will ask for permission to let Amberite Core through the firewall. This is asked once.\n"
                );
                let _ = powershell(
                    &format!(
                        "Start-Process netsh -Verb RunAs -WindowStyle Hidden -Wait -ArgumentList '{add}'"
                    ),
                    PROMPT_TIMEOUT,
                )
                .await;
            }
            if let Some(after) = query().await {
                state = after;
            }
        }

        let as_admin = "Open the Start menu, type cmd, choose \"Run as administrator\", and run:";
        if !covered(&state) {
            let found = match &state.ports {
                Some(rule) => format!(
                    "Windows Firewall is on and the \"{RULE}\" rule only allows ports {rule}"
                ),
                None => format!(
                    "Windows Firewall is on and has no \"{RULE}\" rule (the permission prompt was not accepted)"
                ),
            };
            let mut fix = vec![as_admin.to_string()];
            if state.ports.is_some() {
                fix.push(format!("netsh {delete}"));
            }
            fix.push(format!("netsh {add}"));
            return (Firewall::Blocked { found, fix }, state);
        }
        (Firewall::Open, state)
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::path::Path;

    use tokio::process::Command;

    use super::Firewall;

    async fn run(program: &str, args: &[&str]) -> Option<bool> {
        Command::new(program)
            .args(args)
            .output()
            .await
            .ok()
            .map(|output| output.status.success())
    }

    pub async fn ensure(ports: &[u16], _data_dir: &Path) -> Firewall {
        let root = whoami::username() == "root";
        let ufw_on =
            std::fs::read_to_string("/etc/ufw/ufw.conf").is_ok_and(|conf| {
                conf.lines()
                    .any(|line| line.trim().eq_ignore_ascii_case("ENABLED=yes"))
            });
        if ufw_on {
            let fix: Vec<String> = ports
                .iter()
                .map(|port| format!("sudo ufw allow {port}/tcp"))
                .collect();
            if !root {
                // Only root can read ufw's rules.
                return Firewall::Unsure {
                    found: "ufw is on and Core does not run as root, so it cannot see or change the rules".into(),
                    fix: [vec!["Make sure the firewall on this computer allows the port:".to_string()], fix].concat(),
                };
            }
            for port in ports {
                if run("ufw", &["allow", &format!("{port}/tcp")]).await
                    != Some(true)
                {
                    return Firewall::Blocked {
                        found: format!(
                            "ufw is on and refused to allow port {port}"
                        ),
                        fix,
                    };
                }
            }
            return Firewall::Open;
        }

        if run("firewall-cmd", &["--state"]).await == Some(true) {
            for port in ports {
                let spec = format!("{port}/tcp");
                let query = format!("--query-port={spec}");
                if run("firewall-cmd", &[&query]).await == Some(true) {
                    continue;
                }
                let add = format!("--add-port={spec}");
                let opened = root
                    && run("firewall-cmd", &[&add]).await == Some(true)
                    && run("firewall-cmd", &["--permanent", &add]).await
                        == Some(true);
                if !opened {
                    return Firewall::Blocked {
                        found: format!("firewalld is on and does not allow port {port}"),
                        fix: vec![
                            format!("sudo firewall-cmd --add-port={spec}"),
                            format!("sudo firewall-cmd --permanent --add-port={spec}"),
                        ],
                    };
                }
            }
        }
        Firewall::Open
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::path::Path;

    use tokio::process::Command;

    use super::Firewall;

    pub async fn ensure(_ports: &[u16], _data_dir: &Path) -> Firewall {
        let on =
            Command::new("/usr/libexec/ApplicationFirewall/socketfilterfw")
                .arg("--getglobalstate")
                .output()
                .await
                .is_ok_and(|output| {
                    String::from_utf8_lossy(&output.stdout).contains("enabled")
                });
        if !on {
            return Firewall::Open;
        }
        Firewall::Unsure {
            found: "the macOS firewall is on, and it decides per program".into(),
            fix: vec![
                "In System Settings, Network, Firewall, Options, set Java and Amberite Core to \"Allow incoming connections\".".into(),
            ],
        }
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod platform {
    use std::path::Path;

    use super::Firewall;

    pub async fn ensure(_ports: &[u16], _data_dir: &Path) -> Firewall {
        Firewall::Open
    }
}

#[cfg(test)]
mod tests {
    use super::{rule_covers, rule_ports};

    #[test]
    fn the_rule_lists_ports_outside_the_server_range_once() {
        assert_eq!(
            rule_ports(&[25565, 16662, 30000, 16662]),
            "16662,30000,25565-25664"
        );
    }

    #[test]
    fn a_rule_covers_single_ports_ranges_and_any() {
        assert!(rule_covers("16662,25565-25664", 16662));
        assert!(rule_covers("16662,25565-25664", 25600));
        assert!(!rule_covers("16662,25565-25664", 25665));
        assert!(rule_covers("Any", 1));
    }
}
