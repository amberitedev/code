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
    /// The last version that failed its health check and was rolled back from. Auto-update
    /// skips it.
    pub failed: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trial {
    /// The launcher copied the database aside. It does that once per trial, before the new
    /// version can migrate it, so a relaunch mid-trial never copies a migrated database.
    pub database_saved: bool,
}

impl State {
    /// Switch from `current` to `next`, which still has to prove healthy.
    pub fn update(current: &str, next: &str, failed: Option<String>) -> Self {
        Self {
            active: next.into(),
            previous: Some(current.into()),
            trial: Some(Trial {
                database_saved: false,
            }),
            restore_database: false,
            failed,
        }
    }

    /// Go back to `previous` after the active version failed its trial. `None` when there is
    /// nothing to go back to.
    pub fn failed_trial(self) -> Option<Self> {
        Some(Self {
            active: self.previous?,
            previous: None,
            trial: None,
            restore_database: false,
            failed: Some(self.active),
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

/// Write the state whole, or remove it with `None`.
pub fn save(paths: &Paths, state: Option<&State>) -> io::Result<()> {
    let path = paths.version_state();
    let Some(state) = state else {
        return remove_file(&path);
    };
    std::fs::create_dir_all(paths.versions())?;
    let staged = path.with_extension("json.new");
    std::fs::write(&staged, serde_json::to_vec_pretty(state)?)?;
    std::fs::rename(&staged, &path)
}

/// The database and its WAL files, which only make sense together.
fn database_files(dir: &Path) -> [PathBuf; 3] {
    ["copal.db", "copal.db-wal", "copal.db-shm"].map(|name| dir.join(name))
}

/// Copy the database aside before a new version migrates it. Only while no Core runs.
pub fn save_database(paths: &Paths) -> io::Result<()> {
    let copy = paths.database_before_update();
    match std::fs::remove_dir_all(&copy) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            return Err(error);
        }
        _ => {}
    }
    std::fs::create_dir_all(&copy)?;
    let db = paths.database();
    let live = database_files(db.parent().expect("the database has a folder"));
    for (from, to) in live.iter().zip(database_files(&copy)) {
        if from.exists() {
            std::fs::copy(from, to)?;
        }
    }
    Ok(())
}

/// Put back the database [`save_database`] copied. Only while no Core runs.
pub fn restore_database(paths: &Paths) -> io::Result<()> {
    let copy = database_files(&paths.database_before_update());
    if !copy[0].exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no database was copied before the update",
        ));
    }
    let db = paths.database();
    let live = database_files(db.parent().expect("the database has a folder"));
    for (from, to) in copy.iter().zip(live) {
        if from.exists() {
            std::fs::copy(from, &to)?;
        } else {
            remove_file(&to)?;
        }
    }
    Ok(())
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
            state.failed_trial(),
            Some(State {
                active: "0.1.0".into(),
                previous: None,
                trial: None,
                restore_database: false,
                failed: Some("0.2.0".into()),
            })
        );
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
