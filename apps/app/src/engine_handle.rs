use engine::Engine;
use std::sync::Arc;
use tokio::sync::{Mutex, watch};

/// The engine Tauri commands run against, managed by the App. `initialize_state` sets it; a
/// command that runs before then waits for it.
pub struct EngineHandle {
    engine: watch::Sender<Option<Arc<Engine>>>,
    starting: Mutex<()>,
}

impl Default for EngineHandle {
    fn default() -> Self {
        Self {
            engine: watch::Sender::new(None),
            starting: Mutex::new(()),
        }
    }
}

impl EngineHandle {
    /// Runs `start` only while no engine is set, so a webview reload reuses the running engine.
    pub async fn get_or_start<F>(
        &self,
        start: impl FnOnce() -> F,
    ) -> engine::Result<Arc<Engine>>
    where
        F: Future<Output = engine::Result<Arc<Engine>>>,
    {
        let _starting = self.starting.lock().await;
        if let Some(engine) = self.try_get() {
            return Ok(engine);
        }
        let engine = start().await?;
        self.engine.send_replace(Some(Arc::clone(&engine)));
        Ok(engine)
    }

    pub async fn get(&self) -> Arc<Engine> {
        let mut engine = self.engine.subscribe();
        let engine = engine
            .wait_for(Option::is_some)
            .await
            .expect("the handle owns its sender");
        engine.clone().expect("wait_for returns only a set engine")
    }

    /// The engine if startup has set it, without waiting.
    pub fn try_get(&self) -> Option<Arc<Engine>> {
        self.engine.borrow().clone()
    }
}
