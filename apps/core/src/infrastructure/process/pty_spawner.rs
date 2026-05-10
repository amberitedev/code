use std::io::Write as IoWrite;
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tokio::sync::mpsc;

use crate::ports::process_spawner::{ProcessHandle, ProcessSpawner, SpawnError};

pub struct PtyHandle {
    writer: Arc<Mutex<Box<dyn IoWrite + Send>>>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send>>>,
    stdout_rx: Option<mpsc::Receiver<String>>,
}

impl ProcessHandle for PtyHandle {
    fn send_stdin(&self, line: &str) -> Result<(), SpawnError> {
        let mut w = self.writer.lock().map_err(|e| SpawnError::Failed(e.to_string()))?;
        writeln!(w, "{}", line.trim_end_matches('\n'))
            .map_err(SpawnError::Io)
    }

    fn take_stdout_rx(&mut self) -> Option<mpsc::Receiver<String>> {
        self.stdout_rx.take()
    }

    fn is_running(&self) -> bool {
        let Ok(mut child) = self.child.lock() else { return false };
        matches!(child.try_wait(), Ok(None))
    }

    fn kill(&mut self) -> Result<(), SpawnError> {
        let mut child = self.child.lock().map_err(|e| SpawnError::Failed(e.to_string()))?;
        child.kill().map_err(SpawnError::Io)
    }

    fn pid(&self) -> Option<u32> {
        self.child.lock().ok()?.process_id()
    }
}

pub struct PtySpawner;

#[async_trait]
impl ProcessSpawner for PtySpawner {
    type Handle = PtyHandle;

    async fn spawn(
        &self,
        command: &str,
        args: &[&str],
        cwd: &Path,
        env: &[(&str, &str)],
    ) -> Result<Self::Handle, SpawnError> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize { rows: 50, cols: 200, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| SpawnError::Failed(e.to_string()))?;

        let mut cmd = CommandBuilder::new(command);
        for arg in args {
            cmd.arg(arg);
        }
        cmd.cwd(cwd);
        for (k, v) in env {
            cmd.env(k, v);
        }

        let child = pair.slave
            .spawn_command(cmd)
            .map_err(|e| SpawnError::Failed(e.to_string()))?;

        let writer = pair.master
            .take_writer()
            .map_err(|e| SpawnError::Failed(e.to_string()))?;

        let reader = pair.master
            .try_clone_reader()
            .map_err(|e| SpawnError::Failed(e.to_string()))?;

        // Spawn a thread to read stdout lines and forward to the async channel.
        let (tx, rx) = mpsc::channel::<String>(512);
        std::thread::spawn(move || {
            use std::io::BufRead;
            let buf = std::io::BufReader::new(reader);
            for line in buf.lines() {
                match line {
                    Ok(l) => {
                        if tx.blocking_send(l).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(PtyHandle {
            writer: Arc::new(Mutex::new(writer)),
            child: Arc::new(Mutex::new(child)),
            stdout_rx: Some(rx),
        })
    }
}
