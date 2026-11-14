//! Server records, lifecycle, the shim, console, and restarts.
//!
//! Core never starts Java itself. It writes a launch to `runs/<server>/` and starts a detached
//! shim (see `shim`), which runs Java and outlives Core. [`Servers`] keeps track of which servers
//! have a shim, adopts the shims it finds when Core starts, and records how every run ended.
//! While a server runs, Core reads its console for players and freezes (see `watch`), restarts
//! it after a crash, and tells [`Servers::events`] subscribers, such as port forwarding.

pub mod console;
pub mod insight;
pub mod launch;
pub mod memory;
pub mod platform;
pub mod process;
pub mod properties;
pub mod restarts;
pub mod server_rows;
pub mod shim;
mod watch;

use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
    process::Child,
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};

use color_eyre::eyre::Result;
use engine::Engine;
use sqlx::SqlitePool;
use tokio::sync::broadcast;
use tracing::{info, warn};

use self::server_rows::{Ended, Outcome};
use crate::{daemon::paths::Paths, util::if_present};
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

/// A shim answered with something else than the answer asked for.
fn unexpected(message: Message) -> ServerError {
    ShimError::Invalid(format!("{message:?} is not the answer asked for"))
        .into()
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
        "a backup is copying the server's world; stop or restart it once the backup is done"
    )]
    BackingUp,
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
    /// Each server is the engine instance with its id, which holds its content.
    engine: Arc<Engine>,
    /// See `Config::local_only`.
    local_only: Option<IpAddr>,
    live: Mutex<HashMap<String, Live>>,
    /// Servers held for deletion, which never start again. Only taken while `live` is held.
    deleted: Mutex<HashSet<String>>,
    /// Servers killed while a restart waited for them to stop. The kill wins: they stay off.
    killed: Mutex<HashSet<String>>,
    /// Running servers a hot backup is copying (see [`Servers::copying`]).
    copying: Mutex<HashSet<String>>,
    /// Itself, for the tasks a run starts.
    me: Weak<Servers>,
    /// Servers starting to run and runs ending.
    pub events: broadcast::Sender<Lifecycle>,
    /// What each running server's console said. See `watch`.
    insight: Mutex<HashMap<String, watch::Watched>>,
    /// Servers that crashed too often to start again. Cleared by the next start.
    gave_up: Mutex<HashSet<String>>,
    /// Crash restarts waiting to start: server to the run that crashed and the owner-action
    /// generation it was queued under. See `watch`.
    retries: Mutex<HashMap<String, (String, u64)>>,
    /// How many owner actions each server has seen. Only taken while `live` is held.
    owner_actions: Mutex<HashMap<String, u64>>,
}

/// A change in which servers run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lifecycle {
    /// A run started, or Core adopted one.
    Running(String),
    Ended(String),
}

/// A running server taken by a hot backup until dropped: stopping or restarting it is refused,
/// since its shutdown save would rewrite the world under the copy, and a start waits. A kill
/// still works; it saves nothing.
pub struct Copying {
    servers: Arc<Servers>,
    id: String,
}

impl Drop for Copying {
    fn drop(&mut self) {
        self.servers
            .copying
            .lock()
            .expect("not poisoned")
            .remove(&self.id);
    }
}

/// How long a new shim may take to start Java.
const START_TIMEOUT: Duration = Duration::from_secs(60);

/// How many console lines a crash keeps.
const CRASH_LINES: usize = 50;

impl Servers {
    /// Adopt every server whose shim still runs, record how the others' runs ended, and start
    /// watching for runs that end and shims that appear.
    pub async fn open(
        paths: Paths,
        db: SqlitePool,
        engine: Arc<Engine>,
        local_only: Option<IpAddr>,
    ) -> Result<Arc<Self>> {
        let servers = Arc::new_cyclic(|me| Self {
            paths,
            db,
            engine,
            local_only,
            live: Mutex::new(HashMap::new()),
            deleted: Mutex::default(),
            killed: Mutex::default(),
            copying: Mutex::default(),
            me: me.clone(),
            events: broadcast::channel(64).0,
            insight: Mutex::default(),
            gave_up: Mutex::default(),
            retries: Mutex::default(),
            owner_actions: Mutex::default(),
        });
        servers.remove_orphans().await?;
        servers.scan().await?;
        tokio::spawn(Arc::clone(&servers).watch());
        Ok(servers)
    }

