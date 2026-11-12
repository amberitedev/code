use contracts::{ApiError, ErrorCode};
use engine::{Engine, access::BoxFuture};
use std::sync::{Arc, OnceLock};
use tokio::sync::watch;

/// The engine App setup starts, shared by Tauri commands and the local server's engine routes.
#[derive(Clone)]
pub struct EngineHandle {
    startup: watch::Sender<Startup>,
    start: Arc<OnceLock<Start>>,
}

type Start = Box<
    dyn Fn() -> BoxFuture<'static, anyhow::Result<Arc<Engine>>> + Send + Sync,
>;

enum Startup {
    Starting,
    Ready(Arc<Engine>),
    Failed(ApiError),
}

impl Default for EngineHandle {
    fn default() -> Self {
        Self {
            startup: watch::Sender::new(Startup::Starting),
            start: Arc::default(),
        }
    }
}

impl EngineHandle {
    /// Runs `start` now, and again whenever [`Self::retry`] follows a failure.
    pub fn start<F>(&self, start: impl Fn() -> F + Send + Sync + 'static)
    where
        F: Future<Output = anyhow::Result<Arc<Engine>>> + Send + 'static,
    {
        if self.start.set(Box::new(move || Box::pin(start()))).is_ok() {
            self.run();
        }
    }

    /// Starts again after a failed start. Does nothing while starting or once ready.
    pub fn retry(&self) {
        if self.start.get().is_none() {
            return;
        }
        let failed = self.startup.send_if_modified(|startup| {
            let failed = matches!(startup, Startup::Failed(_));
            if failed {
                *startup = Startup::Starting;
            }
            failed
        });
        if failed {
            self.run();
        }
    }

    fn run(&self) {
        let Some(start) = self.start.get() else {
            return;
        };
        let started = start();
        let handle = self.clone();
        tauri::async_runtime::spawn(async move { handle.set(started.await) });
    }

    pub fn set(&self, started: anyhow::Result<Arc<Engine>>) {
        self.startup.send_replace(match started {
            Ok(engine) => Startup::Ready(engine),
            Err(error) => Startup::Failed(startup_error(error)),
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

    /// Waits for startup to end, and answers why it failed.
    pub async fn ready(&self) -> Result<Arc<Engine>, ApiError> {
        let mut startup = self.startup.subscribe();
        let startup = startup
            .wait_for(|startup| !matches!(startup, Startup::Starting))
            .await
            .expect("the handle owns its sender");
        match &*startup {
            Startup::Ready(engine) => Ok(Arc::clone(engine)),
            Startup::Failed(error) => Err(error.clone()),
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

/// A failed directory move keeps its code, so the page can offer to retry or cancel it.
fn startup_error(error: anyhow::Error) -> ApiError {
    let error = engine::routes::api_error(error);
    match error.error {
        ErrorCode::DirectoryMoveNoSpace
        | ErrorCode::DirectoryMovePermissionDenied
        | ErrorCode::DirectoryMoveFailed => error,
        _ => ApiError::new(ErrorCode::EngineUnavailable, error.description),
    }
}
