//! Process spawner - Implements OSProcessManager for spawning Java servers.

use tokio::process::Command;
use thiserror::Error;
use crate::domain::instances::{ProcessHandle, DomainError};
use crate::domain::ports::OSProcessManager;

/// Process spawner error.
#[derive(Error, Debug)]
pub enum ProcessSpawnerError {
    #[error("Spawn failed: {0}")]
    SpawnFailed(String),
}

/// System process spawner.
pub struct SystemSpawner;

impl SystemSpawner {
    pub fn new() -> Self {
        SystemSpawner
    }
}

#[async_trait::async_trait]
impl OSProcessManager for SystemSpawner {
    async fn spawn(&self, cmd: &str) -> Result<Box<dyn ProcessHandle>, DomainError> {
        let child = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .spawn()
            .map_err(|e| ProcessSpawnerError::SpawnFailed(e.to_string()))?;

        Ok(Box::new(SystemProcessHandle {
            pid: child.id().unwrap_or(0),
            _child: child,
        }))
    }

    async fn kill(&self, _pid: i32) -> Result<(), DomainError> {
        Ok(())
    }
}

/// System process handle.
pub struct SystemProcessHandle {
    pid: u32,
    _child: tokio::process::Child,
}

impl ProcessHandle for SystemProcessHandle {
    fn send_input(&self, _input: &str) -> Result<(), DomainError> {
        Ok(())
    }

    fn is_running(&self) -> bool {
        true
    }

    fn wait(&self) -> Result<i32, DomainError> {
        Ok(0)
    }

    fn start_output_stream(&self, _tx: tokio::sync::mpsc::Sender<String>) -> Result<(), DomainError> {
        // TODO: Implement stdout streaming for pipe-based spawner
        // For now, this is a no-op (PTY spawner should be used instead)
        tracing::warn!("Output streaming not implemented for pipe-based spawner");
        Ok(())
    }
}

/// Mock process spawner for testing.
pub struct MockProcessSpawner;

impl MockProcessSpawner {
    pub fn new() -> Self {
        MockProcessSpawner
    }
}

#[async_trait::async_trait]
impl OSProcessManager for MockProcessSpawner {
    async fn spawn(&self, _cmd: &str) -> Result<Box<dyn ProcessHandle>, DomainError> {
        Ok(Box::new(MockProcessHandle))
    }

    async fn kill(&self, _pid: i32) -> Result<(), DomainError> {
        Ok(())
    }
}

/// Mock process handle.
pub struct MockProcessHandle;

impl ProcessHandle for MockProcessHandle {
    fn send_input(&self, _input: &str) -> Result<(), DomainError> {
        Ok(())
    }

    fn is_running(&self) -> bool {
        true
    }

    fn wait(&self) -> Result<i32, DomainError> {
        Ok(0)
    }

    fn start_output_stream(&self, _tx: tokio::sync::mpsc::Sender<String>) -> Result<(), DomainError> {
        // Mock implementation - no output
        Ok(())
    }
}
