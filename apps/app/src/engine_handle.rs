use contracts::{ApiError, ErrorCode};
use engine::Engine;
use std::sync::Arc;
use tokio::sync::watch;

/// The engine App setup starts, shared by Tauri commands and the local server's engine routes.
#[derive(Clone)]
pub struct EngineHandle {
    startup: watch::Sender<Startup>,
}

enum Startup {
    Starting,
    Ready(Arc<Engine>),
    /// Why it did not start, ready to show.
    Failed(String),
}

impl Default for EngineHandle {
    fn default() -> Self {
        Self {
            startup: watch::Sender::new(Startup::Starting),
        }
    }
}

impl EngineHandle {
    pub fn set(&self, started: anyhow::Result<Arc<Engine>>) {
        self.startup.send_replace(match started {
            Ok(engine) => Startup::Ready(engine),
            Err(error) => Startup::Failed(format!("{error:#}")),
        });
    }

    /// Never returns after a failed start; the page shows that failure from `/events` instead.
    pub async fn get(&self) -> Arc<Engine> {
        let mut startup = self.startup.subscribe();
        let startup = startup
            .wait_for(|startup| matches!(startup, Startup::Ready(_)))
            .await
            .expect("the handle owns its sender");
        match &*startup {
            Startup::Ready(engine) => Arc::clone(engine),
            _ => unreachable!("wait_for returns only a ready engine"),
        }
    }

    /// Waits for startup to end, and answers `engine_unavailable` when it failed.
    pub async fn ready(&self) -> Result<Arc<Engine>, ApiError> {
        let mut startup = self.startup.subscribe();
        let startup = startup
            .wait_for(|startup| !matches!(startup, Startup::Starting))
            .await
            .expect("the handle owns its sender");
        match &*startup {
            Startup::Ready(engine) => Ok(Arc::clone(engine)),
            Startup::Failed(description) => Err(ApiError::new(
                ErrorCode::EngineUnavailable,
                description.clone(),
            )),
            Startup::Starting => {
                unreachable!("wait_for returns a settled startup")
            }
        }
    }

    /// The engine if startup has set it, without waiting.
    pub fn try_get(&self) -> Option<Arc<Engine>> {
        match &*self.startup.borrow() {
            Startup::Ready(engine) => Some(Arc::clone(engine)),
            _ => None,
        }
    }
}
