//! Domain instances - Typestate pattern for compile-time safety.
//! Guarantees you can only call valid operations per state.

use derive_more::Display;
use uuid::Uuid;
use crate::domain::ports::OSProcessManager;

/// Newtype wrapper for Instance IDs.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Display, Debug)]
pub struct InstanceId(pub Uuid);

impl InstanceId {
    pub fn new(id: Uuid) -> Self {
        InstanceId(id)
    }
}

/// Marker type for stopped instances.
pub struct Stopped;

/// Marker type for running instances.
pub struct Running {
    pub process_handle: Box<dyn ProcessHandle>,
}

/// ProcessHandle trait for OS processes.
pub trait ProcessHandle: Send {
    fn send_input(&self, input: &str) -> Result<(), DomainError>;
    fn is_running(&self) -> bool;
    fn wait(&self) -> Result<i32, DomainError>;
    
    /// Start streaming stdout/stderr to channel (for console output)
    /// This is where ANSI color codes are preserved
    fn start_output_stream(&self, tx: tokio::sync::mpsc::Sender<String>) -> Result<(), DomainError>;
}

/// GameInstance with typestate for compile-time safety.
pub struct GameInstance<State> {
    pub id: InstanceId,
    pub name: String,
    pub state: State,
}

impl GameInstance<Stopped> {
    /// Start a stopped instance.
    pub async fn start(
        self,
        manager: &dyn OSProcessManager,
    ) -> Result<GameInstance<Running>, DomainError> {
        let handle = manager.spawn("java -jar server.jar -nogui").await?;
        Ok(GameInstance {
            id: self.id,
            name: self.name,
            state: Running {
                process_handle: handle,
            },
        })
    }
}

impl GameInstance<Running> {
    /// Stop a running instance.
    pub async fn stop(self, _graceful: bool) -> Result<GameInstance<Stopped>, DomainError> {
        Ok(GameInstance {
            id: self.id,
            name: self.name,
            state: Stopped,
        })
    }

    /// Send console command.
    pub fn send_command(&self, cmd: &str) -> Result<(), DomainError> {
        self.state.process_handle.send_input(cmd)
    }
}

/// Domain error type.
#[derive(thiserror::Error, Debug)]
pub enum DomainError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Already running")]
    AlreadyRunning,
    #[error("Not running")]
    NotRunning,
    #[error("Spawn failed: {0}")]
    SpawnFailed(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("PTY error: {0}")]
    PtyError(String),
}

// Add implementations for error conversions
impl From<crate::infrastructure::process_spawner::ProcessSpawnerError> for DomainError {
    fn from(err: crate::infrastructure::process_spawner::ProcessSpawnerError) -> Self {
        DomainError::SpawnFailed(err.to_string())
    }
}

impl From<crate::infrastructure::deno_runtime::DenoRuntimeError> for DomainError {
    fn from(err: crate::infrastructure::deno_runtime::DenoRuntimeError) -> Self {
        DomainError::Database(err.to_string())
    }
}
