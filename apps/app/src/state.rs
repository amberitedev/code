//! The App's engine and the desktop state beside it, which App routes, reactions and the
//! engine's hooks all reach through [`Desktop`].

use std::ops::Deref;
use std::sync::{Arc, Weak};

use dashmap::DashMap;
use engine::Engine;
use sqlx::SqlitePool;
use tokio::sync::{Mutex, OwnedMutexGuard, broadcast};

use crate::local_api::events::AppEvent;
use crate::synced_options::state::SyncedOptions;

/// The App's engine with app.db and the state that lives as long as it. Clones share both. It
/// derefs to the engine, so engine calls take it as they are.
#[derive(Clone)]
pub struct Desktop {
    engine: Arc<Engine>,
    state: Arc<DesktopState>,
}

/// What the App keeps beside its engine.
pub struct DesktopState {
    pub(crate) db: SqlitePool,
    events: broadcast::Sender<AppEvent>,
    pub(crate) synced_options: SyncedOptions,
    screenshots: DashMap<String, Arc<Mutex<()>>>,
}

impl DesktopState {
    pub fn new(db: SqlitePool, events: broadcast::Sender<AppEvent>) -> Self {
        Self {
            db,
            events,
            synced_options: SyncedOptions::default(),
            screenshots: DashMap::default(),
        }
    }
}

impl Desktop {
    pub fn new(engine: Arc<Engine>, state: Arc<DesktopState>) -> Self {
        Self { engine, state }
    }

    pub fn engine(&self) -> &Arc<Engine> {
        &self.engine
    }

    pub fn downgrade(&self) -> WeakDesktop {
        WeakDesktop {
            engine: Arc::downgrade(&self.engine),
            state: Arc::clone(&self.state),
        }
    }

    /// app.db.
    pub fn db(&self) -> &SqlitePool {
        &self.state.db
    }

    pub(crate) fn synced_options(&self) -> &SyncedOptions {
        &self.state.synced_options
    }

    /// Sends an App event to every `/events` socket.
    pub fn emit(&self, event: AppEvent) {
        let _ = self.state.events.send(event);
    }

    pub fn app_events(&self) -> &broadcast::Sender<AppEvent> {
        &self.state.events
    }

    /// Serializes screenshot changes in one instance.
    pub async fn lock_screenshots(
        &self,
        instance_id: &str,
    ) -> OwnedMutexGuard<()> {
        let lock = self
            .state
            .screenshots
            .entry(instance_id.to_string())
            .or_default()
            .clone();
        lock.lock_owned().await
    }

    /// Drops the in-memory state of a removed instance.
    pub fn forget_instance(&self, instance_id: &str) {
        self.state.screenshots.remove(instance_id);
    }
}

/// A [`Desktop`] that does not keep its engine alive, for background work that ends with it.
#[derive(Clone)]
pub struct WeakDesktop {
    engine: Weak<Engine>,
    state: Arc<DesktopState>,
}

impl WeakDesktop {
    pub fn upgrade(&self) -> Option<Desktop> {
        Some(Desktop::new(
            self.engine.upgrade()?,
            Arc::clone(&self.state),
        ))
    }
}

impl Deref for Desktop {
    type Target = Engine;

    fn deref(&self) -> &Engine {
        &self.engine
    }
}
