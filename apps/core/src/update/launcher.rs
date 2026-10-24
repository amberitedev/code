//! `copal run`: the launcher. It runs the active version from `versions/` as a child process and
//! starts it again whenever it exits to update. A version that has not proven healthy is on
//! trial: if it does not write its health marker within [`HEALTH_TIMEOUT`], the launcher stops
//! it, puts the database back, and runs the previous version.
//!
//! The launcher is the binary `copal install` put in place, or the one run by hand in
//! development, and the updater never replaces it. So an old launcher starts every newer Core,
//! and what it relies on never changes:
//! - `versions/state.json`, as `versions::State` reads it;
//! - a version runs as `<binary> core`, stops when its stdin closes, and exits with
//!   [`RESTART`] to be started again;
//! - a healthy version writes `versions/<version>/healthy`.

use std::{
    future::Future,
    path::PathBuf,
    process::Stdio,
    time::{Duration, Instant},
};

use color_eyre::eyre::{Result, WrapErr};
use tokio::process::{Child, Command};
use tracing::{error, info, warn};

use super::versions::{self, FailedTrial};
use crate::daemon::paths::Paths;

/// The exit code with which Core asks to be started again, now as the active version.
pub const RESTART: i32 = 75;
/// How long a new version has to answer its first request.
pub const HEALTH_TIMEOUT: Duration = Duration::from_secs(60);
/// How long Core may take to stop once asked.
const STOP_TIMEOUT: Duration = Duration::from_secs(30);

/// Where a version on trial is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Healthy,
    Waiting,
    Failed,
}

/// A version on trial is healthy once its marker is there. It failed when it exited first or
/// [`HEALTH_TIMEOUT`] passed without it.
pub fn health(marker: bool, exited: bool, elapsed: Duration) -> Health {
    if marker {
        Health::Healthy
    } else if exited || elapsed >= HEALTH_TIMEOUT {
        Health::Failed
    } else {
        Health::Waiting
    }
}

/// Run Core until `stop` resolves or Core exits for any reason but [`RESTART`], and return
/// the exit code. `service` hides Core's console under a service manager that has none.
pub async fn run(
    paths: &Paths,
    service: bool,
    stop: impl Future<Output = ()>,
) -> Result<i32> {
    tokio::pin!(stop);
    let _lock = lock(paths)?;
    loop {
        let (mut child, trial) = start(paths, service)?;
        let stdin = child.stdin.take();
        let started = Instant::now();
        let mut tick = tokio::time::interval(Duration::from_millis(250));
        let mut trial = trial;
        let exited = loop {
            tokio::select! {
                status = child.wait() => break Some(status?),
                () = &mut stop => {
                    drop(stdin);
                    stop_child(&mut child).await;
                    return Ok(0);
                }
                _ = tick.tick(), if trial.is_some() => {
                    let version = trial.as_deref().expect("on trial");
                    let marker = versions::marker(paths, version).exists();
                    match health(marker, false, started.elapsed()) {
                        Health::Healthy => {
                            passed(paths)?;
                            info!(version, "the new version is healthy");
                            trial = None;
                        }
                        Health::Failed => break None,
                        Health::Waiting => {}
                    }
                }
            }
        };
        if let Some(version) = &trial {
            let marker = versions::marker(paths, version).exists();
            if health(marker, exited.is_some(), started.elapsed())
                == Health::Healthy
            {
                passed(paths)?;
            } else {
                let reason = if exited.is_some() {
                    FailedTrial::Exited
                } else {
                    FailedTrial::NoAnswer
                };
                error!(
                    version,
                    "the new version {}; going back to the previous one",
                    reason.describe()
                );
                drop(stdin);
                let _ = child.kill().await;
                fail(paths, reason)?;
                continue;
            }
        }
        let status = exited.expect("Core exited");
        match status.code() {
            Some(RESTART) => info!("starting Core again"),
            code => return Ok(code.unwrap_or(1)),
        }
    }
}

/// Start the active version, and its trial version when it has one. A trial binary the OS
/// refuses to run is a failed trial: Core goes back to the previous version and starts that.
fn start(paths: &Paths, service: bool) -> Result<(Child, Option<String>)> {
    loop {
        let (exe, trial) = prepare(paths)?;
        match spawn(&exe, service) {
            Ok(child) => return Ok((child, trial)),
            Err(error) if trial.is_some() => {
                error!(
                    %error,
                    "the new version would not start; going back to the previous one"
                );
                fail(paths, FailedTrial::NotStarted)?;
            }
            Err(error) => {
                return Err(error).wrap_err_with(|| {
                    format!("could not start Core from {}", exe.display())
                });
            }
        }
    }
}

