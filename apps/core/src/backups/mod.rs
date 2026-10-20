//! The one queued backup engine. A backup is a zip of the server's worlds and a short list of
//! settings files, in `backups/<server>/<backup>.zip`, which only `copal` can reach. Creates and
//! restores queue in `backup_operations` and run one at a time, so a schedule, an update, and a
//! person never copy at once.
//!
//! A running server is backed up hot: Core asks its shim to hold autosave off (the shim turns it
//! back on itself after ten minutes, should Core never come back), runs `save-all flush`, waits
//! for the save, copies, and releases. Mods that write outside the save cycle can still make a
//! hot backup slightly inconsistent; that is accepted.
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

use sqlx::SqlitePool;
use tokio::sync::{Notify, broadcast};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    access::{Caller, now},
    api::events::{BackupState, BackupTask, WsEvent},
    content::folder::{self, Folder},
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
        });
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
        if let Err(error) = records::start(&self.db, &operation).await {
            warn!(%error, "could not start a backup operation");
            tokio::time::sleep(Duration::from_secs(5)).await;
            return;
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

    /// Zip the server's worlds and settings into `destination`, hot when the server runs.
    /// Checks the free space first and leaves nothing behind when it fails. Returns the zip's
    /// size. `progress` is told the share copied.
    pub async fn snapshot(
        &self,
        server_id: &str,
        destination: &Path,
        progress: impl Fn(f64) + Send + 'static,
    ) -> Result<u64, Error> {
        let _copying = self.copying.lock().await;
        let folder = Arc::new(Folder::open(&self.paths.server(server_id))?);
        let files = {
            let folder = Arc::clone(&folder);
            tokio::task::spawn_blocking(move || files(&folder))
                .await
                .map_err(io::Error::other)??
        };
        let total: u64 = files.iter().map(|(_, size)| size).sum();
        let parent = destination.parent().expect("backups are in a folder");
        std::fs::create_dir_all(parent)?;
        enough_space(total, space::free(parent)?)?;

        let hot = self.servers.phase(server_id) == Some(Phase::Running);
        if hot {
            self.hold_and_save(server_id).await?;
        }
        let staged = destination.with_extension("zip.part");
        let result = {
            let staged = staged.clone();
            let folder = Arc::clone(&folder);
            tokio::task::spawn_blocking(move || {
                write_zip(&folder, &files, &staged, total, &progress)
            })
            .await
            .map_err(io::Error::other)
            .map_err(Error::from)
            .and_then(|result| result)
        };
        if hot && let Err(error) = self.servers.release_saves(server_id).await {
            warn!(server = %server_id, %error, "could not turn autosave back on; the shim will");
        }
        match result.and_then(|()| Ok(std::fs::rename(&staged, destination)?)) {
            Ok(()) => Ok(std::fs::metadata(destination)?.len()),
            Err(error) => {
                let _ = std::fs::remove_file(&staged);
                Err(error)
            }
        }
    }

    /// Hold autosave off and wait for a full save to finish.
    async fn hold_and_save(&self, server_id: &str) -> Result<(), Error> {
        self.servers.hold_saves(server_id, SAVE_HOLD_S).await?;
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
        let saved = tokio::time::timeout(SAVE_WAIT, async {
            while let Some(line) = lines.recv().await {
                if line.contains("Saved the game")
                    || line.contains("Saved the world")
                {
                    return true;
                }
            }
            false
        })
        .await;
        if !matches!(saved, Ok(true)) {
            warn!(server = %server_id, "the server did not report its save; copying anyway");
        }
        Ok(())
    }

    /// Replace the server's worlds and settings with a backup's. Holds the server so it cannot
    /// start meanwhile.
    async fn run_restore(&self, operation: &Operation) -> Result<(), Error> {
        let server_id = &operation.server_id;
        let _hold =
            self.servers.hold(server_id).map_err(|error| match error {
                ServerError::AlreadyRunning => Error::Conflict(
                    "Stop the server before restoring a backup.".into(),
                ),
                error => error.into(),
            })?;
        let _copying = self.copying.lock().await;
        let zip = self.file(server_id, &operation.backup_id);
        let folder = Arc::new(Folder::open(&self.paths.server(server_id))?);
        tokio::task::spawn_blocking(move || restore(&folder, &zip))
            .await
            .map_err(io::Error::other)?
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
            let running =
                self.servers.phase(&server_id) == Some(Phase::Running);
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

/// The worlds of a server folder, by name.
fn worlds(folder: &Folder) -> [String; 3] {
    let level = folder.level_name();
    [
        level.clone(),
        format!("{level}_nether"),
        format!("{level}_the_end"),
    ]
}

/// Every file a backup holds, with its size: the worlds, then the settings. Links are skipped.
fn files(folder: &Folder) -> io::Result<Vec<(String, u64)>> {
    let mut files = Vec::new();
    for world in worlds(folder) {
        if let Ok(dir) = folder.open_dir(Some(&world)) {
            walk(&dir, &world, &mut files)?;
        }
    }
    for name in SETTINGS {
        if let Ok(meta) = folder.metadata(name)
            && meta.is_file()
        {
            files.push((name.to_owned(), meta.len()));
        }
    }
    Ok(files)
}

fn walk(
    dir: &cap_std::fs::Dir,
    prefix: &str,
    files: &mut Vec<(String, u64)>,
) -> io::Result<()> {
    for entry in dir.entries()?.filter_map(Result::ok) {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        // Held locked by a running server, and meaningless in a backup.
        if name == "session.lock" {
            continue;
        }
        let path = format!("{prefix}/{name}");
        let kind = entry.file_type()?;
        if kind.is_dir() {
            walk(&folder::open_child(dir, &name, &path)?, &path, files)?;
        } else if kind.is_file() {
            files.push((path, entry.metadata()?.len()));
        }
    }
    Ok(())
}

/// Write `files` from `folder` into a zip at `path`. Region files are stored, being compressed
/// already; the rest is deflated. A file gone by the time it is read is left out.
fn write_zip(
    folder: &Folder,
    files: &[(String, u64)],
    path: &Path,
    total: u64,
    progress: &dyn Fn(f64),
) -> Result<(), Error> {
    use zip::{CompressionMethod, write::SimpleFileOptions};
    let mut zip =
        zip::ZipWriter::new(io::BufWriter::new(std::fs::File::create(path)?));
    let mut done = 0u64;
    for (name, size) in files {
        let Some(mut file) = folder.open_file(name)? else {
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
        let mut buffer = vec![0; 256 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            zip.write_all(&buffer[..read])?;
            done += read as u64;
            progress((done as f64 / total.max(1) as f64).min(1.0));
        }
    }
    zip.finish()?.flush()?;
    Ok(())
}

/// Replace the worlds and settings in `folder` with those in the backup at `zip`.
fn restore(folder: &Folder, zip: &Path) -> Result<(), Error> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(zip)?)?;
    let entries =
        crate::content::archive::entries(&mut archive).map_err(|error| {
            io::Error::new(io::ErrorKind::InvalidData, error.to_string())
        })?;
    // The worlds as named now and everything the backup holds go first, so nothing of the old
    // world is left mixed into the restored one. Core made the backup: it holds only worlds and
    // settings.
    let mut remove: Vec<String> = worlds(folder).into();
    for (path, _) in &entries {
        let top = path.split('/').next().unwrap_or(path).to_owned();
        if !remove.contains(&top) {
            remove.push(top);
        }
    }
    for path in &remove {
        folder.remove(path)?;
    }
    for (index, (path, is_dir)) in entries.iter().enumerate() {
        if *is_dir {
            folder.create_dir_all(path)?;
            continue;
        }
        folder.write(path, &mut archive.by_index(index)?)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
