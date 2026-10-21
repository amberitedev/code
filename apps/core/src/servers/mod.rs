//! Server records, lifecycle, the shim, console, and restarts.
//!
//! Core never starts Java itself. It writes a launch to `runs/<server>/` and starts a detached
//! shim (see `shim`), which runs Java and outlives Core. [`Servers`] keeps track of which servers
//! have a shim, adopts the shims it finds when Core starts, and records how every run ended.

pub mod console;
pub mod launch;
pub mod process;
pub mod records;
pub mod restarts;
pub mod shim;

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    process::Child,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use color_eyre::eyre::Result;
use sqlx::SqlitePool;
use tracing::{info, warn};

use crate::daemon::paths::Paths;
use records::{Ended, Outcome};
use shim::{Connection, ExitStatus, Message, RunRecord, ShimError};

/// Where a server with a shim is in its run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// The shim is starting Java.
    Starting,
    Running,
    /// Core sent `stop` and waits, with no time limit, for Java to exit.
    Stopping,
    /// A shim Core cannot talk to, left running: another Copal version started it (its shim
    /// protocol, when known). Stopping it from the game console, then starting it, upgrades it.
    Unmanaged(Option<u32>),
    /// Held while its loader or pack installs (see [`Hold`]). It cannot start until that ends.
    Installing,
}

