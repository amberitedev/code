use std::{
    io::Write as IoWrite,
    path::Path,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::ports::process_spawner::{
    ProcessHandle, ProcessSpawner, SpawnError,
};

pub struct StdHandle {
    stdin: Arc<Mutex<std::process::ChildStdin>>,
    child: Arc<Mutex<std::process::Child>>,
    stdout_rx: Option<mpsc::Receiver<String>>,
}

impl ProcessHandle for StdHandle {
    fn send_stdin(&self, line: &str) -> Result<(), SpawnError> {
        let mut stdin = self
            .stdin
            .lock()
            .map_err(|e| SpawnError::Failed(e.to_string()))?;
        writeln!(stdin, "{}", line.trim_end_matches('\n'))
            .map_err(SpawnError::Io)
    }

    fn take_stdout_rx(&mut self) -> Option<mpsc::Receiver<String>> {
        self.stdout_rx.take()
    }

    fn is_running(&self) -> bool {
        let Ok(mut child) = self.child.lock() else {
            return false;
        };
        matches!(child.try_wait(), Ok(None))
    }

    fn kill(&mut self) -> Result<(), SpawnError> {
        let mut child = self
            .child
            .lock()
            .map_err(|e| SpawnError::Failed(e.to_string()))?;
        child.kill().map_err(SpawnError::Io)
    }

    fn pid(&self) -> Option<u32> {
        Some(self.child.lock().ok()?.id())
    }
}

pub struct StdSpawner;

#[async_trait]
impl ProcessSpawner for StdSpawner {
    type Handle = StdHandle;

    async fn spawn(
        &self,
        command: &str,
        args: &[&str],
        cwd: &Path,
        env: &[(&str, &str)],
    ) -> Result<Self::Handle, SpawnError> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in env {
            cmd.env(key, value);
        }

        let mut child = cmd.spawn().map_err(SpawnError::Io)?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| SpawnError::Failed("stdin unavailable".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| SpawnError::Failed("stdout unavailable".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| SpawnError::Failed("stderr unavailable".into()))?;
        let (tx, rx) = mpsc::channel::<String>(512);
        spawn_reader(stdout, tx.clone());
        spawn_reader(stderr, tx);

        Ok(StdHandle {
            stdin: Arc::new(Mutex::new(stdin)),
            child: Arc::new(Mutex::new(child)),
            stdout_rx: Some(rx),
        })
    }
}

fn spawn_reader<R>(reader: R, tx: mpsc::Sender<String>)
where
    R: std::io::Read + Send + 'static,
{
    std::thread::spawn(move || {
        use std::io::BufRead;
        let lines = std::io::BufReader::new(reader).lines();
        for line in lines {
            let Ok(line) = line else {
                break;
            };
            if tx.blocking_send(line).is_err() {
                break;
            }
        }
    });
}
