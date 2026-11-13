mod game_option_model;
mod game_options;

pub use self::game_option_model::{CanonicalValue, GameOptionKind};
pub(crate) use self::game_option_model::{StoredOption, StoredPreference};
pub(crate) use self::game_options::*;

use crate::synced_options::{GameLocaleIndexer, PackSyncWorker};
use tokio::sync::{Mutex, MutexGuard};

/// Coordinates options syncing across instances.
#[derive(Default)]
pub(crate) struct SyncedOptions {
    /// Serializes canonical synced-option mutations and checkpoint updates.
    lock: Mutex<()>,
    pub(crate) locales: GameLocaleIndexer,
    pub(crate) packs: PackSyncWorker,
}

impl SyncedOptions {
    pub(crate) async fn lock(&self) -> MutexGuard<'_, ()> {
        self.lock.lock().await
    }
}