    /// Remove the engine instances and server folders that have no server row: what a deletion
    /// left when a file could not go, or when Core stopped between the two databases. A new
    /// server's instance is made before its row, so a create cut short leaves the same.
    async fn remove_orphans(&self) -> Result<()> {
        let rows: HashSet<String> = server_rows::list(&self.db)
            .await?
            .into_iter()
            .map(|row| row.id)
            .collect();
        let mut ids: HashSet<String> = engine::instance::list(&self.engine)
            .await
            .map_err(|error| color_eyre::eyre::eyre!("{error:#}"))?
            .into_iter()
            .map(|instance| instance.instance.id)
            .collect();
        if let Ok(folders) = std::fs::read_dir(self.paths.servers()) {
            ids.extend(
                folders
                    .filter_map(Result::ok)
                    .filter_map(|folder| folder.file_name().into_string().ok()),
            );
        }
        for id in ids.difference(&rows) {
            if shim::is_running(&self.paths, id) {
                continue;
            }
            match self.delete_folders(id).await {
                Ok(()) => {
                    info!(server = %id, "removed a deleted server's files")
                }
                Err(error) => {
                    warn!(server = %id, %error, "could not remove a deleted server's files");
                }
            }
        }
        Ok(())
    }

    fn live(&self) -> std::sync::MutexGuard<'_, HashMap<String, Live>> {
        self.live.lock().expect("not poisoned")
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Where the server is in its run, or `None` without a shim.
    pub fn phase(&self, id: &str) -> Option<Phase> {
        self.live().get(id).map(|live| live.phase)
    }