/// Why a server action failed.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("there is no server {0}")]
    NotFound(String),
    #[error("the server is already running")]
    AlreadyRunning,
    #[error("the server is not running")]
    NotRunning,
    #[error("the server is still starting")]
    Starting,
    #[error("the server is installing; it can start once that is done")]
    Installing,
    #[error(
        "Core cannot reach this server's shim, so the server keeps running on its own. Stop it from the game, then start it again"
    )]
    Unmanaged,
    #[error("Core is out of server ports")]
    NoPorts,
    #[error(
        "accept the Minecraft EULA (https://aka.ms/MinecraftEULA) first: copal eula --accept"
    )]
    Eula,
    #[error(transparent)]
    NotReady(#[from] launch::NotReady),
    #[error("{0}")]
    StartFailed(String),
    #[error(transparent)]
    Shim(#[from] ShimError),
    #[error(transparent)]
    Other(#[from] color_eyre::Report),
}

impl From<sqlx::Error> for ServerError {
    fn from(error: sqlx::Error) -> Self {
        Self::Other(error.into())
    }
}

impl From<std::io::Error> for ServerError {
    fn from(error: std::io::Error) -> Self {
        Self::Other(error.into())
    }
}

/// A server with a shim.
struct Live {
    phase: Phase,
    /// The shim, when this Core started it. Kept to reap it once it exits.
    child: Option<Child>,
}

/// Every server's lifecycle. One per Core.
pub struct Servers {
    paths: Paths,
    db: SqlitePool,
    http: reqwest::Client,
    live: Mutex<HashMap<String, Live>>,
    /// Servers held for deletion, which never start again. Only taken while `live` is held.
    deleted: Mutex<HashSet<String>>,
    /// Servers killed while a restart waited for them to stop. The kill wins: they stay off.
    killed: Mutex<HashSet<String>>,
}

/// How long a new shim may take to start Java.
const START_TIMEOUT: Duration = Duration::from_secs(60);

/// How many console lines a crash keeps.
const CRASH_LINES: usize = 50;

impl Servers {
    /// Adopt every server whose shim still runs, record how the others' runs ended, and start
    /// watching for runs that end and shims that appear.
    pub async fn open(paths: Paths, db: SqlitePool) -> Result<Arc<Self>> {
        let http = crate::installs::download::client()?;
        let servers = Arc::new(Self {
            paths,
            db,
            http,
            live: Mutex::new(HashMap::new()),
            deleted: Mutex::default(),
            killed: Mutex::default(),
        });
        servers.scan().await?;
        tokio::spawn(Arc::clone(&servers).watch());
        Ok(servers)
    }

    fn live(&self) -> std::sync::MutexGuard<'_, HashMap<String, Live>> {
        self.live.lock().expect("not poisoned")
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    /// Where the server is in its run, or `None` without a shim.
    pub fn phase(&self, id: &str) -> Option<Phase> {
        self.live().get(id).map(|live| live.phase)
    }

    fn set_phase(&self, id: &str, phase: Phase) {
        if let Some(live) = self.live().get_mut(id) {
            live.phase = phase;
        }
    }

    /// Take over every shim that runs for a server Core does not track, instead of starting a
    /// second copy, and record the end of runs whose shim is gone. Runs at start and every second:
    /// a Core that died right after starting a shim leaves a run whose shim has not taken its lock
    /// yet, and that shim is adopted once it appears.
    async fn scan(&self) -> Result<()> {
        let boot_id = process::boot_id()?;
        for id in shim::running(&self.paths) {
            if self.live().contains_key(&id) {
                continue;
            }
            let Some(phase) = self.identify(&id, &boot_id).await else {
                continue;
            };
            match phase {
                Phase::Running => {
                    info!(server = %id, "adopted a running server")
                }
                _ => warn!(
                    server = %id,
                    "a server runs under a shim this Core cannot use; it is left running"
                ),
            }
            self.live().entry(id).or_insert(Live { phase, child: None });
        }
        // A run younger than START_TIMEOUT may belong to a shim still starting. The cutoff also
        // keeps this from closing a run a start in progress has just recorded.
        let cutoff =
            chrono::Utc::now().timestamp() - START_TIMEOUT.as_secs() as i64;
        for id in records::servers_with_open_runs(&self.db).await? {
            if !self.live().contains_key(&id)
                && !shim::is_running(&self.paths, &id)
            {
                self.finish(&id, cutoff).await;
            }
        }
        Ok(())
    }

    /// What the shim holding a server's lock is. A shim is trusted only when its run id, pid,
    /// start time, and boot all match. `None` while it has not written `run.json` yet, which it
    /// does within START_TIMEOUT of its launch; after that it is a shim Core cannot use.
    async fn identify(&self, id: &str, boot_id: &str) -> Option<Phase> {
        match shim::read_versioned::<RunRecord>(&self.paths.run_record(id)) {
            Ok(record)
                if record
                    .is_alive(boot_id, process::start_time(record.pid)) =>
            {
                Some(match Connection::open(&self.paths, id).await {
                    Ok(connection) if connection.run_id == record.run_id => {
                        Phase::Running
                    }
                    Err(ShimError::Incompatible(version)) => {
                        Phase::Unmanaged(Some(version))
                    }
                    _ => Phase::Unmanaged(None),
                })
            }
            Err(ShimError::Incompatible(version)) => {
                Some(Phase::Unmanaged(Some(version)))
            }
            _ if self.launch_is_recent(id) => None,
            _ => Some(Phase::Unmanaged(None)),
        }
    }

    /// Whether `launch.json` was written less than START_TIMEOUT ago, so its shim may still be
    /// starting.
    fn launch_is_recent(&self, id: &str) -> bool {
        std::fs::metadata(self.paths.launch_spec(id))
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|written| written.elapsed().ok())
            .is_some_and(|age| age < START_TIMEOUT)
    }

    /// Check every second for shims that exited, record how their runs ended, and adopt shims
    /// that appeared.
    async fn watch(self: Arc<Self>) {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        loop {
            tick.tick().await;
            let ids: Vec<String> = self
                .live()
                .iter()
                .filter(|(_, live)| {
                    !matches!(live.phase, Phase::Starting | Phase::Installing)
                })
                .map(|(id, _)| id.clone())
                .collect();
            for id in ids {
                if shim::is_running(&self.paths, &id) {
                    continue;
                }
                let child =
                    self.live().get_mut(&id).and_then(|live| live.child.take());
                if let Some(mut child) = child {
                    // The shim released its lock as it exited; this returns at once.
                    let _ =
                        tokio::task::spawn_blocking(move || child.wait()).await;
                }
                // Recorded before the server leaves `live`, so a restart waiting on it cannot
                // start the next run first.
                self.finish(&id, i64::MAX).await;
                self.live().remove(&id);
            }
            if let Err(error) = self.scan().await {
                warn!(%error, "could not look for running servers");
            }
        }
    }

    /// Record the end of every open run of a server whose shim is gone, among those started
    /// before `started_before`.
    async fn finish(&self, id: &str, started_before: i64) {
        let status: Option<ExitStatus> =
            shim::read_versioned(&self.paths.run_status(id)).ok();
        let runs = match records::open_runs(&self.db, id, started_before).await
        {
            Ok(runs) => runs,
            Err(error) => {
                warn!(server = %id, %error, "could not read open runs");
                return;
            }
        };
        for run_id in runs {
            let status =
                status.as_ref().filter(|status| status.run_id == run_id);
            let (outcome, exit_code) = records::outcome(status);
            let last_lines = (outcome == Outcome::Crashed).then(|| {
                let mut lines =
                    console::tail(&self.paths.console_log(id), CRASH_LINES)
                        .map(|(lines, _)| lines)
                        .unwrap_or_default();
                lines.extend(status.and_then(|status| status.error.clone()));
                lines.join("\n")
            });
            let ended = Ended {
                at: status.map_or_else(
                    || chrono::Utc::now().timestamp(),
                    |status| status.ended_at,
                ),
                outcome,
                exit_code,
                last_lines,
            };
            info!(server = %id, run = %run_id, ?outcome, ?exit_code, "run ended");
            if let Err(error) =
                records::end_run(&self.db, &run_id, &ended).await
            {
                warn!(server = %id, %error, "could not record the end of a run");
            }
        }
    }

    /// Start a server and wait until its shim runs Java.
    pub async fn start(&self, id: &str) -> Result<(), ServerError> {
        let server = records::get(&self.db, id)
            .await?
            .ok_or_else(|| ServerError::NotFound(id.into()))?;
        {
            let mut live = self.live();
            if live
                .get(id)
                .is_some_and(|live| live.phase == Phase::Installing)
            {
                return Err(ServerError::Installing);
            }
            // The lock also covers a shim this Core does not know, such as one a crashed Core
            // started a moment before it died.
            if live.contains_key(id) || shim::is_running(&self.paths, id) {
                return Err(ServerError::AlreadyRunning);
            }
            if self.deleted.lock().expect("not poisoned").contains(id) {
                return Err(ServerError::NotFound(id.into()));
            }
            live.insert(
                id.into(),
                Live {
                    phase: Phase::Starting,
                    child: None,
                },
            );
        }
        let result = self.launch(&server).await;
        let mut live = self.live();
        match result {
            Ok(()) => {
                if let Some(live) = live.get_mut(id) {
                    live.phase = Phase::Running;
                }
                info!(server = %id, "started");
                Ok(())
            }
            // A shim that holds the lock but never answered keeps its server running.
            Err(error) if shim::is_running(&self.paths, id) => {
                if let Some(live) = live.get_mut(id) {
                    live.phase = Phase::Unmanaged(None);
                }
                Err(error)
            }
            Err(error) => {
                if let Some(mut child) =
                    live.remove(id).and_then(|live| live.child)
                {
                    let _ = child.try_wait();
                }
                Err(error)
            }
        }
    }

    /// How the shim this Core started for a server exited, once it has.
    fn reap(
        &self,
        id: &str,
    ) -> std::io::Result<Option<std::process::ExitStatus>> {
        match self.live().get_mut(id).and_then(|live| live.child.as_mut()) {
            Some(child) => child.try_wait(),
            None => Ok(None),
        }
    }

    async fn launch(
        &self,
        server: &records::ServerRow,
    ) -> Result<(), ServerError> {
        let id = server.id.as_str();
        if crate::api::eula::accepted_at(&self.db).await?.is_none() {
            return Err(ServerError::Eula);
        }
        // The owner accepted for every server; `launch` writes `eula.txt` from the server's flag.
        records::accept_eula(&self.db, id).await?;
        let server = &records::ServerRow {
            eula_accepted: true,
            ..server.clone()
        };
        let run_id = uuid::Uuid::new_v4().to_string();
        let launch = launch::prepare(&self.paths, &self.http, server, &run_id)
            .await
            .map_err(|error| match error.downcast::<launch::NotReady>() {
                Ok(not_ready) => ServerError::NotReady(not_ready),
                Err(error) => ServerError::Other(error),
            })?;
        std::fs::create_dir_all(self.paths.run(id))?;
        // What an earlier run left must not be mistaken for this one's.
        for stale in [self.paths.run_record(id), self.paths.run_status(id)] {
            match std::fs::remove_file(stale) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                    return Err(error.into());
                }
                _ => {}
            }
        }
        shim::write_json(&self.paths.launch_spec(id), &launch)?;
        records::start_run(
            &self.db,
            id,
            &run_id,
            chrono::Utc::now().timestamp(),
        )
        .await?;
        let child = match shim::spawn(&self.paths, id) {
            Ok(child) => child,
            Err(error) => {
                self.finish(id, i64::MAX).await;
                return Err(error.into());
            }
        };
        if let Some(live) = self.live().get_mut(id) {
            live.child = Some(child);
        }
        let deadline = Instant::now() + START_TIMEOUT;
        loop {
            // Checked first: a Java that already exited is not a successful start, whatever
            // run.json says.
            if let Some(exit) = self.reap(id)? {
                self.finish(id, i64::MAX).await;
                let status: Option<ExitStatus> =
                    shim::read_versioned(&self.paths.run_status(id)).ok();
                let shim_log = std::fs::read_to_string(self.paths.shim_log(id))
                    .unwrap_or_default();
                let reason = status
                    .and_then(|status| status.error)
                    .or_else(|| shim_log.lines().last().map(str::to_owned))
                    .unwrap_or_else(|| format!("the shim exited ({exit})"));
                return Err(ServerError::StartFailed(reason));
            }
            // The shim writes run.json once Java runs, after its socket is open.
            if let Ok(record) =
                shim::read_versioned::<RunRecord>(&self.paths.run_record(id))
                && record.run_id == run_id
            {
                return Ok(());
            }
            if Instant::now() > deadline {
                return Err(ServerError::StartFailed(
                    "the shim did not answer within a minute".into(),
                ));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Send a message to a running server's shim.
    async fn tell(
        &self,
        id: &str,
        message: Message,
    ) -> Result<(), ServerError> {
        match self.phase(id) {
            None => Err(ServerError::NotRunning),
            Some(Phase::Starting) => Err(ServerError::Starting),
            Some(Phase::Installing) => Err(ServerError::Installing),
            Some(Phase::Unmanaged(Some(version))) => {
                Err(ShimError::Incompatible(version).into())
            }
            Some(Phase::Unmanaged(None)) => Err(ServerError::Unmanaged),
            Some(Phase::Running | Phase::Stopping) => {
                Connection::open(&self.paths, id)
                    .await?
                    .send(message)
                    .await?;
                Ok(())
            }
        }
    }

    /// Ask the server to stop. It stops when Java exits, however long that takes.
    pub async fn stop(&self, id: &str) -> Result<(), ServerError> {
        self.tell(id, Message::Stop).await?;
        self.set_phase(id, Phase::Stopping);
        Ok(())
    }

    /// Kill the server's Java now.
    pub async fn kill(&self, id: &str) -> Result<(), ServerError> {
        // Recorded first: a restart waiting on this server checks it the moment the shim exits.
        self.killed.lock().expect("not poisoned").insert(id.into());
        let sent = self.tell(id, Message::Kill).await;
        if sent.is_err() {
            self.killed.lock().expect("not poisoned").remove(id);
        }
        sent
    }

    /// A line for the server's console.
    pub async fn command(
        &self,
        id: &str,
        line: &str,
    ) -> Result<(), ServerError> {
        self.tell(
            id,
            Message::Command {
                line: line.to_owned(),
            },
        )
        .await
    }

    /// Wait, at most `timeout`, until no server is starting or stopping. The updater waits so
    /// a restart of Core never lands in the middle of one.
    pub async fn settle(&self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline
            && self.live().values().any(|live| {
                matches!(live.phase, Phase::Starting | Phase::Stopping)
            })
        {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    /// Stop the server, wait for its shim to exit, and start it again.
    /// A kill while it waits cancels the start.
    pub async fn restart(&self, id: &str) -> Result<(), ServerError> {
        self.killed.lock().expect("not poisoned").remove(id);
        self.stop(id).await?;
        while self.phase(id).is_some() {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        if self.killed.lock().expect("not poisoned").remove(id) {
            return Ok(());
        }
        self.start(id).await
    }

    /// Create an unconfigured server.
    pub async fn create(
        &self,
        name: &str,
    ) -> Result<records::ServerRow, ServerError> {
        let server = records::create(&self.db, name)
            .await?
            .ok_or(ServerError::NoPorts)?;
        crate::daemon::folders::create_server_folder(
            &self.paths.server(&server.id),
        )?;
        Ok(server)
    }

    /// Hold a stopped server for deletion, so it cannot start while its row and files go. Fails
    /// while it runs. `release` undoes it when the deletion does not happen.
    pub fn hold_for_deletion(&self, id: &str) -> Result<(), ServerError> {
        let live = self.live();
        if live.contains_key(id) || shim::is_running(&self.paths, id) {
            return Err(ServerError::AlreadyRunning);
        }
        self.deleted.lock().expect("not poisoned").insert(id.into());
        Ok(())
    }

    pub fn release(&self, id: &str) {
        let _live = self.live();
        self.deleted.lock().expect("not poisoned").remove(id);
    }

    /// Remove a held server's world and run files, once its row is gone.
    pub fn delete_folders(&self, id: &str) -> Result<(), ServerError> {
        for folder in [self.paths.server(id), self.paths.run(id)] {
            match std::fs::remove_dir_all(&folder) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                    return Err(error.into());
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Hold a stopped server while its content changes: it cannot start, and no second hold is
    /// taken, until the hold drops.
    pub fn hold(self: &Arc<Self>, id: &str) -> Result<Hold, ServerError> {
        let mut live = self.live();
        match live.get(id).map(|live| live.phase) {
            Some(Phase::Installing) => return Err(ServerError::Installing),
            Some(_) => return Err(ServerError::AlreadyRunning),
            None if shim::is_running(&self.paths, id) => {
                return Err(ServerError::AlreadyRunning);
            }
            None => {}
        }
        if self.deleted.lock().expect("not poisoned").contains(id) {
            return Err(ServerError::NotFound(id.into()));
        }
        live.insert(
            id.into(),
            Live {
                phase: Phase::Installing,
                child: None,
            },
        );
        Ok(Hold {
            servers: Arc::clone(self),
            id: id.into(),
        })
    }

    /// Run an installer, such as Forge's, through a shim like a server run: as `copal-servers`,
    /// in the server folder, its output in `console.log`. Waits for it to exit; fails unless it
    /// exits with 0. Not recorded as a run.
    pub async fn run_installer(
        &self,
        hold: &Hold,
        java: PathBuf,
        args: Vec<String>,
    ) -> Result<(), ServerError> {
        let id = hold.id.as_str();
        let run_id = uuid::Uuid::new_v4().to_string();
        let dir = self.paths.server(id);
        crate::daemon::folders::create_server_folder(&dir)?;
        crate::daemon::folders::create_server_folder(&dir.join("tmp"))?;
        std::fs::create_dir_all(self.paths.run(id))?;
        for stale in [self.paths.run_record(id), self.paths.run_status(id)] {
            match std::fs::remove_file(stale) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                    return Err(error.into());
                }
                _ => {}
            }
        }
        let mut full_args = vec![
            "-Xmx1G".to_owned(),
            format!("-Djava.io.tmpdir={}", dir.join("tmp").display()),
            format!("-Duser.home={}", dir.display()),
        ];
        full_args.extend(args);
        shim::write_json(
            &self.paths.launch_spec(id),
            &shim::Launch {
                v: shim::PROTOCOL,
                run_id: run_id.clone(),
                java,
                args: full_args,
                dir,
                memory_limit_mb: launch::memory_limit(1024),
            },
        )?;
        let mut child = shim::spawn(&self.paths, id)?;
        tokio::task::spawn_blocking(move || child.wait())
            .await
            .map_err(std::io::Error::other)??;
        let status =
            shim::read_versioned::<ExitStatus>(&self.paths.run_status(id))
                .ok()
                .filter(|status| status.run_id == run_id);
        match status {
            Some(ExitStatus {
                exit_code: Some(0), ..
            }) => Ok(()),
            status => {
                let lines = console::tail(&self.paths.console_log(id), 5)
                    .map(|(lines, _)| {
                        lines.join(
                            "
",
                        )
                    })
                    .unwrap_or_default();
                let reason = status
                    .as_ref()
                    .and_then(|status| status.error.clone())
                    .or_else(|| {
                        status.and_then(|status| status.exit_code).map(|code| {
                            format!("the installer exited with {code}")
                        })
                    })
                    .unwrap_or_else(|| "the installer did not finish".into());
                Err(ServerError::StartFailed(format!(
                    "{reason}
{lines}"
                )))
            }
        }
    }
}

/// A server held for an install. See [`Servers::hold`].
pub struct Hold {
    servers: Arc<Servers>,
    id: String,
}

impl Hold {
    pub fn id(&self) -> &str {
        &self.id
    }
}

impl Drop for Hold {
    fn drop(&mut self) {
        let mut live = self.servers.live();
        if live
            .get(&self.id)
            .is_some_and(|live| live.phase == Phase::Installing)
        {
            live.remove(&self.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    async fn servers() -> (tempfile::TempDir, Arc<Servers>) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        crate::daemon::folders::create(&paths, None).unwrap();
        let servers = Servers::open(paths, db::memory().await).await.unwrap();
        (dir, servers)
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_link_planted_in_a_server_folder_is_not_followed() {
        use std::os::unix::fs::PermissionsExt;

        let (dir, servers) = servers().await;
        let server = servers.create("A").await.unwrap();
        records::set_installed(
            &servers.db,
            &server.id,
            "vanilla",
            "1.21.1",
            None,
        )
        .await
        .unwrap();
        crate::api::eula::accept(&servers.db).await.unwrap();
        // A mod points `tmp` at a store it may only read.
        let store = dir.path().join("java");
        std::fs::set_permissions(
            &store,
            std::fs::Permissions::from_mode(0o750),
        )
        .unwrap();
        std::os::unix::fs::symlink(
            &store,
            servers.paths().server(&server.id).join("tmp"),
        )
        .unwrap();

        let error = servers.start(&server.id).await.unwrap_err();
        assert!(
            error.to_string().contains("is a link or not a folder"),
            "{error}"
        );
        let mode = std::fs::metadata(&store).unwrap().permissions().mode();
        assert_eq!(mode & 0o7777, 0o750);
        assert_eq!(servers.phase(&server.id), None);
    }

    #[tokio::test]
    async fn a_server_held_for_deletion_never_starts() {
        let (_dir, servers) = servers().await;
        let server = servers.create("A").await.unwrap();
        servers.hold_for_deletion(&server.id).unwrap();
        assert!(matches!(
            servers.start(&server.id).await,
            Err(ServerError::NotFound(_))
        ));
        servers.release(&server.id);
        assert!(!matches!(
            servers.start(&server.id).await,
            Err(ServerError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn a_shim_started_by_a_core_that_died_is_adopted_when_it_appears() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        crate::daemon::folders::create(&paths, None).unwrap();
        let db = db::memory().await;
        let server = records::create(&db, "A").await.unwrap().unwrap();
        // The dead Core recorded the run and wrote the launch; its shim has not started yet.
        let now = chrono::Utc::now().timestamp();
        records::start_run(&db, &server.id, "late", now)
            .await
            .unwrap();
        std::fs::create_dir_all(paths.run(&server.id)).unwrap();
        shim::write_json(
            &paths.launch_spec(&server.id),
            &shim::Launch {
                v: shim::PROTOCOL,
                run_id: "late".into(),
                java: "java".into(),
                args: Vec::new(),
                dir: paths.server(&server.id),
                memory_limit_mb: 1024,
            },
        )
        .unwrap();

        let servers = Servers::open(paths.clone(), db.clone()).await.unwrap();
        assert_eq!(servers.phase(&server.id), None);
        assert_eq!(
            records::open_runs(&db, &server.id, i64::MAX).await.unwrap(),
            ["late"]
        );

        // The shim takes its lock and records itself, as this test process.
        let _lock = shim::try_lock(&paths, &server.id).unwrap().unwrap();
        let pid = std::process::id();
        shim::write_json(
            &paths.run_record(&server.id),
            &RunRecord {
                v: shim::PROTOCOL,
                run_id: "late".into(),
                pid,
                start_time: process::start_time(pid).unwrap(),
                boot_id: process::boot_id().unwrap(),
                java_pid: pid,
            },
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while servers.phase(&server.id).is_none() && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(
            servers.phase(&server.id).is_some(),
            "the late shim was ignored"
        );
        assert!(matches!(
            servers.start(&server.id).await,
            Err(ServerError::AlreadyRunning)
        ));
        assert_eq!(
            records::open_runs(&db, &server.id, i64::MAX).await.unwrap(),
            ["late"]
        );
    }

    #[tokio::test]
    async fn a_held_lock_means_running_and_no_second_start() {
        let (_dir, servers) = servers().await;
        let server = servers.create("A").await.unwrap();
        let _shim = shim::try_lock(servers.paths(), &server.id)
            .unwrap()
            .unwrap();
        assert!(matches!(
            servers.start(&server.id).await,
            Err(ServerError::AlreadyRunning)
        ));
    }

    #[tokio::test]
    async fn a_shim_of_another_protocol_is_left_running() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let db = db::memory().await;
        let server = records::create(&db, "A").await.unwrap().unwrap();
        let _shim = shim::try_lock(&paths, &server.id).unwrap().unwrap();
        std::fs::write(
            paths.run_record(&server.id),
            r#"{"v":2,"run_id":"newer","pid":1}"#,
        )
        .unwrap();

        let servers = Servers::open(paths, db).await.unwrap();
        assert_eq!(servers.phase(&server.id), Some(Phase::Unmanaged(Some(2))));
        assert!(matches!(
            servers.stop(&server.id).await,
            Err(ServerError::Shim(ShimError::Incompatible(2)))
        ));
        assert!(matches!(
            servers.start(&server.id).await,
            Err(ServerError::AlreadyRunning)
        ));
    }

    #[tokio::test]
    async fn runs_left_open_by_a_dead_shim_are_recorded_on_start() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let db = db::memory().await;
        let server = records::create(&db, "A").await.unwrap().unwrap();
        records::start_run(&db, &server.id, "killed", 1)
            .await
            .unwrap();
        records::start_run(&db, &server.id, "vanished", 2)
            .await
            .unwrap();
        std::fs::create_dir_all(paths.run(&server.id)).unwrap();
        std::fs::write(paths.console_log(&server.id), "booting\nboom\n")
            .unwrap();
        shim::write_json(
            &paths.run_status(&server.id),
            &ExitStatus {
                v: shim::PROTOCOL,
                run_id: "killed".into(),
                exit_code: Some(1),
                ended_at: 5,
                ended_by: shim::EndedBy::Kill,
                error: None,
            },
        )
        .unwrap();

        let servers = Servers::open(paths, db.clone()).await.unwrap();
        assert_eq!(servers.phase(&server.id), None);
        let runs: Vec<(String, Outcome, Option<String>)> = sqlx::query_as(
            "SELECT id, outcome, last_lines FROM runs ORDER BY started_at",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        assert_eq!(
            runs,
            [
                ("killed".into(), Outcome::Killed, None),
                (
                    "vanished".into(),
                    Outcome::Crashed,
                    Some("booting\nboom".into())
                ),
            ]
        );
    }
}
