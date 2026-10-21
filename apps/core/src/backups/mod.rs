//! The one queued backup engine. A backup is a zip of the server's worlds and a short list of
//! settings files, in `backups/<server>/<backup>.zip`, which only `copal` can reach. Creates and
//! restores queue in `backup_operations` and run one at a time, so a schedule, an update, and a
//! person never copy at once.
//!
//! A running server is backed up hot: Core asks its shim to hold autosave off (the shim turns it
//! back on itself after ten minutes, should Core never come back), runs `save-all flush`, waits
//! for the save, lists and copies the files, and releases. A copy that would outlast the hold
//! fails instead. A stopped server is held stopped for the copy. Mods that write outside the save
//! cycle can still make a hot backup slightly inconsistent; that is accepted, but a file that
//! grows past the size listed fails the backup rather than filling the disk.
//!
//! Every backup holds `copal-backup.json`, naming the world folders and settings files it speaks
//! for. A restore reads and checks the whole backup first, moves exactly those roots aside into
//! a folder of its own in `backups/<server>/`, which servers cannot reach, unpacks the backup in
//! their place, and puts the old ones back if that fails. Its journal lets a Core that stopped
//! midway put them back when it starts, before the server can. A backup is checked by the rules
//! a restore applies before it counts as made.
//!
//! Each server has a schedule (default: daily, keeping 7). Pre-update backups keep their own last
//! 2, so updates never evict the scheduled ones; manual backups are kept until deleted.

pub mod records;
mod space;

use std::{
    collections::HashMap,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::sync::{Notify, broadcast};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    access::{Caller, now},
    api::events::{BackupState, BackupTask, WsEvent},
    content::{
        archive,
        folder::{self, Folder},
    },
    daemon::paths::Paths,
    servers::{Phase, ServerError, Servers, console},
};
use records::{Kind, OpKind, OpState, Operation};
pub use space::enough_space;

/// Settings files a backup keeps beside the worlds.
const SETTINGS: [&str; 5] = [
    "server.properties",
    "ops.json",
    "whitelist.json",
    "banned-players.json",
    "banned-ips.json",
];
/// How long autosave may stay off for one hot backup.
const SAVE_HOLD_S: u64 = crate::servers::shim::MAX_SAVE_HOLD_S;
/// How long Core waits for `save-all flush` to finish before copying anyway.
const SAVE_WAIT: Duration = Duration::from_secs(120);
/// A hot copy fails this long before the shim's hold runs out, so it never copies while saving
/// is on again.
const HOLD_MARGIN: Duration = Duration::from_secs(30);
/// What a backup holds, inside it.
const MANIFEST: &str = "copal-backup.json";
/// A restore's journal, in its folder.
const JOURNAL: &str = "swap.json";
/// Pre-update backups kept per server.
const KEEP_PRE_UPDATE: u32 = 2;
/// How often the schedule is checked.
const SCHEDULE_TICK: Duration = Duration::from_secs(60);
/// How long a download link works, once.
const DOWNLOAD_GRANT: Duration = Duration::from_secs(300);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "there is not enough free disk space for this backup: it needs about {needed_mb} MB and {free_mb} MB are free"
    )]
    Space { needed_mb: u64, free_mb: u64 },
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Server(#[from] ServerError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// What a download link may fetch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// The server's worlds as they are now.
    World,
    Backup(String),
}

/// A download link someone with access asked for: single use, for a few minutes.
#[derive(Debug, Clone)]
pub struct Grant {
    pub caller: Caller,
    pub server_id: String,
    pub target: Target,
    expires: Instant,
}

/// The backup engine. One per Core.
pub struct Backups {
    paths: Paths,
    db: SqlitePool,
    servers: Arc<Servers>,
    /// Progress for console sockets, by server. Content installs share it.
    pub events: broadcast::Sender<(String, WsEvent)>,
    wake: Notify,
    /// The ids of operations as they finish.
    finished: broadcast::Sender<i64>,
    /// Held while copying a server, for a backup or a download of its world.
    copying: tokio::sync::Mutex<()>,
    grants: Mutex<HashMap<String, Grant>>,
    /// Servers a restore could not put back, held so they never start on a half-restored world
    /// until someone looks (the old files are in the restore's folder).
    stuck: Mutex<Vec<crate::servers::Hold>>,
}

