//! `copal setup` and `copal uninstall`, each run once as admin. Setup creates the two
//! accounts, the data folder with its permissions, the rights to start Java as `copal-servers`,
//! the firewall rules, and the service. Nothing runs as admin after that but
//! `copal doctor --fix`, which repairs the firewall rules and starts the service.

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
use crate::{
    cli::{
        Done,
        ui::{Mark, Ui},
    },
    network::firewall,
};

/// The helper `copal` runs through `sudo` to become `copal-servers`, and the only program the
/// sudoers file allows. It prints its pid, which the program it runs keeps, so the shim knows
/// which process is Java.
#[cfg(unix)]
pub const HELPER: &str = "/usr/local/lib/copal/run-as-servers";

#[cfg(unix)]
const HELPER_SCRIPT: &str = "#!/bin/sh\n\
# Installed by `copal setup`. sudoers lets the copal account run this, only as copal-servers.\n\
umask 007\n\
echo \"copal-pid $$\" >&2\n\
exec \"$@\"\n";

#[cfg(unix)]
const SUDOERS: &str = "/etc/sudoers.d/copal";

/// What setup recorded, in `db/install.toml`. Only `copal` and admins can read it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallRecord {
    /// The OS id of the account that ran setup: a uid on Linux and macOS, a SID on Windows.
    /// Recorded explicitly, so `sudo` or SSH cannot confuse who it is. Only this account and
    /// `copal` may open the local socket.
    pub owner: String,
    /// Core's port when setup opened the firewall, so uninstall closes the same rule.
    pub port: u16,
    /// Windows: the password of `copal-servers`, which the shim needs to log on as it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub servers_password: Option<String>,
}

impl InstallRecord {
    /// The record, or `None` when Core was never set up on this data folder. An account
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
                    "Core is set up on this machine and its data folder belongs to the {CORE} account. \
                     Use the service, or the copal commands, instead of running Core yourself"
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

/// `copal setup`, which the installer runs once. `owner` names the account that may use the
/// CLI; by default the one that ran setup (through `sudo`, the one that ran `sudo`). Run again
/// on a Core that is set up, it changes nothing and points at `copal doctor`, which repairs.
pub fn setup(
    ui: &Ui,
    owner: Option<&str>,
    allow_downgrade: bool,
) -> Result<Done> {
    ui.intro("copal setup");
    require_admin("copal setup")?;
    refuse_data_dir_override("setup")?;
    let config = Config::load()?;
    let paths = Paths::new(&config.data_dir);
    if service::state() != service::State::NotRegistered
        && InstallRecord::load(&paths)?.is_some()
    {
        ui.say(Mark::Done, "Core is already set up on this computer.");
        if owner.is_some() {
            ui.say(
                Mark::Warning,
                "--owner was not applied: the owner is chosen when Core is first set up.",
            );
        }
        ui.outro("Check it, and repair what is wrong, with: copal doctor");
        return Ok(Done::ok(&serde_json::json!({ "set_up": false })));
    }
    if !allow_downgrade {
        refuse_older_than_active(&paths)?;
    }

    // Core must not run while its binary is replaced and its folders change hands. The binary
    // goes first, so a replacement that fails has not touched any account or password.
    stop_core_with_no_servers(&paths, "setup")?;
    let exe = match install_binary() {
        Ok(exe) => exe,
        Err(error) => {
            let _ = service::start();
            return Err(error);
        }
    };
    ui.say(Mark::Done, format!("Program: {}", exe.display()));
    // The program just set up runs, not a version an earlier Core updated to. It is at least
    // as new as that one (see `refuse_older_than_active`), unless the owner allowed the
    // downgrade.
    crate::update::versions::save(&paths, None)?;
    let created = accounts::create()?;
    ui.say(
        Mark::Done,
        format!("Accounts: {CORE} and {SERVERS}, which nobody can sign in to"),
    );
    let owner = match owner {
        Some(name) => accounts::id_of(name)?,
        None => accounts::current_id()?,
    };
    ui.say(
        Mark::Done,
        format!("Owner: {owner} may use the copal command"),
    );
    #[cfg(unix)]
    {
        install_sudoers()?;
        ui.say(
            Mark::Done,
            format!(
                "sudoers: {CORE} may run {HELPER} as {SERVERS}, nothing else"
            ),
        );
    }

    folders::create(&paths, Some(&created.accounts))?;
    ui.say(
        Mark::Done,
        format!("Data folder: {}", paths.root().display()),
    );
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
        ui.say(Mark::Done, format!("Firewall: {line}"));
    }

