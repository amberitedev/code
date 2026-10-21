//! `copal install` and `copal uninstall`, each run once as admin. Install creates the two
//! accounts, the data folder with its permissions, the rights to start Java as `copal-servers`,
//! the firewall rules, and the service. Nothing runs as admin after that.

#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;

use color_eyre::eyre::{Result, WrapErr, bail};
use serde::{Deserialize, Serialize};

use super::{
    accounts::{self, Accounts, CORE, SERVERS},
    config::Config,
    folders,
    paths::Paths,
    service,
};
use crate::network::firewall;

/// The helper `copal` runs through `sudo` to become `copal-servers`, and the only program the
/// sudoers file allows. It prints its pid, which the program it runs keeps, so the shim knows
/// which process is Java.
#[cfg(unix)]
pub const HELPER: &str = "/usr/local/lib/copal/run-as-servers";

#[cfg(unix)]
const HELPER_SCRIPT: &str = "#!/bin/sh\n\
# Installed by `copal install`. sudoers lets the copal account run this, only as copal-servers.\n\
umask 007\n\
echo \"copal-pid $$\" >&2\n\
exec \"$@\"\n";

#[cfg(unix)]
const SUDOERS: &str = "/etc/sudoers.d/copal";

/// What install recorded, in `db/install.toml`. Only `copal` and admins can read it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallRecord {
    /// The OS id of the account that ran install: a uid on Linux and macOS, a SID on Windows.
    /// Recorded explicitly, so `sudo` or SSH cannot confuse who it is. Only this account and
    /// `copal` may open the local socket.
    pub owner: String,
    /// Core's port when install opened the firewall, so uninstall closes the same rule.
    pub port: u16,
    /// Windows: the password of `copal-servers`, which the shim needs to log on as it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub servers_password: Option<String>,
}

impl InstallRecord {
    /// The record, or `None` when Copal was never installed on this data folder. An account
    /// that cannot read it is told so: only `copal` and admins may.
    pub fn load(paths: &Paths) -> Result<Option<Self>> {
        let path = paths.install_record();
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                Ok(Some(toml::from_str(&text).wrap_err_with(|| {
                    format!("{} is not valid", path.display())
                })?))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(None)
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::PermissionDenied =>
            {
                bail!(
                    "Copal is installed on this machine and its data folder belongs to the {CORE} account. \
                     Use the service, or the copal CLI commands, instead of running Core yourself"
                )
            }
            Err(error) => Err(error)
                .wrap_err_with(|| format!("could not read {}", path.display())),
        }
    }

    fn save(&self, paths: &Paths, accounts: &Accounts) -> Result<()> {
        let path = paths.install_record();
        std::fs::write(&path, toml::to_string(self)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::os::unix::fs::chown(
                &path,
                Some(accounts.core.uid),
                Some(accounts.core.gid),
            )?;
            std::fs::set_permissions(
                &path,
                std::fs::Permissions::from_mode(0o600),
            )?;
        }
        // Windows: `db/` lets only `copal` and admins in, and the file inherits that.
        #[cfg(windows)]
        let _ = accounts;
        Ok(())
    }
}

