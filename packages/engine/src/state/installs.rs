use crate::install::control::InstallControl;
use crate::install::store::LiveJob;
use crate::state::DirectoryInfo;
use contracts::Side;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as SyncMutex, Weak};
use tokio::sync::{Mutex, Notify, Semaphore};
use uuid::Uuid;

const MAX_CONCURRENT_INSTALL_JOBS: usize = 3;

/// Limits install job concurrency and holds the in-memory state of this engine's jobs.
#[derive(Debug)]
pub(crate) struct Installs {
    /// A server engine's jobs are read by remote callers, so they name no host path. These are
    /// its folders, which a job's messages name by their own name instead. `None` on a client.
    pub(crate) server_folders: Option<Vec<PathBuf>>,
    /// Admission covers setup and deletion. A target reservation stays with its worker
    /// until cleanup finishes, so backups and rollback cannot overlap another install.
    pub(crate) admission: Mutex<()>,
    pub(crate) job_semaphore: Semaphore,
    pub(crate) db_semaphore: Semaphore,
    pub(crate) controls: SyncMutex<HashMap<Uuid, Arc<InstallControl>>>,
    pub(crate) targets: SyncMutex<HashMap<String, Weak<Mutex<()>>>>,
    pub(crate) live_jobs: SyncMutex<HashMap<Uuid, LiveJob>>,
    pub(crate) completions: SyncMutex<HashMap<Uuid, Weak<Notify>>>,
}

impl Installs {
    pub(crate) fn new(side: Side, directories: &DirectoryInfo) -> Self {
        let server_folders = (side == Side::Server).then(|| {
            let mut folders = vec![
                directories.config_dir.clone(),
                directories.instances_dir(),
                directories.metadata_dir(),
            ];
            // A folder inside another one goes first, so it keeps its own name.
            folders.sort_by_key(|folder| {
                std::cmp::Reverse(folder.as_os_str().len())
            });
            folders
        });
        Self {
            server_folders,
            admission: Mutex::new(()),
            job_semaphore: Semaphore::new(MAX_CONCURRENT_INSTALL_JOBS),
            db_semaphore: Semaphore::new(1),
            controls: SyncMutex::default(),
            targets: SyncMutex::default(),
            live_jobs: SyncMutex::default(),
            completions: SyncMutex::default(),
        }
    }
}