impl Backups {
    /// Fail what a stopped Core left running, then start the queue and the schedule.
    pub async fn open(
        paths: Paths,
        db: SqlitePool,
        servers: Arc<Servers>,
    ) -> Result<Arc<Self>, Error> {
        let backups = Arc::new(Self {
            paths,
            db,
            servers,
            events: broadcast::channel(256).0,
            wake: Notify::new(),
            finished: broadcast::channel(64).0,
            copying: tokio::sync::Mutex::new(()),
            grants: Mutex::default(),
            stuck: Mutex::default(),
        });
        backups.recover_restores().await;
        for operation in records::fail_interrupted(&backups.db).await? {
            if operation.kind == OpKind::Create {
                let _ = std::fs::remove_file(
                    backups.file(&operation.server_id, &operation.backup_id),
                );
            }
        }
        backups.remove_leftovers();
        tokio::spawn(Arc::clone(&backups).work());
        tokio::spawn(Arc::clone(&backups).schedule());
        Ok(backups)
    }

    /// Where a backup's zip is.
    pub fn file(&self, server_id: &str, backup_id: &str) -> PathBuf {
        self.paths
            .backups()
            .join(server_id)
            .join(format!("{backup_id}.zip"))
    }

    /// Where a restore keeps what it moved aside.
    fn restore_folder(&self, server_id: &str) -> PathBuf {
        self.paths
            .backups()
            .join(server_id)
            .join(format!("restore-{}", Uuid::new_v4().simple()))
    }