/// `copal install`. `owner` names the account that may use the CLI; by default the one that
/// ran install (through `sudo`, the one that ran `sudo`). Refuses while a server runs, like
/// uninstall: it replaces the binary shims run from and resets the accounts' passwords.
pub fn install(owner: Option<&str>, allow_downgrade: bool) -> Result<()> {
    require_admin()?;
    refuse_data_dir_override("install")?;
    let config = Config::load()?;
    let paths = Paths::new(&config.data_dir);
    if !allow_downgrade {
        refuse_older_than_active(&paths)?;
    }

    // Core must not run while its binary is replaced and its folders change hands. The binary
    // goes first, so a replacement that fails has not touched any account or password.
    stop_core_with_no_servers(&paths, "install")?;
    let exe = match install_binary() {
        Ok(exe) => exe,
        Err(error) => {
            let _ = service::start();
            return Err(error);
        }
    };
    println!("Program: {}", exe.display());
    // The program just installed runs, not a version an earlier install updated to. It is at
    // least as new as that one (see `refuse_older_than_active`), unless the owner allowed the
    // downgrade.
    crate::update::versions::save(&paths, None)?;
    let created = accounts::create()?;
    println!("Accounts: {CORE} and {SERVERS}, without interactive sign-in");
    let owner = match owner {
        Some(name) => accounts::id_of(name)?,
        None => accounts::current_id()?,
    };
    println!("Owner: {owner} may use the copal CLI");
    #[cfg(unix)]
    {
        install_sudoers()?;
        println!("sudoers: {CORE} may run {HELPER} as {SERVERS}, nothing else");
    }

    folders::create(&paths, Some(&created.accounts))?;
    println!("Data folder: {}", paths.root().display());
    InstallRecord {
        owner,
        port: config.port,
        #[cfg(windows)]
        servers_password: Some(created.servers_password.clone()),
        #[cfg(not(windows))]
        servers_password: None,
    }
    .save(&paths, &created.accounts)?;

    for line in firewall::open(config.port)? {
        println!("Firewall: {line}");
    }

    #[cfg(windows)]
    let core_password = Some(created.core_password.as_str());
    #[cfg(not(windows))]
    let core_password = None;
    match service::register(&exe, core_password) {
        Ok(line) => println!("Service: {line}"),
        Err(error) => bail!("could not register the service: {error:#}"),
    }
    println!("\nCopal is installed.");
    Ok(())
}

/// `copal uninstall`. Refuses while a server runs, because its shim runs as an account this
/// removes. `remove_data` deletes the data folder too: worlds, backups, everything.
pub fn uninstall(remove_data: bool) -> Result<()> {
    require_admin()?;
    refuse_data_dir_override("uninstall")?;
    let config = Config::load()?;
    let paths = Paths::new(&config.data_dir);
    let record = InstallRecord::load(&paths)?;

    stop_core_with_no_servers(&paths, "uninstall")?;
    service::unregister()?;
    println!("Service: removed");

    let port = record.as_ref().map_or(config.port, |record| record.port);
    for line in firewall::close(port)? {
        println!("Firewall: {line}");
    }

    #[cfg(unix)]
    for path in [SUDOERS, HELPER] {
        remove_file(Path::new(path))?;
    }
    let program = program_path();
    if let Some(dir) = program.parent()
        && dir.exists()
        && let Err(error) = std::fs::remove_dir_all(dir)
    {
        println!(
            "Program: could not remove {} ({error}); delete it by hand",
            dir.display()
        );
    }

    accounts::remove()?;
    println!("Accounts: removed {CORE} and {SERVERS}");

    if remove_data {
        if paths.root().exists() {
            std::fs::remove_dir_all(paths.root()).wrap_err_with(|| {
                format!("could not remove {}", paths.root().display())
            })?;
        }
        println!("Data folder: removed {}", paths.root().display());
    } else {
        println!(
            "Data folder: kept {}. Run copal uninstall --remove-data to delete it",
            paths.root().display()
        );
    }
    println!("\nCopal is uninstalled.");
    Ok(())
}

/// Refuse to install a binary older than the version Core updated itself to: that version may
/// have migrated the database past what this binary can open, and it would be a downgrade.
fn refuse_older_than_active(paths: &Paths) -> Result<()> {
    let current = crate::update::current();
    if let Some(state) = crate::update::versions::load(paths)?
        && semver::Version::parse(&state.active)
            .is_ok_and(|active| active > current)
    {
        bail!(
            "Core has updated itself to {}, newer than this copal ({current}), and may have changed its database past what {current} can open. Run install with copal {} or newer, or pass --allow-downgrade to install {current} anyway",
            state.active,
            state.active
        );
    }
    Ok(())
}

/// Install and uninstall work on this machine's data folder only, never a development one.
fn refuse_data_dir_override(command: &str) -> Result<()> {
    if std::env::var_os("CORE_DATA_DIR").is_some() {
        bail!(
            "copal {command} always uses this machine's data folder. Unset CORE_DATA_DIR and run it again"
        );
    }
    Ok(())
}

