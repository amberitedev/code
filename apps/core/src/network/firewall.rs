//! This computer's own firewall. `copal install` opens it once, as admin, for Core's port and the
//! server port range, and `copal uninstall` closes it again. Core never runs a firewall tool
//! after that, so it needs no permission to.
//!
//! Windows: one inbound rule named "Amberite Core". Linux: ufw and firewalld rules. macOS: the
//! application firewall decides per program and is off by default; install leaves it alone.

use std::{ops::RangeInclusive, process::Command};

use color_eyre::eyre::{Result, bail};

/// Ports Core hands to servers, lowest first. The firewall is opened for all of them at install.
pub const SERVER_PORTS: RangeInclusive<u16> = 25565..=25664;

/// What the firewall does with a port. Filled by the reachability diagnosis (step 8).
#[allow(dead_code)] // Wired in step 8: reachability reporting.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Firewall {
    /// No firewall is on, or it allows the port.
    #[default]
    Open,
    /// A firewall is on and does not allow the port. `fix` is the steps to open it.
    Blocked { found: String, fix: Vec<String> },
    /// A firewall is on and Core cannot tell whether it allows the port.
    Unsure { found: String, fix: Vec<String> },
}

/// Allow Core's port and the server port range in. Returns what was changed, to print.
pub fn open(core_port: u16) -> Result<Vec<String>> {
    platform::open(core_port)
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

    use super::{SERVER_PORTS, run};

    const RULE: &str = "Amberite Core";

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
                "profile=any",
            ],
        )?;
        Ok(vec![format!(
            "Windows Firewall: rule \"{RULE}\" allows TCP {ports}"
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

    use super::{SERVER_PORTS, run};

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
        }
        if firewalld() {
            let range = range.replace(':', "-");
            run(
                "firewall-cmd",
                &[
                    "--permanent",
                    &format!("--add-port={core_port}/tcp"),
                    &format!("--add-port={range}/tcp"),
                ],
            )?;
            run("firewall-cmd", &["--reload"])?;
            done.push(format!(
                "firewalld: allowed TCP {core_port} and {range}"
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
        }
        if firewalld() {
            let range = range.replace(':', "-");
            let removed = run(
                "firewall-cmd",
                &[
                    "--permanent",
                    &format!("--remove-port={core_port}/tcp"),
                    &format!("--remove-port={range}/tcp"),
                ],
            );
            run("firewall-cmd", &["--reload"])?;
            if removed.is_ok() {
                done.push(format!(
                    "firewalld: removed TCP {core_port} and {range}"
                ));
            }
        }
        Ok(done)
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod platform {
    use color_eyre::eyre::Result;

    pub fn open(_core_port: u16) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    pub fn close(_core_port: u16) -> Result<Vec<String>> {
        Ok(Vec::new())
    }
}