/// Hold the lock that makes this the only launcher on the data folder, for as long as it
/// supervises. A second `copal run` would otherwise change the state and the database under a
/// running Core. An account that may not open the data folder can change nothing there, so it
/// goes on without the lock, and the Core it starts says what is wrong.
fn lock(paths: &Paths) -> Result<Option<std::fs::File>> {
    let path = paths.launcher_lock();
    let file = match std::fs::create_dir_all(paths.versions()).and_then(|()| {
        std::fs::File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
    }) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            return Ok(None);
        }
        Err(error) => {
            return Err(error).wrap_err_with(|| {
                format!("could not open {}", path.display())
            });
        }
    };
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => color_eyre::eyre::bail!(
            "copal run is already running on {}",
            paths.root().display()
        ),
        Err(std::fs::TryLockError::Error(error)) => Err(error.into()),
    }
}

/// The binary to run and, when it is on trial, its version. A trial's database is copied
/// aside here, before the new version runs, and a requested rollback's is put back.
fn prepare(paths: &Paths) -> Result<(PathBuf, Option<String>)> {
    let own = std::env::current_exe;
    let Some(mut state) = versions::load(paths)? else {
        return Ok((own()?, None));
    };
    if state.restore_database {
        // The flag stays until the restore is done, so one cut short is done again, and Core does
        // not start on a database the rolled-back version cannot open.
        versions::restore_database(paths).wrap_err(
            "could not put the database from before the update back; Core starts once it is",
        )?;
        state.restore_database = false;
        versions::save(paths, Some(&state))?;
        info!(
            version = %state.active,
            "rolled back with the database from before the update"
        );
    }
    if let Some(trial) = &mut state.trial
        && !trial.database_saved
    {
        let marker = versions::marker(paths, &state.active);
        match std::fs::remove_file(marker) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                return Err(error.into());
            }
            _ => {}
        }
        versions::save_database(paths)
            .wrap_err("could not copy the database before the update")?;
        trial.database_saved = true;
        versions::save(paths, Some(&state))?;
        info!(version = %state.active, "trying the new version");
    }
    let exe = versions::binary(paths, &state.active);
    if !exe.exists() {
        warn!(
            version = %state.active,
            "the active version is missing; running the launcher's own"
        );
        return Ok((own()?, None));
    }
    Ok((exe, state.trial.map(|_| state.active)))
}

/// The version on trial proved healthy.
fn passed(paths: &Paths) -> Result<()> {
    if let Some(mut state) = versions::load(paths)? {
        state.trial = None;
        versions::save(paths, Some(&state))?;
    }
    Ok(())
}

/// The version on trial failed: put the database back and return to the previous version.
fn fail(paths: &Paths, reason: FailedTrial) -> Result<()> {
    let Some(state) = versions::load(paths)? else {
        return Ok(());
    };
    if state
        .trial
        .as_ref()
        .is_some_and(|trial| trial.database_saved)
    {
        versions::restore_database(paths)
            .wrap_err("could not put the database back")?;
    }
    let back = state
        .failed_trial(reason)
        .filter(|back| versions::binary(paths, &back.active).exists());
    versions::save(paths, back.as_ref())?;
    Ok(())
}

fn spawn(exe: &std::path::Path, service: bool) -> std::io::Result<Child> {
    let mut command = Command::new(exe);
    command.arg("core").stdin(Stdio::piped()).kill_on_drop(true);
    if service {
        command.stdout(Stdio::null()).stderr(Stdio::null());
        #[cfg(windows)]
        command.creation_flags(
            windows_sys::Win32::System::Threading::CREATE_NO_WINDOW,
        );
    }
    command.spawn()
}