/// Stop Core, then refuse while any server runs: its shim runs as an account `command` changes.
/// Shims outlive Core, so checking after the stop is exact, and no start can slip in between.
/// When servers run, Core is started again so they can be stopped through it.
fn stop_core_with_no_servers(paths: &Paths, command: &str) -> Result<()> {
    service::stop()?;
    let running = crate::servers::shim::running(paths);
    if !running.is_empty() {
        let _ = service::start();
        bail!(
            "these servers are still running: {}. Stop them first (copal servers stop <id>), then run {command} again",
            running.join(", ")
        );
    }
    Ok(())
}

fn require_admin() -> Result<()> {
    #[cfg(unix)]
    let admin = nix::unistd::geteuid().is_root();
    #[cfg(windows)]
    let admin = super::win::is_admin();
    if !admin {
        bail!(if cfg!(windows) {
            "run this from a terminal opened with \"Run as administrator\""
        } else {
            "run this with sudo"
        });
    }
    Ok(())
}

/// Where install puts the binary the service runs. Only admins can change it.
fn program_path() -> PathBuf {
    if cfg!(windows) {
        std::env::var_os("ProgramFiles")
            .map_or_else(|| PathBuf::from(r"C:\Program Files"), PathBuf::from)
            .join("Copal")
            .join("copal.exe")
    } else {
        PathBuf::from("/usr/local/lib/copal/copal")
    }
}

/// Copy this binary to `program_path`, unless it already runs from there. On Linux and macOS it
/// is 0755 whatever mode the build gave it, so only root can change it.
fn install_binary() -> Result<PathBuf> {
    let target = program_path();
    let current = std::env::current_exe()?;
    if dunce::canonicalize(&current).ok() != dunce::canonicalize(&target).ok() {
        let dir = target.parent().expect("the program has a folder");
        std::fs::create_dir_all(dir)?;
        let staged = dir.join("copal.new");
        std::fs::copy(&current, &staged)?;
        std::fs::rename(&staged, &target).wrap_err_with(|| {
            format!(
                "could not replace {}. A running server still uses it; stop every server and run install again",
                target.display()
            )
        })?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            &target,
            std::fs::Permissions::from_mode(0o755),
        )?;
    }
    Ok(target)
}

#[cfg(unix)]
fn install_sudoers() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let helper = Path::new(HELPER);
    std::fs::create_dir_all(helper.parent().expect("the helper has a folder"))?;
    std::fs::write(helper, HELPER_SCRIPT)?;
    std::fs::set_permissions(helper, std::fs::Permissions::from_mode(0o755))?;

    // sudo skips files in sudoers.d with a dot in their name, so a half-written file is never
    // read. A file visudo rejects would break sudo for everyone, so it is checked first.
    let staged = format!("{SUDOERS}.new");
    std::fs::write(
        &staged,
        format!("{CORE} ALL=({SERVERS}) NOPASSWD: {HELPER}\n"),
    )?;
    std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o440))?;
    let checked = std::process::Command::new("visudo")
        .args(["-cf", &staged])
        .output()?;
    if !checked.status.success() {
        let _ = std::fs::remove_file(&staged);
        bail!(
            "visudo rejected the sudoers file: {}",
            String::from_utf8_lossy(&checked.stderr).trim()
        );
    }
    std::fs::rename(&staged, SUDOERS)?;
    Ok(())
}

#[cfg(unix)]
fn remove_file(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(error).wrap_err_with(|| {
                format!("could not remove {}", path.display())
            })
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::versions::{self, State};

    #[test]
    fn an_install_older_than_the_active_version_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        assert!(refuse_older_than_active(&paths).is_ok());
        versions::save(&paths, Some(&State::update("0.0.1", "99.0.0", None)))
            .unwrap();
        assert!(refuse_older_than_active(&paths).is_err());
        versions::save(&paths, Some(&State::update("99.0.0", "0.0.1", None)))
            .unwrap();
        assert!(refuse_older_than_active(&paths).is_ok());
    }
}