    /// Every server with a shim, and where it is in its run.
    pub fn phases(&self) -> HashMap<String, Phase> {
        self.live()
            .iter()
            .map(|(id, live)| (id.clone(), live.phase))
            .collect()
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
            let running = phase == Phase::Running;
            self.live()
                .entry(id.clone())
                .or_insert(Live { phase, child: None });
            if running {
                self.on_running(&id, true);
            }
        }
        // A run younger than START_TIMEOUT may belong to a shim still starting. The cutoff also
        // keeps this from closing a run a start in progress has just recorded.
        let cutoff =
            chrono::Utc::now().timestamp() - START_TIMEOUT.as_secs() as i64;
        for id in server_rows::servers_with_open_runs(&self.db).await? {
            if !self.live().contains_key(&id)
                && !shim::is_running(&self.paths, &id)
            {
                self.finish(&id, cutoff, None).await;
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
                self.finish(&id, i64::MAX, None).await;
                let generation = {
                    let mut live = self.live();
                    live.remove(&id);
                    self.generation(&id)
                };
                self.ended(&id, generation).await;
            }
            if let Err(error) = self.scan().await {
                warn!(%error, "could not look for running servers");
            }
        }
    }

    /// Record the end of every open run of a server whose shim is gone, among those started
    /// before `started_before`, or only run `only`.
    async fn finish(&self, id: &str, started_before: i64, only: Option<&str>) {
        let status: Option<ExitStatus> =
            shim::read_versioned(&self.paths.run_status(id)).ok();
        let runs =
            match server_rows::open_runs(&self.db, id, started_before).await {
                Ok(runs) => runs,
                Err(error) => {
                    warn!(server = %id, %error, "could not read open runs");
                    return;
                }
            };
        for run_id in runs
            .into_iter()
            .filter(|run| only.is_none_or(|only| run == only))
        {
            let status =
                status.as_ref().filter(|status| status.run_id == run_id);
            let (outcome, exit_code) = server_rows::outcome(status);
            let last_lines = (outcome == Outcome::Crashed).then(|| {
                let mut lines: Vec<String> =
                    console::tail(&self.paths.console_log(id), CRASH_LINES)
                        .map(|(lines, _)| lines)
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|line| console::for_people(line))
                        .collect();
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
                server_rows::end_run(&self.db, &run_id, &ended).await
            {
                warn!(server = %id, %error, "could not record the end of a run");
            }
        }
    }

    /// Start a server and wait until its shim runs Java. Replaces a pending crash restart.
    pub async fn start(&self, id: &str) -> Result<(), ServerError> {
        self.start_inner(id, None).await
    }

    /// [`start`](Self::start), or with `crashed`, the crash restart queued for that run, which
    /// does nothing when it was cancelled meanwhile.
    async fn start_inner(
        &self,
        id: &str,
        crashed: Option<&str>,
    ) -> Result<(), ServerError> {
        // A server that exited during a hot backup, such as a crash restart, starts once the
        // copy is done. A copy ends within the shim's ten-minute hold.
        while self.is_copying(id) {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        let server = server_rows::get(&self.db, id)
            .await?
            .ok_or_else(|| ServerError::NotFound(id.into()))?;
        if self.engine_installing(id).await? {
            return Err(ServerError::Installing);
        }
        {
            let mut live = self.live();
            match crashed {
                None => {
                    self.owner_acted_locked(id);
                }
                // Taken under the lock owner actions take, so one that came first wins.
                Some(run) => {
                    let retry = self.retries().remove(id);
                    if retry != Some((run.to_owned(), self.generation(id))) {
                        return Ok(());
                    }
                }
            }
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
        let run_id = uuid::Uuid::new_v4().to_string();
        let result = self.launch(&server, &run_id).await;
        match result {
            Ok(()) => {
                self.set_phase(id, Phase::Running);
                info!(server = %id, "started");
                self.gave_up.lock().expect("not poisoned").remove(id);
                self.on_running(id, false);
                Ok(())
            }
            // A shim that holds the lock but never answered keeps its server running. The
            // watcher sees it end like any run.
            Err(error) if shim::is_running(&self.paths, id) => {
                self.set_phase(id, Phase::Unmanaged(None));
                Err(error)
            }
            Err(error) => {
                self.start_failed(id, &run_id).await;
                Err(error)
            }
        }
    }

    /// A start that failed with no shim left: record how run `run_id` ended, if it got that
    /// far, let go of the server, and handle a crash as any other. The watcher skips starting
    /// servers, so this is the one place a crash while starting is seen. The end is recorded
    /// while the start still holds the server, and only for this run, so it cannot close a run
    /// a later start recorded.
    async fn start_failed(&self, id: &str, run_id: &str) {
        self.finish(id, i64::MAX, Some(run_id)).await;
        let (child, generation) = {
            let mut live = self.live();
            (
                live.remove(id).and_then(|live| live.child),
                self.generation(id),
            )
        };
        if let Some(mut child) = child {
            let _ = child.try_wait();
        }
        self.after_crash(id, run_id, generation).await;
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

    /// Whether the engine has an install job on the server, such as one it took up again after
    /// Core restarted.
    async fn engine_installing(&self, id: &str) -> Result<bool, ServerError> {
        engine::install::store::list_active_for_instance(id, self.engine.pool())
            .await
            .map(|jobs| !jobs.is_empty())
            .map_err(|error| {
                ServerError::Other(color_eyre::eyre::eyre!("{error:#}"))
            })
    }

    async fn launch(
        &self,
        server: &server_rows::ServerRow,
        run_id: &str,
    ) -> Result<(), ServerError> {
        let id = server.id.as_str();
        if crate::api::eula::accepted_at(&self.db).await?.is_none() {
            return Err(ServerError::Eula);
        }
        let platform =
            platform::platform(&self.engine, id)
                .await
                .map_err(|error| {
                    ServerError::Other(color_eyre::eyre::eyre!("{error:#}"))
                })?;
        let launch = launch::prepare(
            &self.paths,
            &self.engine,
            server,
            platform.as_ref(),
            memory::flags(&self.db, id).await?,
            run_id,
            self.local_only,
        )
        .await
        .map_err(|error| {
            match error.downcast::<launch::NotReady>() {
                Ok(not_ready) => ServerError::NotReady(not_ready),
                Err(error) => ServerError::Other(error),
            }
        })?;
        self.open_meta().await?;
        std::fs::create_dir_all(self.paths.run(id))?;
        // What an earlier run left must not be mistaken for this one's.
        for stale in [self.paths.run_record(id), self.paths.run_status(id)] {
            if_present(std::fs::remove_file(stale))?;
        }
        shim::write_json(&self.paths.launch_spec(id), &launch)?;
        server_rows::start_run(
            &self.db,
            id,
            run_id,
            chrono::Utc::now().timestamp(),
        )
        .await?;
        let child = match shim::spawn(&self.paths, id) {
            Ok(child) => child,
            Err(error) => {
                self.finish(id, i64::MAX, Some(run_id)).await;
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
                self.finish(id, i64::MAX, Some(run_id)).await;
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

    /// Let Core read every file the server's account wrote in its folder, such as the 0600
    /// `level.dat` Minecraft writes (see `shim::java::share_with_core`). Nothing to do in
    /// development, where servers run as Core's own account.
    pub async fn share_with_core(&self, id: &str) -> Result<(), ServerError> {
        if crate::daemon::install::InstallRecord::load(&self.paths)?.is_none() {
            return Ok(());
        }
        let dir = self.paths.server(id);
        tokio::task::spawn_blocking(move || shim::java::share_with_core(&dir))
            .await
            .map_err(std::io::Error::other)??;
        Ok(())
    }

    /// Let servers read and run the engine's game files in `meta/` (see
    /// `daemon::folders::open_to_servers`). Nothing to do in development.
    async fn open_meta(&self) -> Result<(), ServerError> {
        if crate::daemon::install::InstallRecord::load(&self.paths)?.is_none() {
            return Ok(());
        }
        let meta = self.paths.meta();
        tokio::task::spawn_blocking(move || {
            crate::daemon::folders::open_to_servers(&meta)
        })
        .await
        .map_err(std::io::Error::other)??;
        Ok(())
    }

    /// Take a running server for a hot backup's copy (see [`Copying`]).
    pub fn copying(self: &Arc<Self>, id: &str) -> Copying {
        self.copying
            .lock()
            .expect("not poisoned")
            .insert(id.to_owned());
        Copying {
            servers: Arc::clone(self),
            id: id.to_owned(),
        }
    }

    fn is_copying(&self, id: &str) -> bool {
        self.copying.lock().expect("not poisoned").contains(id)
    }

    /// Ask the server to stop. It stops when Java exits, however long that takes. A server
    /// waiting to start again after a crash stays stopped. Refused while a hot backup copies it.
    pub async fn stop(&self, id: &str) -> Result<(), ServerError> {
        if self.is_copying(id) {
            return Err(ServerError::BackingUp);
        }
        let cancelled = self.owner_acted(id);
        match self.tell(id, Message::Stop).await {
            Err(ServerError::NotRunning) if cancelled => return Ok(()),
            sent => sent?,
        }
        self.set_phase(id, Phase::Stopping);
        Ok(())
    }

    /// Kill the server's Java now. A server waiting to start again after a crash stays stopped.
    pub async fn kill(&self, id: &str) -> Result<(), ServerError> {
        let cancelled = self.owner_acted(id);
        // Recorded first: a restart waiting on this server checks it the moment the shim exits.
        self.killed.lock().expect("not poisoned").insert(id.into());
        match self.tell(id, Message::Kill).await {
            Ok(()) => Ok(()),
            Err(error) => {
                self.killed.lock().expect("not poisoned").remove(id);
                match error {
                    ServerError::NotRunning if cancelled => Ok(()),
                    error => Err(error),
                }
            }
        }
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

    /// Turn autosave off for a backup for at most `seconds`, once the shim confirms it; the shim
    /// turns it back on then if [`Servers::release_saves`] never comes. Returns how long the
    /// shim holds it, timed from before it was asked.
    pub async fn hold_saves(
        &self,
        id: &str,
        seconds: u64,
    ) -> Result<Duration, ServerError> {
        match self.ask(id, Message::HoldSaves { seconds }).await? {
            Message::SavesHeld { seconds } => Ok(Duration::from_secs(seconds)),
            other => Err(unexpected(other)),
        }
    }

    /// Turn autosave back on after [`Servers::hold_saves`], once the shim confirms it.
    pub async fn release_saves(&self, id: &str) -> Result<(), ServerError> {
        match self.ask(id, Message::ReleaseSaves).await? {
            Message::SavesReleased => Ok(()),
            other => Err(unexpected(other)),
        }
    }

    /// Send a running server's shim a message and wait for its answer.
    async fn ask(
        &self,
        id: &str,
        message: Message,
    ) -> Result<Message, ServerError> {
        if self.phase(id) != Some(Phase::Running) {
            return Err(ServerError::NotRunning);
        }
        let mut connection = Connection::open(&self.paths, id).await?;
        connection.send(message).await?;
        Ok(connection.recv(Duration::from_secs(10)).await?)
    }

    /// Wait, at most `timeout`, until no server is starting, stopping, or waiting to start
    /// again after a crash. The updater waits so a restart of Core never lands in the middle of
    /// one.
    pub async fn settle(&self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline && self.unsettled() {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    fn unsettled(&self) -> bool {
        !self.retries().is_empty()
            || self.live().values().any(|live| {
                matches!(live.phase, Phase::Starting | Phase::Stopping)
            })
    }

    /// Stop the server, wait for its shim to exit, and start it again.
    /// A kill while it waits cancels the start.
    pub async fn restart(&self, id: &str) -> Result<(), ServerError> {
        self.killed.lock().expect("not poisoned").remove(id);
        self.stop(id).await?;
        self.start_once_stopped(id).await
    }

    /// [`restart`](Self::restart) for run `run` only, from `watch`'s numbering. Core's own
    /// restarts are planned for one run: the run is claimed, as stopping, under the lock a new
    /// start needs, so a restart planned for one run never stops the next.
    pub async fn restart_run(
        &self,
        id: &str,
        run: u64,
    ) -> Result<(), ServerError> {
        if self.is_copying(id) {
            return Err(ServerError::BackingUp);
        }
        {
            let mut live = self.live();
            let Some(this) =
                live.get_mut(id).filter(|this| this.phase == Phase::Running)
            else {
                return Ok(());
            };
            if self.run_of(id) != Some(run) {
                return Ok(());
            }
            this.phase = Phase::Stopping;
        }
        self.killed.lock().expect("not poisoned").remove(id);
        if let Err(error) = self.tell(id, Message::Stop).await {
            self.set_phase(id, Phase::Running);
            return Err(error);
        }
        self.start_once_stopped(id).await
    }

    /// Wait for a stopping server's shim to exit, then start it, unless it was killed.
    async fn start_once_stopped(&self, id: &str) -> Result<(), ServerError> {
        while self.phase(id).is_some() {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        if self.killed.lock().expect("not poisoned").remove(id) {
            return Ok(());
        }
        self.start(id).await
    }

    /// Create an unconfigured server: its engine instance, then its row.
    pub async fn create(
        &self,
        name: &str,
    ) -> Result<server_rows::ServerRow, ServerError> {
        let id = uuid::Uuid::new_v4().to_string();
        // The engine makes the folder; Core then gives it the server account's rights.
        platform::create(&self.engine, &id, name)
            .await
            .map_err(|error| {
                ServerError::Other(color_eyre::eyre::eyre!("{error:#}"))
            })?;
        let created = async {
            crate::daemon::folders::create_server_folder(
                &self.paths.server(&id),
            )?;
            server_rows::create(&self.db, &id, name, |port| {
                server_rows::port_free(port, self.local_only)
            })
            .await?
            .ok_or(ServerError::NoPorts)
        }
        .await;
        if created.is_err()
            && let Err(error) = self.delete_folders(&id).await
        {
            warn!(server = %id, %error, "could not remove a server that was not created");
        }
        created
    }

    /// Hold a stopped server for deletion, so it cannot start while its row and files go. Fails
    /// while it runs. `release` undoes it when the deletion does not happen.
    pub fn hold_for_deletion(&self, id: &str) -> Result<(), ServerError> {
        let live = self.live();
        self.owner_acted_locked(id);
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

    /// Remove a held server's engine instance, world, run files, and backups, once its row is
    /// gone.
    pub async fn delete_folders(&self, id: &str) -> Result<(), ServerError> {
        engine::instance::remove(&self.engine, id)
            .await
            .map_err(|error| {
                ServerError::Other(color_eyre::eyre::eyre!("{error:#}"))
            })?;
        for folder in [
            self.paths.server(id),
            self.paths.run(id),
            self.paths.backups().join(id),
        ] {
            if_present(std::fs::remove_dir_all(&folder))?;
        }
        Ok(())
    }

    /// Hold a stopped server while its content changes: it cannot start, and no second hold is
    /// taken, until the hold drops.
    pub fn hold(self: &Arc<Self>, id: &str) -> Result<Hold, ServerError> {
        let mut live = self.live();
        // An install is the owner's action: a crash restart waiting to start is cancelled.
        self.owner_acted_locked(id);
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
}

#[cfg(test)]
impl Servers {
    /// A Core's servers in `paths`, on loopback, with an engine of their own.
    pub async fn open_for_test(paths: Paths, db: SqlitePool) -> Arc<Self> {
        let engine = crate::daemon::start_engine(&paths, None).await.unwrap();
        Self::open(
            paths,
            db,
            engine,
            Some(std::net::Ipv4Addr::LOCALHOST.into()),
        )
        .await
        .unwrap()
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
        let servers = Servers::open_for_test(paths, db::memory().await).await;
        (dir, servers)
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_link_planted_in_a_server_folder_is_not_followed() {
        use std::os::unix::fs::PermissionsExt;

        let (dir, servers) = servers().await;
        let server = servers.create("A").await.unwrap();
        platform::set(
            &servers.engine,
            &server.id,
            &platform::Platform {
                loader: engine::state::ModLoader::Vanilla,
                game_version: "1.21.1".into(),
                loader_version: None,
            },
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
    async fn what_a_server_left_without_its_row_goes_at_start() {
        let (_dir, servers) = servers().await;
        let kept = servers.create("Kept").await.unwrap();
        let gone = servers.create("Gone").await.unwrap();
        sqlx::query("DELETE FROM servers WHERE id = ?")
            .bind(&gone.id)
            .execute(&servers.db)
            .await
            .unwrap();
        // An engine removal that deleted the instance but not all of its folder.
        let stray = servers.paths.server("stray");
        std::fs::create_dir_all(stray.join("world")).unwrap();

        let servers = Servers::open(
            servers.paths.clone(),
            servers.db.clone(),
            Arc::clone(&servers.engine),
            None,
        )
        .await
        .unwrap();
        let instance = |id: String| {
            let engine = Arc::clone(&servers.engine);
            async move { engine::instance::get(&engine, &id).await.unwrap() }
        };
        assert!(instance(gone.id.clone()).await.is_none());
        assert!(!servers.paths.server(&gone.id).exists());
        assert!(!stray.exists());
        assert!(instance(kept.id.clone()).await.is_some());
        assert!(servers.paths.server(&kept.id).exists());
    }

    #[tokio::test]
    async fn a_shim_started_by_a_core_that_died_is_adopted_when_it_appears() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        crate::daemon::folders::create(&paths, None).unwrap();
        let db = db::memory().await;
        let server = server_rows::create(&db, "a", "A", |_| true)
            .await
            .unwrap()
            .unwrap();
        // The dead Core recorded the run and wrote the launch; its shim has not started yet.
        let now = chrono::Utc::now().timestamp();
        server_rows::start_run(&db, &server.id, "late", now)
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

        let servers = Servers::open_for_test(paths.clone(), db.clone()).await;
        assert_eq!(servers.phase(&server.id), None);
        assert_eq!(
            server_rows::open_runs(&db, &server.id, i64::MAX)
                .await
                .unwrap(),
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
            server_rows::open_runs(&db, &server.id, i64::MAX)
                .await
                .unwrap(),
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
    async fn a_server_a_hot_backup_copies_cannot_be_stopped_or_restarted() {
        let (_dir, servers) = servers().await;
        let server = servers.create("A").await.unwrap();
        let copying = servers.copying(&server.id);
        assert!(matches!(
            servers.stop(&server.id).await,
            Err(ServerError::BackingUp)
        ));
        assert!(matches!(
            servers.restart(&server.id).await,
            Err(ServerError::BackingUp)
        ));
        // Core's own planned restarts too.
        assert!(matches!(
            servers.restart_run(&server.id, 1).await,
            Err(ServerError::BackingUp)
        ));
        drop(copying);
        assert!(matches!(
            servers.stop(&server.id).await,
            Err(ServerError::NotRunning)
        ));
    }

    #[tokio::test]
    async fn a_shim_of_an_older_protocol_is_left_running() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let db = db::memory().await;
        let server = server_rows::create(&db, "a", "A", |_| true)
            .await
            .unwrap()
            .unwrap();
        let _shim = shim::try_lock(&paths, &server.id).unwrap().unwrap();
        std::fs::write(
            paths.run_record(&server.id),
            r#"{"v":1,"run_id":"older","pid":1}"#,
        )
        .unwrap();

        let servers = Servers::open_for_test(paths, db).await;
        assert_eq!(servers.phase(&server.id), Some(Phase::Unmanaged(Some(1))));
        assert!(matches!(
            servers.stop(&server.id).await,
            Err(ServerError::Shim(ShimError::Incompatible(1)))
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
        let server = server_rows::create(&db, "a", "A", |_| true)
            .await
            .unwrap()
            .unwrap();
        server_rows::start_run(&db, &server.id, "killed", 1)
            .await
            .unwrap();
        server_rows::start_run(&db, &server.id, "vanished", 2)
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

        let servers = Servers::open_for_test(paths, db.clone()).await;
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