    #[cfg(windows)]
    let core_password = Some(created.core_password.as_str());
    #[cfg(not(windows))]
    let core_password = None;
    match service::register(&exe, core_password) {
        Ok(line) => ui.say(Mark::Done, format!("Service: {line}")),
        Err(error) => bail!("could not register the service: {error:#}"),
    }
    ui.outro(if cfg!(windows) {
        "Core is set up. Next: copal pair"
    } else {
        "Core is set up. Next, as yourself and without sudo: copal pair"
    });
    Ok(Done::ok(&serde_json::json!({ "set_up": true })))
}

/// `copal uninstall`. Refuses while a server runs, because its shim runs as an account this
/// removes. The data folder (worlds, backups, everything) stays unless `remove_data`; in a
/// terminal, with neither flag, the person is asked.
pub fn uninstall(
    ui: &Ui,
    remove_data: bool,
    keep_data: bool,
    yes: bool,
) -> Result<Done> {
    ui.intro("copal uninstall");
    require_admin("copal uninstall")?;
    refuse_data_dir_override("uninstall")?;
    let config = Config::load()?;
    let paths = Paths::new(&config.data_dir);
    let record = InstallRecord::load(&paths)?;
    let folder = paths.root().display().to_string();

    let remove_data = if remove_data || keep_data || yes || !ui.terminal() {
        remove_data
    } else {
        ui.select(
            &format!(
                "What should happen to the servers, worlds and backups in {folder}?"
            ),
            &[
                (
                    false,
                    "Keep them".to_owned(),
                    "setting Core up again finds them".to_owned(),
                ),
                (
                    true,
                    "Delete them".to_owned(),
                    "there is no undo".to_owned(),
                ),
            ],
            "Pass --keep-data or --remove-data",
        )?
    };
    let question = if remove_data {
        "Remove Core, and delete every server, world and backup?"
    } else {
        "Remove Core from this computer?"
    };
    if !yes && !ui.confirm(question, "--yes")? {
        ui.outro("Nothing was changed.");
        return Ok(Done::ok(&serde_json::json!({ "uninstalled": false })));
    }

    let spinner = ui.spinner("Stopping Core");
    if let Err(error) = stop_core_with_no_servers(&paths, "uninstall")
        .and_then(|()| service::unregister())
    {
        spinner.clear();
        return Err(error.wrap_err("Core is still set up"));
    }
    spinner.done("Service: removed");

    let port = record.as_ref().map_or(config.port, |record| record.port);
    for line in firewall::close(port)? {
        ui.say(Mark::Done, format!("Firewall: {line}"));
    }

    #[cfg(unix)]
    for path in [SUDOERS, HELPER] {
        crate::util::if_present(std::fs::remove_file(path))
            .wrap_err_with(|| format!("could not remove {path}"))?;
    }
    let program = program_path();
    if let Some(dir) = program.parent()
        && dir.exists()
        && let Err(error) = std::fs::remove_dir_all(dir)
    {
        ui.say(
            Mark::Warning,
            format!(
                "Program: could not remove {} ({error}). Delete it by hand.",
                dir.display()
            ),
        );
    }

    accounts::remove()?;
    ui.say(
        Mark::Done,
        format!("Accounts: removed {CORE} and {SERVERS}"),
    );

    if remove_data {
        // The key that takes this Core off its account goes with the folder: use it first.
        let left = left_account(&paths, config.backend_url.as_deref());
        if paths.root().exists() {
            std::fs::remove_dir_all(paths.root())
                .wrap_err_with(|| format!("could not remove {folder}"))?;
        }
        ui.say(Mark::Done, format!("Data folder: removed {folder}"));
        match left {
            Some(true) => ui.say(
                Mark::Done,
                "Amberite: this Core was removed from its account",
            ),
            Some(false) => ui.say(
                Mark::Warning,
                "Amberite could not be told. Remove this Core there too, under Settings, Cores.",
            ),
            None => {}
        }
    } else {
        ui.say(
            Mark::Info,
            format!(
                "Data folder: kept {folder}, with every server, world and backup. Delete it by hand when you no longer want them."
            ),
        );
    }
    ui.outro("Core is uninstalled.");
    Ok(Done::ok(&serde_json::json!({
        "uninstalled": true,
        "data_removed": remove_data,
    })))
}

