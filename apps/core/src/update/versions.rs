//! The `versions/` folder: every installed Core version in a folder of its own, and the state
//! that tells the launcher (`update::launcher`) which one to run.
//!
//! - `versions/<version>/copal` (`copal.exe` on Windows): that version's binary, never
//!   overwritten.
//! - `versions/<version>/healthy`: that version answered its first request.
//! - `versions/state.json`: a [`State`]. Core writes it to update or roll back; the launcher
//!   writes it while it tries a new version.
//!
//! Without `state.json` the launcher runs its own binary, as in development and right after
//! `copal install`.

use std::{
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::daemon::paths::Paths;

/// Which version the launcher runs. Launchers are never updated, so a field never changes its
/// meaning and new fields must be optional.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub active: String,
    /// The version before `active`, kept for a rollback.
    pub previous: Option<String>,
    /// Set while `active` has not proven healthy.
    pub trial: Option<Trial>,
    /// Set by a manual rollback: put back the database copied before the update.
    #[serde(default)]
    pub restore_database: bool,
    /// The last version that failed its health check and was rolled back from. Neither
    /// auto-update nor `copal update apply` installs it again unless asked to retry.
    pub failed: Option<String>,
    /// How `failed` failed. Launchers from before this field leave it out.
    pub failed_reason: Option<FailedTrial>,
}

/// How a version on trial failed, as the launcher saw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FailedTrial {
    /// The OS would not run its binary.
    NotStarted,
    /// It ran, and exited before it answered a request.
    Exited,
    /// It ran, and did not answer a request within a minute.
    NoAnswer,
}

impl FailedTrial {
    /// What happened, for a person, after "Copal <version>".
    pub fn describe(self) -> &'static str {
        match self {
            Self::NotStarted => "never started: this machine would not run it",
            Self::Exited => "started, then exited before it answered",
            Self::NoAnswer => "started, but did not answer within a minute",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trial {
    /// The launcher copied the database aside. It does that once per trial, before the new
    /// version can migrate it, so a relaunch mid-trial never copies a migrated database.
    pub database_saved: bool,
}

impl State {
    /// Switch from `current` to `next`, which still has to prove healthy. The last failure, if
    /// `earlier` recorded one, is kept.
    pub fn update(current: &str, next: &str, earlier: Option<&Self>) -> Self {
        Self {
            active: next.into(),
            previous: Some(current.into()),
            trial: Some(Trial {
                database_saved: false,
            }),
            restore_database: false,
            failed: earlier.and_then(|earlier| earlier.failed.clone()),
            failed_reason: earlier.and_then(|earlier| earlier.failed_reason),
        }
    }

    /// Go back to `previous` after the active version failed its trial. `None` when there is
    /// nothing to go back to.
    pub fn failed_trial(self, reason: FailedTrial) -> Option<Self> {
        Some(Self {
            active: self.previous?,
            previous: None,
            trial: None,
            restore_database: false,
            failed: Some(self.active),
            failed_reason: Some(reason),
        })
    }

    /// Go back to `previous` on request, with the database as it was before the update.
    pub fn roll_back(self) -> Option<Self> {
        Some(Self {
            active: self.previous?,
            previous: None,
            trial: None,
            restore_database: true,
            failed: self.failed,
            failed_reason: self.failed_reason,
        })
    }
}

/// A version's binary.
pub fn binary(paths: &Paths, version: &str) -> PathBuf {
    paths.version(version).join(if cfg!(windows) {
        "copal.exe"
    } else {
        "copal"
    })
}

/// The health marker of a version.
pub fn marker(paths: &Paths, version: &str) -> PathBuf {
    paths.version(version).join("healthy")
}

/// The state, or `None` without one. An account that may not read `versions/` gets `None`
/// too, so the launcher runs its own binary, which says what is wrong.
pub fn load(paths: &Paths) -> io::Result<Option<State>> {
    match std::fs::read(paths.version_state()) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

/// Write the state whole and durably, or remove it with `None`. After a power loss the file is
/// the old state or the new one, never part of one.
pub fn save(paths: &Paths, state: Option<&State>) -> io::Result<()> {
    let path = paths.version_state();
    let Some(state) = state else {
        remove_file(&path)?;
        // A fresh install clears the state before the data folder exists: nothing to sync.
        if !paths.versions().exists() {
            return Ok(());
        }
        return sync_dir(&paths.versions());
    };
    std::fs::create_dir_all(paths.versions())?;
    let staged = path.with_extension("json.new");
    write_synced(&staged, &serde_json::to_vec_pretty(state)?)?;
    std::fs::rename(&staged, &path)?;
    sync_dir(&paths.versions())
}

/// Write a file and wait until its bytes are on disk.
fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    std::io::Write::write_all(&mut file, bytes)?;
    file.sync_all()
}

/// Copy a file and wait until the copy is on disk.
fn copy_synced(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::copy(from, to)?;
    std::fs::File::options().write(true).open(to)?.sync_all()
}

/// Make the renames and removals in `dir` durable. Windows cannot open a folder this way; NTFS
/// journals the rename itself.
fn sync_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    std::fs::File::open(dir)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

/// The database and its WAL files, which only make sense together.
fn database_files(dir: &Path) -> [PathBuf; 3] {
    ["copal.db", "copal.db-wal", "copal.db-shm"].map(|name| dir.join(name))
}

/// Copy the database aside before a new version migrates it. Only while no Core runs. The copy
/// is made in a staging folder and synced before it takes the place of the last one, so a copy
/// that exists is whole.
pub fn save_database(paths: &Paths) -> io::Result<()> {
    let copy = paths.database_before_update();
    let staged = copy.with_extension("new");
    remove_dir(&staged)?;
    std::fs::create_dir_all(&staged)?;
    let db = paths.database();
    let db_dir = db.parent().expect("the database has a folder");
    for (from, to) in database_files(db_dir).iter().zip(database_files(&staged))
    {
        if from.exists() {
            copy_synced(from, &to)?;
        }
    }
    sync_dir(&staged)?;
    remove_dir(&copy)?;
    std::fs::rename(&staged, &copy)?;
    sync_dir(db_dir)
}

/// Put back the database [`save_database`] copied. Only while no Core runs. Each file is copied
/// next to the live one, synced, then renamed over it, and the copy is left as it was, so a
/// restore that is cut short can be done again from the start.
pub fn restore_database(paths: &Paths) -> io::Result<()> {
    let copy = database_files(&paths.database_before_update());
    if !copy[0].exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no database was copied before the update",
        ));
    }
    let db = paths.database();
    let db_dir = db.parent().expect("the database has a folder");
    for (from, to) in copy.iter().zip(database_files(db_dir)) {
        if from.exists() {
            let name = to.file_name().expect("a database file has a name");
            let staged = to
                .with_file_name(format!("{}.restore", name.to_string_lossy()));
            copy_synced(from, &staged)?;
            std::fs::rename(&staged, &to)?;
        } else {
            remove_file(&to)?;
        }
    }
    sync_dir(db_dir)
}

