use crate::install::control::InstallControl;
use crate::install::store::LiveJob;
use std::collections::HashMap;
use std::sync::{Arc, Mutex as SyncMutex, Weak};
use tokio::sync::{Mutex, Notify, Semaphore};
use uuid::Uuid;

const MAX_CONCURRENT_INSTALL_JOBS: usize = 3;

/// Limits install job concurrency and holds the in-memory state of this engine's jobs.
#[derive(Debug)]
pub(crate) struct Installs {
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
    pub(crate) fn new() -> Self {
        Self {
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
