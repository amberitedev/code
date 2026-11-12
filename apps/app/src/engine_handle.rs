use engine::State;
use std::sync::Arc;
use tokio::sync::watch;

/// The engine Tauri commands run against, managed by the App. `initialize_state` sets it; a
/// command that runs before then waits for it.
pub struct EngineHandle(watch::Sender<Option<Arc<State>>>);

impl Default for EngineHandle {
    fn default() -> Self {
        Self(watch::Sender::new(None))
    }
}

impl EngineHandle {
    pub fn set(&self, engine: Arc<State>) {
        self.0.send_replace(Some(engine));
    }

    pub async fn get(&self) -> Arc<State> {
        let mut engine = self.0.subscribe();
        let engine = engine
            .wait_for(Option::is_some)
            .await
            .expect("the handle owns its sender");
        engine.clone().expect("wait_for returns only a set engine")
    }

    /// The engine if startup has set it, without waiting.
    pub fn try_get(&self) -> Option<Arc<State>> {
        self.0.borrow().clone()
    }
}