fn remove_dir(path: &Path) -> io::Result<()> {
    match std::fs::remove_dir_all(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// Copy the running binary into its version folder, so an update has a version to roll back
/// to. The launcher's own binary may run outside `versions/`.
pub fn keep_running(paths: &Paths, version: &str) -> io::Result<()> {
    let target = binary(paths, version);
    if target.exists() {
        return Ok(());
    }
    let dir = paths.version(version);
    std::fs::create_dir_all(&dir)?;
    let staged = dir.join("copal.new");
    std::fs::copy(std::env::current_exe()?, &staged)?;
    std::fs::rename(staged, target)
}

/// Remove every version folder but `keep`, and downloads that never finished. A version whose
/// binary still runs a server's shim cannot go on Windows; it goes on a later start.
pub fn clean(paths: &Paths, keep: &[&str]) {
    let Ok(entries) = std::fs::read_dir(paths.versions()) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !entry.path().is_dir() || keep.contains(&name.as_ref()) {
            continue;
        }
        if let Err(error) = std::fs::remove_dir_all(entry.path()) {
            tracing::debug!(%error, version = %name, "kept an old version");
        }
    }
}

fn remove_file(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        crate::daemon::folders::create(&paths, None).unwrap();
        (dir, paths)
    }

    #[test]
    fn a_failed_trial_goes_back_and_remembers_the_failure() {
        let state = State::update("0.1.0", "0.2.0", None);
        assert_eq!(
            state.failed_trial(FailedTrial::NoAnswer),
            Some(State {
                active: "0.1.0".into(),
                previous: None,
                trial: None,
                restore_database: false,
                failed: Some("0.2.0".into()),
                failed_reason: Some(FailedTrial::NoAnswer),
            })
        );
    }

    #[test]
    fn a_state_from_a_launcher_without_failure_reasons_still_loads() {
        let state: State = serde_json::from_str(
            r#"{"active":"0.2.0","previous":null,"trial":null,"restore_database":false,"failed":"0.3.0"}"#,
        )
        .unwrap();
        assert_eq!(state.failed.as_deref(), Some("0.3.0"));
        assert_eq!(state.failed_reason, None);
    }

    #[test]
    fn a_rollback_on_request_restores_the_database() {
        let state = State {
            trial: None,
            ..State::update("0.1.0", "0.2.0", None)
        };
        let back = state.roll_back().unwrap();
        assert_eq!(back.active, "0.1.0");
        assert!(back.restore_database);
        assert_eq!(back.roll_back(), None);
    }

    #[test]
    fn the_database_copy_comes_back_whole() {
        let (_dir, paths) = paths();
        let db = paths.database();
        let wal = db.with_file_name("copal.db-wal");
        std::fs::write(&db, "before").unwrap();
        save_database(&paths).unwrap();
        // The new version migrates and leaves a WAL the old one never had.
        std::fs::write(&db, "migrated").unwrap();
        std::fs::write(&wal, "migrated wal").unwrap();
        restore_database(&paths).unwrap();
        assert_eq!(std::fs::read_to_string(&db).unwrap(), "before");
        assert!(!wal.exists());
    }

    #[test]
    fn state_is_saved_and_removed() {
        let (_dir, paths) = paths();
        assert_eq!(load(&paths).unwrap(), None);
        let state = State::update("0.1.0", "0.2.0", None);
        save(&paths, Some(&state)).unwrap();
        assert_eq!(load(&paths).unwrap(), Some(state));
        save(&paths, None).unwrap();
        assert_eq!(load(&paths).unwrap(), None);
    }

    #[test]
    fn clearing_the_state_before_the_folders_exist_is_fine() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("fresh"));
        save(&paths, None).unwrap();
    }

    #[test]
    fn cleaning_keeps_the_versions_asked_for() {
        let (_dir, paths) = paths();
        for version in ["0.1.0", "0.2.0", "0.3.0", "0.4.0.partial"] {
            std::fs::create_dir_all(paths.version(version)).unwrap();
        }
        save(&paths, Some(&State::update("0.2.0", "0.3.0", None))).unwrap();
        clean(&paths, &["0.2.0", "0.3.0"]);
        let mut left: Vec<String> = std::fs::read_dir(paths.versions())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(left, ["0.2.0", "0.3.0", "state.json"]);
    }
}
