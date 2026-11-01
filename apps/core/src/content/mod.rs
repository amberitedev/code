//! Content: the files on a server's disk, with the origin of each tracked file recorded in
//! `content_files` (from the pack, or added on the server). No mods table: the folders are the
//! list, and a row only says who put a file there and what it was then, which is how Core knows
//! the owner edited it (see `pack::edited`).
//!
//! One change at a time per server: a second one is refused as busy rather than queued. Loader and
//! pack installs run in the background, hold the server so it cannot start, and report progress
//! and their result to the server's console sockets as `install-progress` and
//! `installation-result`.

pub mod addons;
pub mod archive;
pub mod folder;
pub mod hosts;
pub mod install;
pub mod manifest;
pub mod modrinth;
pub mod pack;
pub mod properties;
pub mod records;
pub mod shared;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use sqlx::SqlitePool;
use tokio::sync::{OwnedMutexGuard, broadcast};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    api::{
        content::Installing,
        events::{
            GameVersionChange, InstallProgressItem, InstallProgressKey,
            InstallationResult, WsEvent,
        },
    },
    backups::Backups,
    daemon::paths::Paths,
    installs::{self, download},
    servers::{Hold, ServerError, Servers},
};

/// Why a content change failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "the server is busy with another change to its content; try again when it is done"
    )]
    Busy,
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Server(#[from] ServerError),
    #[error(transparent)]
    Modrinth(#[from] modrinth::ModrinthError),
    #[error(transparent)]
    Pack(#[from] pack::PackError),
    #[error(transparent)]
    Archive(#[from] archive::ArchiveError),
    #[error(transparent)]
    Path(#[from] folder::PathError),
    #[error(transparent)]
    Install(#[from] installs::InstallError),
    #[error(transparent)]
    Download(#[from] download::DownloadError),
    #[error(transparent)]
    Backup(#[from] crate::backups::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Other(#[from] color_eyre::Report),
}

/// What a server's content is doing, for the content list.
#[derive(Debug, Clone, Default)]
pub struct Activity {
    pub installing: Option<Installing>,
    pub items: Vec<InstallProgressItem>,
    /// Why the last background install failed, until the next one starts.
    pub error: Option<String>,
}

/// Content state shared by every route.
pub struct Content {
    pub paths: Paths,
    pub db: SqlitePool,
    pub servers: Arc<Servers>,
    /// Core's own fixed sources. See `installs::download::client`.
    pub http: reqwest::Client,
    /// Pack content, from allowed hosts only. See `hosts`.
    pub downloads: reqwest::Client,
    pub modrinth: modrinth::Modrinth,
    /// What addon jars say about themselves, for the content list.
    pub manifests: manifest::Cache,
    /// Updates back the world up first.
    pub backups: Arc<Backups>,
    activity: Mutex<HashMap<String, Activity>>,
    busy: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// Install progress and results, by server, on the channel backups use. Console sockets
    /// pass them on.
    pub events: broadcast::Sender<(String, WsEvent)>,
}

/// One server's content, taken for a change. Dropping it lets the next change in.
pub struct Busy {
    _guard: OwnedMutexGuard<()>,
}

impl Content {
    /// Content for every server; also starts the check for newer Java patches.
    pub fn open(
        paths: Paths,
        db: SqlitePool,
        servers: Arc<Servers>,
        backups: Arc<Backups>,
    ) -> color_eyre::Result<Arc<Self>> {
        let http = download::client()?;
        let content = Arc::new(Self {
            modrinth: modrinth::Modrinth::new(http.clone()),
            manifests: manifest::Cache::default(),
            downloads: hosts::client()?,
            http,
            paths,
            db,
            servers,
            activity: Mutex::default(),
            busy: Mutex::default(),
            events: backups.events.clone(),
            backups,
        });
        let java = Arc::clone(&content);
        tokio::spawn(async move {
            installs::java::update_all(&java.paths, &java.http).await
        });
        Ok(content)
    }

    /// Take a server's content for a change, or fail at once when another change has it.
    pub fn busy(&self, server_id: &str) -> Result<Busy, Error> {
        let lock = Arc::clone(
            self.busy
                .lock()
                .expect("not poisoned")
                .entry(server_id.to_owned())
                .or_default(),
        );
        lock.try_lock_owned()
            .map(|guard| Busy { _guard: guard })
            .map_err(|_| Error::Busy)
    }

    /// Take a server's content for a change, waiting for the change that has it to finish.
    pub async fn busy_wait(&self, server_id: &str) -> Busy {
        let lock = Arc::clone(
            self.busy
                .lock()
                .expect("not poisoned")
                .entry(server_id.to_owned())
                .or_default(),
        );
        Busy {
            _guard: lock.lock_owned().await,
        }
    }

    pub fn activity(&self, server_id: &str) -> Activity {
        self.activity
            .lock()
            .expect("not poisoned")
            .get(server_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Forget why the last background install failed, once what failed is gone.
    pub fn clear_error(&self, server_id: &str) {
        self.update(server_id, |activity| activity.error = None);
    }

    fn update(
        &self,
        server_id: &str,
        change: impl FnOnce(&mut Activity),
    ) -> Activity {
        let mut activity = self.activity.lock().expect("not poisoned");
        let entry = activity.entry(server_id.to_owned()).or_default();
        change(entry);
        entry.clone()
    }

    /// Progress for an install the caller runs itself, outside [`Content::spawn`]. The caller
    /// ends it with [`Progress::finish`].
    pub fn progress(self: &Arc<Self>, server_id: &str) -> Progress {
        self.update(server_id, |activity| {
            activity.items.clear();
            activity.error = None;
        });
        Progress {
            content: Arc::clone(self),
            server_id: server_id.to_owned(),
        }
    }

    fn emit(&self, server_id: &str, event: WsEvent) {
        let _ = self.events.send((server_id.to_owned(), event));
    }

    /// Run an install in the background with the server held and its content taken. Its
    /// progress and result go to the server's console sockets.
    pub fn spawn<F, Fut>(
        self: &Arc<Self>,
        hold: Hold,
        busy: Busy,
        installing: Installing,
        work: F,
    ) where
        F: FnOnce(Arc<Self>, Hold, Progress) -> Fut + Send + 'static,
        Fut: Future<Output = Result<Option<GameVersionChange>, Error>> + Send,
    {
        let server_id = hold.id().to_owned();
        self.update(&server_id, |activity| {
            *activity = Activity {
                installing: Some(installing),
                ..Activity::default()
            };
        });
        let content = Arc::clone(self);
        tokio::spawn(async move {
            let progress = Progress {
                content: Arc::clone(&content),
                server_id: server_id.clone(),
            };
            let result =
                work(Arc::clone(&content), hold, progress.clone()).await;
            drop(busy);
            let (game_version, error) = match result {
                Ok(change) => (change, None),
                Err(error) => (None, Some(format!("{error:#}"))),
            };
            match &error {
                None => {
                    info!(server = %server_id, ?installing, "install finished")
                }
                Some(error) => {
                    warn!(server = %server_id, ?installing, %error, "install failed")
                }
            }
            progress.finish(error.as_deref());
            content.emit(
                &server_id,
                WsEvent::InstallationResult(match error {
                    None => InstallationResult::Ok { game_version },
                    Some(reason) => InstallationResult::Err {
                        reason: Some(reason),
                    },
                }),
            );
        });
    }
}

/// Reports one install's progress, item by item.
#[derive(Clone)]
pub struct Progress {
    content: Arc<Content>,
    server_id: String,
}

impl Progress {
    /// Set how far `id` is, from 0 to 1. Sent on as a percent, the App's scale, when it moved a
    /// whole percent.
    pub fn set(&self, id: &str, key: &InstallProgressKey, progress: f64) {
        let progress = (progress.clamp(0.0, 1.0) * 100.0).floor();
        let world_id = Uuid::parse_str(&self.server_id).unwrap_or_default();
        let mut changed = false;
        let activity = self.content.update(&self.server_id, |activity| {
            match activity.items.iter_mut().find(|item| item.id == id) {
                Some(item) => {
                    changed = item.progress != Some(progress);
                    item.progress = Some(progress);
                }
                None => {
                    changed = true;
                    activity.items.push(InstallProgressItem {
                        world_id,
                        key: key.clone(),
                        id: id.to_owned(),
                        progress: Some(progress),
                        error: None,
                    });
                }
            }
        });
        if changed {
            self.content.emit(
                &self.server_id,
                WsEvent::InstallProgress {
                    items: activity.items,
                },
            );
        }
    }

    /// End the install: every item is done, or failed with `error`, which the App shows. The
    /// items stay until the next install starts, so a page opened later sees the outcome too.
    pub fn finish(&self, error: Option<&str>) {
        let activity = self.content.update(&self.server_id, |activity| {
            activity.installing = None;
            activity.error = error.map(str::to_owned);
            for item in &mut activity.items {
                match error {
                    None => item.progress = Some(100.0),
                    Some(error) => item.error = Some(error.to_owned()),
                }
            }
        });
        self.content.emit(
            &self.server_id,
            WsEvent::InstallProgress {
                items: activity.items,
            },
        );
    }
}