    /// Undo every restore a stopped Core left midway, holding its server meanwhile. A server
    /// whose restore cannot be undone stays held.
    async fn recover_restores(&self) {
        let Ok(servers) = std::fs::read_dir(self.paths.backups()) else {
            return;
        };
        for server in servers.filter_map(Result::ok) {
            let Ok(server_id) = server.file_name().into_string() else {
                continue;
            };
            let Ok(entries) = std::fs::read_dir(server.path()) else {
                continue;
            };
            for entry in entries.filter_map(Result::ok) {
                let stage = entry.path();
                let is_restore = entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with("restore-"));
                if !is_restore || !stage.is_dir() {
                    continue;
                }
                let hold = match self.servers.hold(&server_id) {
                    Ok(hold) => hold,
                    Err(error) => {
                        warn!(server = %server_id, %error, "could not hold a server to undo its restore");
                        continue;
                    }
                };
                let root = self.paths.server(&server_id);
                let undone = tokio::task::spawn_blocking(move || {
                    let folder = Folder::open(&root)?;
                    undo(&folder, &stage)
                })
                .await
                .map_err(io::Error::other)
                .map_err(Error::from)
                .and_then(|result| result);
                match undone {
                    Ok(()) => {
                        info!(server = %server_id, "undid a restore Core stopped during")
                    }
                    Err(error) => {
                        warn!(server = %server_id, %error, "could not undo a restore; the server stays stopped and its old files are in backups");
                        self.stuck.lock().expect("not poisoned").push(hold);
                    }
                }
            }
        }
    }

    /// Half-written zips and world downloads a stopped Core left behind.
    fn remove_leftovers(&self) {
        let Ok(servers) = std::fs::read_dir(self.paths.backups()) else {
            return;
        };
        for server in servers.filter_map(Result::ok) {
            let Ok(files) = std::fs::read_dir(server.path()) else {
                continue;
            };
            for file in files.filter_map(Result::ok) {
                let name = file.file_name().to_string_lossy().into_owned();
                if name.ends_with(".part") || name.starts_with("download-") {
                    let _ = std::fs::remove_file(file.path());
                }
            }
        }
    }

    /// Queue a backup of a server. Returns the backup's id and its create operation's.
    pub async fn create(
        &self,
        server_id: &str,
        name: &str,
        kind: Kind,
        user_id: Option<&str>,
    ) -> Result<(String, i64), Error> {
        let backup = records::Backup {
            id: Uuid::new_v4().to_string(),
            server_id: server_id.to_owned(),
            name: name.to_owned(),
            kind,
            state: records::State::Queued,
            created_at: now(),
        };
        let operation =
            records::queue_backup(&self.db, &backup, user_id).await?;
        self.wake.notify_one();
        Ok((backup.id, operation))
    }

    /// Queue a restore of a backup over the server's worlds. The server must be stopped.
    pub async fn restore(
        &self,
        server_id: &str,
        backup_id: &str,
        user_id: Option<&str>,
    ) -> Result<i64, Error> {
        let backup = records::backup(&self.db, server_id, backup_id)
            .await?
            .filter(|backup| backup.state == records::State::Done)
            .ok_or_else(|| {
                Error::NotFound(format!(
                    "There is no finished backup {backup_id}."
                ))
            })?;
        if self.servers.phase(server_id).is_some() {
            return Err(Error::Conflict(
                "Stop the server before restoring a backup.".into(),
            ));
        }
        let operation = records::queue_operation(
            &self.db,
            &backup.id,
            server_id,
            OpKind::Restore,
            user_id,
        )
        .await?;
        self.wake.notify_one();
        Ok(operation)
    }

    /// Queue a failed backup again.
    pub async fn retry(
        &self,
        server_id: &str,
        backup_id: &str,
        user_id: Option<&str>,
    ) -> Result<i64, Error> {
        let backup = records::backup(&self.db, server_id, backup_id)
            .await?
            .filter(|backup| backup.state == records::State::Failed)
            .ok_or_else(|| {
                Error::NotFound(format!(
                    "There is no failed backup {backup_id}."
                ))
            })?;
        let operation = records::queue_operation(
            &self.db,
            &backup.id,
            server_id,
            OpKind::Create,
            user_id,
        )
        .await?;
        self.wake.notify_one();
        Ok(operation)
    }

    /// Wait until an operation has finished; its error when it failed.
    pub async fn wait(&self, operation_id: i64) -> Result<(), String> {
        let mut finished = self.finished.subscribe();
        loop {
            match records::operation(&self.db, operation_id).await {
                Ok(Some(operation)) => match operation.state {
                    OpState::Done => return Ok(()),
                    OpState::Failed | OpState::Cancelled => {
                        return Err(operation
                            .error
                            .unwrap_or_else(|| "cancelled".into()));
                    }
                    OpState::Queued | OpState::Running => {}
                },
                Ok(None) => return Err("the backup was cancelled".into()),
                Err(error) => return Err(error.to_string()),
            }
            // A missed message only costs a second.
            let _ =
                tokio::time::timeout(Duration::from_secs(1), finished.recv())
                    .await;
        }
    }

    /// Back a server up before an update and wait for it.
    pub async fn pre_update(&self, server_id: &str) -> Result<(), Error> {
        let (_, operation) = self
            .create(server_id, "Before update", Kind::PreUpdate, None)
            .await?;
        self.wait(operation).await.map_err(|error| {
            Error::Conflict(format!(
                "The backup before the update failed: {error}"
            ))
        })
    }

    /// Delete a backup and its zip. Not while it is being made or restored.
    pub async fn delete(
        &self,
        server_id: &str,
        backup_id: &str,
    ) -> Result<(), Error> {
        records::backup(&self.db, server_id, backup_id)
            .await?
            .ok_or_else(|| {
                Error::NotFound(format!("There is no backup {backup_id}."))
            })?;
        if records::busy(&self.db, backup_id).await? {
            return Err(Error::Conflict(
                "The backup is in use; try again when it is done.".into(),
            ));
        }
        remove_file(&self.file(server_id, backup_id))?;
        records::delete_backup(&self.db, backup_id).await?;
        Ok(())
    }

    /// Run queued operations, oldest first, one at a time.
    async fn work(self: Arc<Self>) {
        loop {
            match records::next_queued(&self.db).await {
                Ok(Some(operation)) => self.run(operation).await,
                Ok(None) => {
                    let _ = tokio::time::timeout(
                        Duration::from_secs(60),
                        self.wake.notified(),
                    )
                    .await;
                }
                Err(error) => {
                    warn!(%error, "could not read the backup queue");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    async fn run(&self, operation: Operation) {
        match records::start(&self.db, &operation).await {
            Ok(true) => {}
            // Cancelled since it was read.
            Ok(false) => return,
            Err(error) => {
                warn!(%error, "could not start a backup operation");
                tokio::time::sleep(Duration::from_secs(5)).await;
                return;
            }
        }
        let task = match operation.kind {
            OpKind::Create => BackupTask::Create,
            OpKind::Restore => BackupTask::Restore,
        };
        let started = chrono::Utc::now().timestamp_millis();
        send_progress(
            &self.events,
            &operation,
            task,
            BackupState::Ongoing,
            0.0,
            started,
            None,
        );
        let result = match operation.kind {
            OpKind::Create => self.run_create(&operation, task, started).await,
            OpKind::Restore => self.run_restore(&operation).await.map(|()| 0),
        };
        let error = result.as_ref().err().map(|error| format!("{error:#}"));
        match &error {
            None => {
                info!(server = %operation.server_id, backup = %operation.backup_id, kind = ?operation.kind, "backup operation done")
            }
            Some(error) => {
                warn!(server = %operation.server_id, backup = %operation.backup_id, kind = ?operation.kind, %error, "backup operation failed")
            }
        }
        if operation.kind == OpKind::Create && error.is_some() {
            let _ = remove_file(
                &self.file(&operation.server_id, &operation.backup_id),
            );
        }
        let recorded = match &result {
            Ok(size) => Ok(*size),
            Err(_) => Err(error.as_deref().unwrap_or_default()),
        };
        if let Err(error) =
            records::finish(&self.db, &operation, recorded).await
        {
            warn!(%error, "could not record the end of a backup operation");
        }
        if operation.kind == OpKind::Create && result.is_ok() {
            self.apply_retention(&operation.server_id, &operation.backup_id)
                .await;
        }
        let state = if result.is_ok() {
            BackupState::Done
        } else {
            BackupState::Failed
        };
        let ended = Some(chrono::Utc::now().timestamp_millis());
        send_progress(
            &self.events,
            &operation,
            task,
            state,
            1.0,
            started,
            ended,
        );
        let _ = self.finished.send(operation.id);
    }

    async fn run_create(
        &self,
        operation: &Operation,
        task: BackupTask,
        started: i64,
    ) -> Result<u64, Error> {
        let destination = self.file(&operation.server_id, &operation.backup_id);
        let events = self.events.clone();
        let reported = operation.clone();
        let last = Mutex::new(0u32);
        self.snapshot(&operation.server_id, &destination, move |share| {
            let percent = (share * 100.0) as u32;
            let mut last = last.lock().expect("not poisoned");
            if percent > *last {
                *last = percent;
                send_progress(
                    &events,
                    &reported,
                    task,
                    BackupState::Ongoing,
                    share,
                    started,
                    None,
                );
            }
        })
        .await
    }

    /// Delete what the new backup's kind keeps no more of.
    async fn apply_retention(&self, server_id: &str, backup_id: &str) {
        let Ok(Some(backup)) =
            records::backup(&self.db, server_id, backup_id).await
        else {
            return;
        };
        let keep = match backup.kind {
            Kind::Manual => return,
            Kind::PreUpdate => KEEP_PRE_UPDATE,
            Kind::Scheduled => {
                match records::schedule(&self.db, server_id).await {
                    Ok(schedule) => schedule.keep.max(1),
                    Err(_) => return,
                }
            }
        };
        let Ok(excess) =
            records::excess(&self.db, server_id, backup.kind, keep).await
        else {
            return;
        };
        for old in excess {
            if let Err(error) = self.delete(server_id, &old).await {
                warn!(server = %server_id, backup = %old, %error, "could not delete an old backup");
            }
        }
    }

    /// Zip the server's worlds and settings into `destination`, hot when the server runs and
    /// holding it stopped when it is not. Checks the free space first and leaves nothing behind
    /// when it fails. Returns the zip's size. `progress` is told the share copied.
    pub async fn snapshot(
        &self,
        server_id: &str,
        destination: &Path,
        progress: impl Fn(f64) + Send + 'static,
    ) -> Result<u64, Error> {
        let _copying = self.copying.lock().await;
        let folder = Arc::new(Folder::open(&self.paths.server(server_id))?);
        let (hot, _hold) = match self.servers.phase(server_id) {
            Some(Phase::Running) => (true, None),
            // An install holds it stopped; the update waiting for this backup changes nothing
            // until it is done.
            Some(Phase::Installing) => (false, None),
            None => {
                let hold =
                    self.servers.hold(server_id).map_err(
                        |error| match error {
                            ServerError::AlreadyRunning => Error::Conflict(
                                "The server started; back it up again.".into(),
                            ),
                            error => error.into(),
                        },
                    )?;
                (false, Some(hold))
            }
            Some(Phase::Unmanaged(_)) => {
                return Err(Error::Conflict(
                    "Another Copal version started this server, so Core cannot hold its saves. Restart it, then back it up."
                        .into(),
                ));
            }
            Some(Phase::Starting | Phase::Stopping) => {
                return Err(Error::Conflict(
                    "The server is starting or stopping; back it up once it runs or has stopped."
                        .into(),
                ));
            }
        };
        // Held from before the save until saving is on again.
        let _copying = hot.then(|| self.servers.copying(server_id));
        let save = async {
            if hot {
                self.hold_and_save(server_id).await.map(Some)
            } else {
                Ok(None)
            }
        };
        let result = copy(folder, destination, progress, save).await;
        if hot && let Err(error) = self.servers.release_saves(server_id).await {
            warn!(server = %server_id, %error, "could not turn autosave back on; the shim will");
        }
        result
    }

    /// Hold autosave off and wait for a full save to finish. Returns when the copy must be done.
    async fn hold_and_save(&self, server_id: &str) -> Result<Instant, Error> {
        let asked = Instant::now();
        let held = self.servers.hold_saves(server_id, SAVE_HOLD_S).await?;
        let deadline = asked + held.saturating_sub(HOLD_MARGIN);
        let log = self.paths.console_log(server_id);
        let (_, end) = console::tail(&log, 0)?;
        let (sender, mut lines) = tokio::sync::mpsc::channel(256);
        console::follow(log, end, sender);
        if let Err(error) =
            self.servers.command(server_id, "save-all flush").await
        {
            let _ = self.servers.release_saves(server_id).await;
            return Err(error.into());
        }
        let wait =
            SAVE_WAIT.min(deadline.saturating_duration_since(Instant::now()));
        // Copying while the flush still writes could keep a half-written region file. The
        // caller turns saving back on either way.
        if !saved(&mut lines, wait).await {
            return Err(Error::Conflict(format!(
                "The server did not finish saving within {} seconds, so no backup was made; try again.",
                wait.as_secs()
            )));
        }
        Ok(deadline)
    }

    /// Replace the server's worlds and settings with a backup's. Holds the server so it cannot
    /// start meanwhile.
    async fn run_restore(&self, operation: &Operation) -> Result<(), Error> {
        let server_id = &operation.server_id;
        let hold =
            self.servers.hold(server_id).map_err(|error| match error {
                ServerError::AlreadyRunning => Error::Conflict(
                    "Stop the server before restoring a backup.".into(),
                ),
                error => error.into(),
            })?;
        let _copying = self.copying.lock().await;
        let zip = self.file(server_id, &operation.backup_id);
        let root = self.paths.server(server_id);
        let folder = Arc::new(Folder::open(&root)?);
        let stage = self.restore_folder(server_id);
        let result = tokio::task::spawn_blocking({
            let stage = stage.clone();
            move || restore(&folder, &root, &stage, &zip)
        })
        .await
        .map_err(io::Error::other)?;
        // Its folder outlives a restore only when what it moved aside could not be put back.
        if result.is_err() && stage.exists() {
            warn!(server = %server_id, stage = %stage.display(), "a failed restore could not put the old files back; the server stays stopped");
            self.stuck.lock().expect("not poisoned").push(hold);
        }
        result
    }

    /// Back up servers whose schedule is due, every minute.
    async fn schedule(self: Arc<Self>) {
        let mut tick = tokio::time::interval(SCHEDULE_TICK);
        loop {
            tick.tick().await;
            if let Err(error) = self.queue_due().await {
                warn!(%error, "could not check backup schedules");
            }
        }
    }

    /// Queue a scheduled backup of each server that is due one: its interval has passed since the
    /// last, and it ran since then, so its world may have changed.
    pub async fn queue_due(&self) -> Result<(), Error> {
        let servers: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM servers WHERE game_version IS NOT NULL",
        )
        .fetch_all(&self.db)
        .await?;
        let now = now();
        for server_id in servers {
            let schedule = records::schedule(&self.db, &server_id).await?;
            if schedule.interval_hours == 0
                || records::server_busy(&self.db, &server_id).await?
            {
                continue;
            }
            let last = records::last_scheduled(&self.db, &server_id).await?;
            if last.is_some_and(|last| {
                now - last < i64::from(schedule.interval_hours) * 3600
            }) {
                continue;
            }
            let phase = self.servers.phase(&server_id);
            if phase.is_some_and(|phase| phase != Phase::Running) {
                continue;
            }
            let running = phase == Some(Phase::Running);
            let ran_since = records::last_run_end(&self.db, &server_id)
                .await?
                .is_some_and(|ended| last.is_none_or(|last| ended > last));
            if running || ran_since {
                self.create(
                    &server_id,
                    "Scheduled backup",
                    Kind::Scheduled,
                    None,
                )
                .await?;
            }
        }
        Ok(())
    }

    /// A single-use link for `target`, valid for five minutes.
    pub fn grant(
        &self,
        caller: Caller,
        server_id: &str,
        target: Target,
    ) -> String {
        let mut bytes = [0u8; 32];
        ring::rand::SecureRandom::fill(
            &ring::rand::SystemRandom::new(),
            &mut bytes,
        )
        .expect("the system has randomness");
        let token = hex::encode(bytes);
        let mut grants = self.grants.lock().expect("not poisoned");
        grants.retain(|_, grant| grant.expires > Instant::now());
        grants.insert(
            token.clone(),
            Grant {
                caller,
                server_id: server_id.to_owned(),
                target,
                expires: Instant::now() + DOWNLOAD_GRANT,
            },
        );
        token
    }

    /// Use up a download link.
    pub fn take(&self, token: &str) -> Option<Grant> {
        self.grants
            .lock()
            .expect("not poisoned")
            .remove(token)
            .filter(|grant| grant.expires > Instant::now())
    }
}

/// Whether the console reports a finished save within `wait`.
async fn saved(
    lines: &mut tokio::sync::mpsc::Receiver<String>,
    wait: Duration,
) -> bool {
    let reported = async {
        while let Some(line) = lines.recv().await {
            if line.contains("Saved the game")
                || line.contains("Saved the world")
            {
                return true;
            }
        }
        false
    };
    matches!(tokio::time::timeout(wait, reported).await, Ok(true))
}

/// Tell console sockets how far an operation is.
fn send_progress(
    events: &broadcast::Sender<(String, WsEvent)>,
    operation: &Operation,
    task: BackupTask,
    state: BackupState,
    progress: f64,
    started: i64,
    finished: Option<i64>,
) {
    let _ = events.send((
        operation.server_id.clone(),
        WsEvent::BackupProgress {
            id: Uuid::parse_str(&operation.backup_id).unwrap_or_default(),
            task,
            state,
            progress,
            start_time: Some(started),
            finish_time: finished,
        },
    ));
}

/// Remove a file; one that is not there is fine.
fn remove_file(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// The world folders of a server folder, by name.
fn worlds(folder: &Folder) -> [String; 3] {
    let level = folder.level_name();
    [
        level.clone(),
        format!("{level}_nether"),
        format!("{level}_the_end"),
    ]
}

/// What a backup speaks for: a restore replaces exactly these. World folders are listed whether
/// or not they existed, so a world made since is not left beside the restored one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Manifest {
    v: u32,
    worlds: Vec<String>,
    settings: Vec<String>,
}

impl Manifest {
    /// The roots, checked: world folders named after one level, settings from [`SETTINGS`].
    fn check(&self) -> Result<(), Error> {
        let refuse =
            |reason: &str| Error::Conflict(format!("This backup {reason}."));
        if self.v != 1 {
            return Err(refuse("was made by another Copal version"));
        }
        let Some(level) = self.worlds.first() else {
            return Err(refuse("names no world"));
        };
        let worlds_ok = self.worlds.len() <= 3
            && self.worlds.iter().enumerate().all(|(n, world)| {
                folder::clean(world).is_ok_and(|clean| clean == *world)
                    && [
                        level.clone(),
                        format!("{level}_nether"),
                        format!("{level}_the_end"),
                    ][n] == *world
            });
        if !worlds_ok || SETTINGS.contains(&level.as_str()) {
            return Err(refuse("names worlds Core would not have"));
        }
        if !self
            .settings
            .iter()
            .all(|name| SETTINGS.contains(&name.as_str()))
        {
            return Err(refuse("names settings Core would not keep"));
        }
        // Each root once, none inside another, compared without case as Windows and macOS do:
        // a root swapped twice would overwrite its own rollback copy.
        let roots: Vec<String> =
            self.roots().map(|root| folder::fold_case(root)).collect();
        let overlap = roots.iter().enumerate().any(|(n, a)| {
            roots.iter().skip(n + 1).any(|b| {
                a == b
                    || a.strip_prefix(b.as_str())
                        .is_some_and(|r| r.starts_with('/'))
                    || b.strip_prefix(a.as_str())
                        .is_some_and(|r| r.starts_with('/'))
            })
        });
        if overlap {
            return Err(refuse("names a file or folder twice"));
        }
        Ok(())
    }

    /// Whether `path` from the backup is one of its roots or inside one.
    fn covers(&self, path: &str) -> bool {
        self.settings.iter().any(|name| name == path)
            || self.worlds.iter().any(|world| {
                path == world
                    || path
                        .strip_prefix(world.as_str())
                        .is_some_and(|rest| rest.starts_with('/'))
            })
    }

    fn roots(&self) -> impl Iterator<Item = &String> {
        self.worlds.iter().chain(&self.settings)
    }
}

/// What one backup copies: its manifest and every file with its size, measured after the save.
#[derive(Debug)]
struct Listing {
    manifest: Manifest,
    files: Vec<(String, u64)>,
}

/// List what a backup of `folder` holds: the worlds, then the settings. Links are skipped. A
/// name or count a restore would refuse fails the listing, so no backup is made that cannot be
/// restored.
fn files(folder: &Folder) -> Result<Listing, Error> {
    let worlds = worlds(folder);
    let mut files = Vec::new();
    for world in &worlds {
        restorable(world)?;
        match folder.open_dir(Some(world)) {
            Ok(dir) => walk(&dir, world, &mut files)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    let mut settings = Vec::new();
    for name in SETTINGS {
        if let Ok(meta) = folder.metadata(name)
            && meta.is_file()
        {
            files.push((name.to_owned(), meta.len()));
            settings.push(name.to_owned());
        }
    }
    check_count(files.len())?;
    let manifest = Manifest {
        v: 1,
        worlds: worlds.into(),
        settings,
    };
    manifest.check()?;
    Ok(Listing { manifest, files })
}

/// Refuse a path a restore would refuse.
fn restorable(path: &str) -> Result<(), Error> {
    match folder::clean(path) {
        Ok(clean) if clean == path => Ok(()),
        _ => Err(Error::Conflict(format!(
            "{path} has a name a backup cannot hold; rename it and back up again."
        ))),
    }
}

/// Refuse more files than a restore unpacks (the manifest is one more).
fn check_count(files: usize) -> Result<(), Error> {
    if files + 1 > archive::MAX_ENTRIES {
        return Err(Error::Conflict(format!(
            "The worlds hold more than {} files, more than a backup can restore.",
            archive::MAX_ENTRIES - 1
        )));
    }
    Ok(())
}

fn walk(
    dir: &cap_std::fs::Dir,
    prefix: &str,
    files: &mut Vec<(String, u64)>,
) -> Result<(), Error> {
    for entry in dir.entries()? {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else {
            return Err(Error::Conflict(format!(
                "A file in {prefix} has a name a backup cannot hold; rename it and back up again."
            )));
        };
        // Held locked by a running server, and meaningless in a backup.
        if name == "session.lock" {
            continue;
        }
        let path = format!("{prefix}/{name}");
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        restorable(&path)?;
        if kind.is_dir() {
            walk(&folder::open_child(dir, &name, &path)?, &path, files)?;
        } else if kind.is_file() {
            files.push((path, entry.metadata()?.len()));
        }
        check_count(files.len())?;
    }
    Ok(())
}

/// Flush the server with `save`, which gives the time a hot copy must end by, then list its files,
/// check the space, and zip them into `destination`. Leaves nothing behind when it fails.
async fn copy(
    folder: Arc<Folder>,
    destination: &Path,
    progress: impl Fn(f64) + Send + 'static,
    save: impl Future<Output = Result<Option<Instant>, Error>>,
) -> Result<u64, Error> {
    let deadline = save.await?;
    let listing = {
        let folder = Arc::clone(&folder);
        tokio::task::spawn_blocking(move || files(&folder))
            .await
            .map_err(io::Error::other)??
    };
    let total: u64 = listing.files.iter().map(|(_, size)| size).sum();
    let parent = destination.parent().expect("backups are in a folder");
    std::fs::create_dir_all(parent)?;
    enough_space(total, space::free(parent)?)?;
    let staged = destination.with_extension("zip.part");
    let result = {
        let staged = staged.clone();
        tokio::task::spawn_blocking(move || {
            write_zip(&folder, &listing, &staged, deadline, &progress)
        })
        .await
        .map_err(io::Error::other)
        .map_err(Error::from)
        .and_then(|result| result)
    };
    match result.and_then(|()| Ok(std::fs::rename(&staged, destination)?)) {
        Ok(()) => Ok(std::fs::metadata(destination)?.len()),
        Err(error) => {
            let _ = std::fs::remove_file(&staged);
            Err(error)
        }
    }
}

/// Write the listed files from `folder` into a zip at `path`, the manifest first. Region files
/// are stored, being compressed already; the rest is deflated. A file gone by the time it is
/// read is left out; one larger than listed fails the copy, as does passing `deadline`.
fn write_zip(
    folder: &Folder,
    listing: &Listing,
    path: &Path,
    deadline: Option<Instant>,
    progress: &dyn Fn(f64),
) -> Result<(), Error> {
    use zip::{CompressionMethod, write::SimpleFileOptions};
    let late = || {
        deadline.is_some_and(|deadline| Instant::now() >= deadline).then(|| {
            Error::Conflict(format!(
                "The copy would outlast the {} minutes autosave may stay off; stop the server and back it up then.",
                SAVE_HOLD_S / 60
            ))
        })
    };
    let total: u64 = listing.files.iter().map(|(_, size)| size).sum();
    let mut zip =
        zip::ZipWriter::new(io::BufWriter::new(std::fs::File::create(path)?));
    zip.start_file(MANIFEST, SimpleFileOptions::default())?;
    zip.write_all(
        &serde_json::to_vec(&listing.manifest).map_err(io::Error::other)?,
    )?;
    let mut done = 0u64;
    let mut buffer = vec![0; 256 * 1024];
    for (name, size) in &listing.files {
        let Some(file) = folder.open_file(name)? else {
            continue;
        };
        let compressed = name.ends_with(".mca") || name.ends_with(".mcc");
        let options = SimpleFileOptions::default()
            .compression_method(if compressed {
                CompressionMethod::Stored
            } else {
                CompressionMethod::Deflated
            })
            .large_file(*size >= u64::from(u32::MAX));
        zip.start_file(name.as_str(), options)?;
        let mut file = file.take(size + 1);
        let mut read_total = 0u64;
        loop {
            if let Some(error) = late() {
                return Err(error);
            }
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            read_total += read as u64;
            if read_total > *size {
                return Err(Error::Conflict(format!(
                    "{name} grew while it was copied; back up again."
                )));
            }
            zip.write_all(&buffer[..read])?;
            done += read as u64;
            progress((done as f64 / total.max(1) as f64).min(1.0));
        }
    }
    zip.finish()?.flush()?;
    Ok(())
}

/// How far a restore got, in its journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Step {
    /// Moving the roots aside: those in `old/` were moved, the rest are as they were.
    Aside,
    /// Unpacking: every root is the backup's, or partly so, and the old ones are in `old/`.
    Unpack,
    Done,
}

/// A restore's journal: the roots it replaces and how far it got. Written before each step.
#[derive(Debug, Serialize, Deserialize)]
struct Journal {
    v: u32,
    roots: Vec<String>,
    step: Step,
}

/// Replace the worlds and settings in `folder` (at `root`) with those in the backup at `zip`,
/// keeping what it replaces in `stage` until it is done.
///
/// Every entry is read through first, which checks it against the manifest and its checksum
/// against the archive, before anything moves. Then each root the manifest names is moved into
/// `stage/old/`, a folder only Core can reach on the same disk, and the backup is unpacked in
/// their place. If that fails the old roots are moved back; if even that fails, `stage` is kept
/// with its journal, and [`undo`] finishes the job.
fn restore(
    folder: &Folder,
    root: &Path,
    stage: &Path,
    zip: &Path,
) -> Result<(), Error> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(zip)?)?;
    let entries = archive::entries(&mut archive).map_err(|error| {
        io::Error::new(io::ErrorKind::InvalidData, error.to_string())
    })?;
    let manifest: Manifest = {
        let mut text = String::new();
        archive
            .by_name(MANIFEST)
            .map_err(|_| Error::Conflict("This is not a Core backup.".into()))?
            .take(64 * 1024)
            .read_to_string(&mut text)?;
        serde_json::from_str(&text).map_err(|_| {
            Error::Conflict("This backup's manifest is unreadable.".into())
        })?
    };
    manifest.check()?;
    if let Some((path, _)) = entries
        .iter()
        .find(|(path, _)| path != MANIFEST && !manifest.covers(path))
    {
        return Err(Error::Conflict(format!(
            "This backup holds {path}, which it does not name."
        )));
    }
    let mut size = 0u64;
    for index in 0..archive.len() {
        // Read to the end, so a damaged entry fails its checksum now.
        size += io::copy(&mut archive.by_index(index)?, &mut io::sink())?;
    }
    enough_space(size, space::free(root)?)?;

    std::fs::create_dir_all(stage)?;
    let aside = Folder::open(stage)?;
    let roots: Vec<String> = manifest.roots().cloned().collect();
    let journal = |step| -> io::Result<()> {
        crate::servers::shim::write_json(
            &stage.join(JOURNAL),
            &Journal {
                v: 1,
                roots: roots.clone(),
                step,
            },
        )
    };
    let result = (|| -> Result<(), Error> {
        journal(Step::Aside)?;
        for path in &roots {
            if folder.exists(path) {
                folder.move_to(path, &aside, &format!("old/{path}"))?;
            }
        }
        journal(Step::Unpack)?;
        for (index, (path, is_dir)) in entries.iter().enumerate() {
            if path == MANIFEST {
                continue;
            }
            if *is_dir {
                folder.create_dir_all(path)?;
            } else {
                folder.write(path, &mut archive.by_index(index)?)?;
            }
        }
        journal(Step::Done)?;
        Ok(())
    })();
    // Windows cannot remove a folder that is open.
    drop(aside);
    match result {
        Ok(()) => {
            if let Err(error) = std::fs::remove_dir_all(stage) {
                warn!(stage = %stage.display(), %error, "could not remove a finished restore's folder");
            }
            Ok(())
        }
        Err(error) => {
            if let Err(undo_error) = undo(folder, stage) {
                warn!(stage = %stage.display(), error = %undo_error, "could not put back what a failed restore moved aside");
            }
            Err(error)
        }
    }
}

/// Put back what the restore in `stage` moved aside, by its journal, and remove `stage`. Run on
/// a restore that failed, and on start for one a stopped Core left midway. `stage` is kept when
/// this fails.
fn undo(folder: &Folder, stage: &Path) -> Result<(), Error> {
    let journal: Journal = match std::fs::read(stage.join(JOURNAL)) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
            Error::Conflict(format!(
                "The restore's journal is unreadable: {error}"
            ))
        })?,
        // Stopped before the journal: nothing was moved.
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            std::fs::remove_dir_all(stage)?;
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    if journal.step != Step::Done {
        let aside = Folder::open(stage)?;
        for path in &journal.roots {
            let kept = format!("old/{path}");
            // While unpacking, what is at a root is the backup's, or a part of it.
            if journal.step == Step::Unpack {
                folder.remove(path)?;
            }
            if aside.exists(&kept) {
                aside.move_to(&kept, folder, path)?;
            }
        }
    }
    std::fs::remove_dir_all(stage)?;
    Ok(())
}

#[cfg(test)]
mod tests;