/// Take the Core on this data folder off its account, before the folder goes. `None` when it
/// is not paired or has no database, `Some(false)` when the backend could not be told. The
/// database is only read, never migrated: this binary may be older than the Core that last
/// wrote it.
fn left_account(paths: &Paths, backend_url: Option<&str>) -> Option<bool> {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    let database = paths.database();
    if !database.exists() {
        return None;
    }
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return Some(false);
    };
    runtime.block_on(async {
        let options = SqliteConnectOptions::new()
            .filename(&database)
            .read_only(true);
        let Ok(db) = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
        else {
            return Some(false);
        };
        let left = crate::api::pairing::leave(&db, backend_url).await;
        db.close().await;
        left.unwrap_or(Some(false))
    })
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
            "Core has updated itself to {}, newer than this copal ({current}), and may have changed its database past what {current} can open. Run setup with copal {} or newer, or pass --allow-downgrade to set up {current} anyway",
            state.active,
            state.active
        );
    }
    Ok(())
}

/// Setup and uninstall work on this machine's data folder only, never a development one.
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
            "these servers are still running: {}. Stop them first with copal stop --all{}, then run {command} again",
            running.join(", "),
            if cfg!(unix) { " (without sudo)" } else { "" }
        );
    }
    Ok(())
}

/// Whether this process may change the machine: root, or an elevated admin on Windows.
pub fn is_admin() -> bool {
    #[cfg(unix)]
    let admin = nix::unistd::geteuid().is_root();
    #[cfg(windows)]
    let admin = super::win::is_admin();
    admin
}

/// Where an admin runs a command on Windows, to begin a sentence that tells someone to.
const ADMIN_TERMINAL: &str =
    "In a terminal opened with \"Run as administrator\"";

/// A step that takes an admin, the command last so it can be copied: `Set it up with: sudo
/// copal setup`, or on Windows the same step led by where to run it. `step` is the words
/// before the command, such as `Set it up with`.
pub fn as_admin(step: &str, command: &str) -> String {
    if cfg!(windows) {
        let mut step = step.chars();
        let step: String = step
            .next()
            .map(|first| first.to_lowercase().chain(step).collect())
            .unwrap_or_default();
        format!("{ADMIN_TERMINAL}, {step}: {command}")
    } else {
        format!("{step}: sudo {command}")
    }
}

/// Refuse without admin rights, saying how to run `command` with them.
fn require_admin(command: &str) -> Result<()> {
    if !is_admin() {
        return Err(crate::cli::client::Stuck {
            what: "This needs an admin.".into(),
            next: as_admin("Run it again with", command),
        }
        .into());
    }
    Ok(())
}

/// Where setup puts the binary the service runs. Only admins can change it.
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
                "could not replace {}. A running server still uses it; stop every server and run setup again",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::versions::{self, State};

    #[test]
    fn a_setup_older_than_the_active_version_is_refused() {
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
