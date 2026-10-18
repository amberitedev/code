//! Server records, lifecycle, the shim, console, and restarts.
//!
//! Core never starts Java itself. It writes a launch to `runs/<server>/` and starts a detached
//! shim (see `shim`), which runs Java and outlives Core. [`Servers`] keeps track of which servers
//! have a shim, adopts the shims it finds when Core starts, and records how every run ended.

pub mod console;
pub mod launch;
pub mod process;
pub mod records;
pub mod shim;

use std::{
    collections::HashMap,
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
    #[error(
        "Core cannot reach this server's shim, so the server keeps running on its own. Stop it from the game, then start it again"
    )]
    Unmanaged,
    #[error("Core is out of server ports")]
    NoPorts,
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
}

/// How long a new shim may take to start Java.
const START_TIMEOUT: Duration = Duration::from_secs(60);

/// How many console lines a crash keeps.
const CRASH_LINES: usize = 50;

impl Servers {
    /// Adopt every server whose shim still runs, record how the others' runs ended, and start
    /// watching for runs that end.
    pub async fn open(paths: Paths, db: SqlitePool) -> Result<Arc<Self>> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("copal/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(15))
            .build()?;
        let servers = Arc::new(Self {
            paths,
            db,
            http,
            live: Mutex::new(HashMap::new()),
        });
        servers.adopt().await?;
        for id in records::servers_with_open_runs(&servers.db).await? {
            if !servers.live().contains_key(&id) {
                servers.finish(&id).await;
            }
        }
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

    /// Take over every server a shim still runs for, instead of starting a second copy. A
    /// shim is trusted only when its run id, pid, start time, and boot all match.
    async fn adopt(&self) -> Result<()> {
        let boot_id = process::boot_id()?;
        for id in shim::running(&self.paths) {
            let phase = self.identify(&id, &boot_id).await;
            match phase {
                Phase::Running => {
                    info!(server = %id, "adopted a running server")
                }
                _ => warn!(
                    server = %id,
                    "a server runs under a shim this Core cannot use; it is left running"
                ),
            }
            self.live().insert(id, Live { phase, child: None });
        }
        Ok(())
    }

    /// What the shim holding a server's lock is. A shim that holds the lock but has not yet
    /// written `run.json` is starting; it gets a few seconds.
    async fn identify(&self, id: &str, boot_id: &str) -> Phase {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match shim::read_versioned::<RunRecord>(&self.paths.run_record(id))
            {
                Ok(record)
                    if record
                        .is_alive(boot_id, process::start_time(record.pid)) =>
                {
                    return match Connection::open(&self.paths, id).await {
                        Ok(connection)
                            if connection.run_id == record.run_id =>
                        {
                            Phase::Running
                        }
                        Err(ShimError::Incompatible(version)) => {
                            Phase::Unmanaged(Some(version))
                        }
                        _ => Phase::Unmanaged(None),
                    };
                }
                Err(ShimError::Incompatible(version)) => {
                    return Phase::Unmanaged(Some(version));
                }
                _ if Instant::now() > deadline => {
                    return Phase::Unmanaged(None);
                }
                _ => tokio::time::sleep(Duration::from_millis(200)).await,
            }
        }
    }

    /// Check every second for shims that exited, and record how their runs ended.
    async fn watch(self: Arc<Self>) {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        loop {
            tick.tick().await;
            let ids: Vec<String> = self
                .live()
                .iter()
                .filter(|(_, live)| live.phase != Phase::Starting)
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
                self.finish(&id).await;
                self.live().remove(&id);
            }
        }
    }

    /// Record the end of every open run of a server whose shim is gone.
    async fn finish(&self, id: &str) {
        let status: Option<ExitStatus> =
            shim::read_versioned(&self.paths.run_status(id)).ok();
        let runs = match records::open_runs(&self.db, id).await {
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
            // The lock also covers a shim this Core does not know, such as one a crashed Core
            // started a moment before it died.
            if live.contains_key(id) || shim::is_running(&self.paths, id) {
                return Err(ServerError::AlreadyRunning);
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
                self.finish(id).await;
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
                self.finish(id).await;
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
        self.tell(id, Message::Kill).await
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

    /// Stop the server, wait for its shim to exit, and start it again.
    pub async fn restart(&self, id: &str) -> Result<(), ServerError> {
        self.stop(id).await?;
        while self.phase(id).is_some() {
            tokio::time::sleep(Duration::from_millis(250)).await;
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

    /// Install Vanilla `game_version` on a stopped server.
    pub async fn install_vanilla(
        &self,
        id: &str,
        game_version: &str,
    ) -> Result<(), ServerError> {
        records::get(&self.db, id)
            .await?
            .ok_or_else(|| ServerError::NotFound(id.into()))?;
        if self.phase(id).is_some() {
            return Err(ServerError::AlreadyRunning);
        }
        launch::install_vanilla(&self.paths, &self.http, game_version).await?;
        records::set_installed(&self.db, id, "vanilla", game_version, None)
            .await?;
        info!(server = %id, game_version, "installed Vanilla");
        Ok(())
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
        records::accept_eula(&servers.db, &server.id).await.unwrap();
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