/// Wait for Core to stop after its stdin closed, and kill it if it does not.
async fn stop_child(child: &mut Child) {
    if tokio::time::timeout(STOP_TIMEOUT, child.wait())
        .await
        .is_err()
    {
        warn!("Core did not stop in time; killing it");
        let _ = child.kill().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::versions::State;

    #[test]
    fn a_version_is_healthy_once_its_marker_lands() {
        let second = Duration::from_secs(1);
        assert_eq!(health(false, false, second), Health::Waiting);
        assert_eq!(health(true, false, second), Health::Healthy);
        // The marker counts even when it lands just before Core exits.
        assert_eq!(health(true, true, second), Health::Healthy);
    }

    #[test]
    fn no_marker_within_a_minute_or_an_early_exit_fails() {
        assert_eq!(health(false, false, HEALTH_TIMEOUT), Health::Failed);
        assert_eq!(health(false, true, Duration::from_secs(1)), Health::Failed);
        assert_eq!(
            health(false, false, HEALTH_TIMEOUT - Duration::from_millis(1)),
            Health::Waiting
        );
    }

    #[test]
    fn a_second_launcher_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let first = lock(&paths).unwrap();
        assert!(first.is_some());
        assert!(lock(&paths).is_err());
        drop(first);
        assert!(lock(&paths).unwrap().is_some());
    }

    #[tokio::test]
    async fn a_trial_binary_the_os_refuses_falls_back_to_the_previous_version()
    {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        crate::daemon::folders::create(&paths, None).unwrap();
        std::fs::write(paths.database(), "0.1.0 schema").unwrap();
        for version in ["0.1.0", "0.2.0"] {
            std::fs::create_dir_all(paths.version(version)).unwrap();
        }
        // Something that starts, as the previous version, and an empty file the OS will not run.
        let shell = if cfg!(windows) {
            std::path::PathBuf::from(std::env::var("ComSpec").unwrap())
        } else {
            "/bin/sh".into()
        };
        std::fs::copy(&shell, versions::binary(&paths, "0.1.0")).unwrap();
        std::fs::write(versions::binary(&paths, "0.2.0"), "").unwrap();
        versions::save(&paths, Some(&State::update("0.1.0", "0.2.0", None)))
            .unwrap();

        let (mut child, trial) = start(&paths, true).unwrap();
        let _ = child.kill().await;
        assert_eq!(trial, None);
        let state = versions::load(&paths).unwrap().unwrap();
        assert_eq!(state.active, "0.1.0");
        assert_eq!(state.failed.as_deref(), Some("0.2.0"));
        assert_eq!(
            std::fs::read_to_string(paths.database()).unwrap(),
            "0.1.0 schema"
        );
    }

    #[test]
    fn a_rollback_whose_restore_fails_is_retried_and_holds_core_back() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        crate::daemon::folders::create(&paths, None).unwrap();
        std::fs::create_dir_all(paths.version("0.1.0")).unwrap();
        std::fs::write(versions::binary(&paths, "0.1.0"), "").unwrap();
        let back = State::update("0.1.0", "0.2.0", None).roll_back().unwrap();
        versions::save(&paths, Some(&back)).unwrap();
        // No copy from before the update: the restore cannot happen.
        assert!(prepare(&paths).is_err());
        assert!(versions::load(&paths).unwrap().unwrap().restore_database);

        std::fs::write(paths.database(), "before").unwrap();
        versions::save_database(&paths).unwrap();
        std::fs::write(paths.database(), "after").unwrap();
        prepare(&paths).unwrap();
        assert_eq!(
            std::fs::read_to_string(paths.database()).unwrap(),
            "before"
        );
        assert!(!versions::load(&paths).unwrap().unwrap().restore_database);
    }

    #[test]
    fn a_failed_trial_puts_the_database_back_and_the_previous_version_on() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        crate::daemon::folders::create(&paths, None).unwrap();
        for version in ["0.1.0", "0.2.0"] {
            std::fs::create_dir_all(paths.version(version)).unwrap();
            std::fs::write(versions::binary(&paths, version), "").unwrap();
        }
        std::fs::write(paths.database(), "0.1.0 schema").unwrap();
        versions::save(&paths, Some(&State::update("0.1.0", "0.2.0", None)))
            .unwrap();

        let (exe, trial) = prepare(&paths).unwrap();
        assert_eq!(exe, versions::binary(&paths, "0.2.0"));
        assert_eq!(trial.as_deref(), Some("0.2.0"));
        std::fs::write(paths.database(), "0.2.0 schema").unwrap();
        // A relaunch mid-trial must not copy the migrated database over the saved one.
        prepare(&paths).unwrap();

        fail(&paths, FailedTrial::Exited).unwrap();
        assert_eq!(
            std::fs::read_to_string(paths.database()).unwrap(),
            "0.1.0 schema"
        );
        let state = versions::load(&paths).unwrap().unwrap();
        assert_eq!(state.active, "0.1.0");
        assert_eq!(state.failed.as_deref(), Some("0.2.0"));
        assert_eq!(state.failed_reason, Some(FailedTrial::Exited));
        assert_eq!(prepare(&paths).unwrap().1, None);
    }
}
