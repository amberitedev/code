use std::path::Path;

use async_trait::async_trait;
use tokio::sync::mpsc;

#[derive(Debug, thiserror::Error)]
pub enum SpawnError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("spawn failed: {0}")]
    Failed(String),
}

/// Handle to a running child process.
pub trait ProcessHandle: Send + 'static {
    /// Write a line to the process stdin.
    fn send_stdin(&self, line: &str) -> Result<(), SpawnError>;
    /// Take the stdout line receiver. Can only succeed once.
    fn take_stdout_rx(&mut self) -> Option<mpsc::Receiver<String>>;
    /// Returns true if the process is still alive.
    fn is_running(&self) -> bool;
    /// Send SIGKILL (or TerminateProcess on Windows).
    fn kill(&mut self) -> Result<(), SpawnError>;
    /// Return the OS process ID if known.
    fn pid(&self) -> Option<u32> { None }
}

#[async_trait]
pub trait ProcessSpawner: Send + Sync + 'static {
    type Handle: ProcessHandle;

    async fn spawn(
        &self,
        command: &str,
        args: &[&str],
        cwd: &Path,
        env: &[(&str, &str)],
    ) -> Result<Self::Handle, SpawnError>;
}
