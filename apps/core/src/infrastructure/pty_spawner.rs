//! PTY Process Spawner - Spawns processes inside pseudo-terminal for color console support.
//! 
//! This allows Minecraft servers to output ANSI color codes by tricking them into
//! thinking they're running in a real terminal.
//!
//! TODO: Consider adding fallback to pipe-based spawner if PTY fails on certain platforms.
//! For now, PTY is the only spawner implementation.

use portable_pty::{PtySystem, CommandBuilder, NativePtySystem, MasterPty, Child as PtyChild};
use std::sync::Arc;
use tokio::sync::mpsc;
use crate::domain::instances::{ProcessHandle, DomainError};
use crate::domain::ports::OSProcessManager;

/// PTY-based process spawner.
pub struct PtySpawner {
    pty_system: NativePtySystem,
}

impl PtySpawner {
    pub fn new() -> Self {
        PtySpawner {
            pty_system: NativePtySystem::default(),
        }
    }
}

/// PTY process handle with output streaming capability.
pub struct PtyProcessHandle {
    pid: u32,
    // Keep PTY master alive - dropping it closes the PTY
    _master: Box<dyn MasterPty + Send>,
    // Child process
    _child: Box<dyn PtyChild + Send>,
}

#[async_trait::async_trait]
impl OSProcessManager for PtySpawner {
    async fn spawn(&self, cmd: &str) -> Result<Box<dyn ProcessHandle>, DomainError> {
        // Parse command into executable + args
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            return Err(DomainError::PtyError("Empty command".into()));
        }

        let exe = parts[0];
        let args: Vec<&str> = parts[1..].to_vec();

        let mut cmd_builder = CommandBuilder::new(exe);
        if !args.is_empty() {
            cmd_builder.args(&args);
        }

        // Open PTY with standard terminal size
        let pair = self.pty_system
            .openpty(portable_pty::PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| DomainError::PtyError(format!("Failed to open PTY: {}", e)))?;

        // Spawn command inside PTY
        let child = pair.slave
            .spawn_command(cmd_builder)
            .map_err(|e| DomainError::PtyError(format!("Failed to spawn in PTY: {}", e)))?;

        let pid = child.process_id().unwrap_or(0);

        Ok(Box::new(PtyProcessHandle {
            pid,
            _master: pair.master,
            _child: child,
        }))
    }

    async fn kill(&self, _pid: i32) -> Result<(), DomainError> {
        // TODO: Implement proper process killing
        // For now, just return Ok
        Ok(())
    }
}

impl ProcessHandle for PtyProcessHandle {
    fn send_input(&self, input: &str) -> Result<(), DomainError> {
        // TODO: Implement sending input to PTY (for console commands)
        // This requires writing to the PTY master
        tracing::debug!("Sending input to PTY: {}", input);
        Ok(())
    }

    fn is_running(&self) -> bool {
        // TODO: Implement proper process status check
        true
    }

    fn wait(&self) -> Result<i32, DomainError> {
        // TODO: Implement waiting for process to exit
        Ok(0)
    }

    fn start_output_stream(&self, tx: mpsc::Sender<String>) -> Result<(), DomainError> {
        // Get reader from PTY master (synchronous)
        let reader = self._master.try_clone_reader()
            .map_err(|e| DomainError::PtyError(format!("Failed to clone PTY reader: {}", e)))?;

        // Spawn blocking task to read PTY output (it's synchronous)
        let tx_clone = tx.clone();
        tokio::task::spawn_blocking(move || {
            use std::io::{BufRead, BufReader};
            
            let mut reader = BufReader::new(reader);
            let mut line = String::new();
            
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => {
                        // EOF - process ended
                        if !line.is_empty() {
                            let _ = tx_clone.blocking_send(line);
                        }
                        break;
                    }
                    Ok(_) => {
                        // Remove trailing newline for cleaner output
                        let line = line.trim_end().to_string();
                        if !line.is_empty() {
                            if tx_clone.blocking_send(line).is_err() {
                                // Receiver dropped, stop reading
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("PTY read error: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(())
    }
}

impl Drop for PtyProcessHandle {
    fn drop(&mut self) {
        // PTY master and child will be dropped automatically via Arc
        tracing::debug!("PTY process handle dropped (pid: {})", self.pid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_spawner_creation() {
        let spawner = PtySpawner::new();
        assert!(true); // Basic sanity check
    }
}
