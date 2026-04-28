use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::ports::process_spawner::{ProcessHandle, ProcessSpawner, SpawnError};

/// Fake spawner used in tests — no real process is created.
pub struct MockSpawner;

pub struct MockHandle {
    stdin_tx: mpsc::Sender<String>,
    stdout_rx: Option<mpsc::Receiver<String>>,
    running: Arc<AtomicBool>,
}

impl MockHandle {
    /// Feed a line of "output" to the handle's stdout channel from tests.
    pub fn feed_output(&self, line: &str) {
        let _ = self.stdin_tx.try_send(line.to_string());
    }
}

impl ProcessHandle for MockHandle {
    fn send_stdin(&self, line: &str) -> Result<(), SpawnError> {
        let _ = self.stdin_tx.try_send(line.to_string());
        Ok(())
    }

    fn take_stdout_rx(&mut self) -> Option<mpsc::Receiver<String>> {
        self.stdout_rx.take()
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn kill(&mut self) -> Result<(), SpawnError> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }
}

#[async_trait]
impl ProcessSpawner for MockSpawner {
    type Handle = MockHandle;

    async fn spawn(
        &self,
        _command: &str,
        _args: &[&str],
        _cwd: &Path,
        _env: &[(&str, &str)],
    ) -> Result<Self::Handle, SpawnError> {
        let (stdin_tx, stdout_rx) = mpsc::channel(64);
        Ok(MockHandle {
            stdin_tx,
            stdout_rx: Some(stdout_rx),
            running: Arc::new(AtomicBool::new(true)),
        })
    }
}
