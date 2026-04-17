//! Instance actor - Actor model for zero-lock instance orchestration.
//! Each server runs in its own Tokio task.

use std::sync::Arc;
use tokio::sync::{mpsc, broadcast};
use crate::domain::instances::InstanceId;
use crate::domain::ports::OSProcessManager;
use crate::domain::instances::{ProcessHandle};

/// Commands sent to instance actor.
#[derive(Debug)]
pub enum InstanceCommand {
    Start,
    Stop { graceful: bool },
    Kill,
    SendCommand(String),
}

/// Events emitted by instance actor.
#[derive(Debug, Clone)]
pub enum InstanceEvent {
    Started,
    Stopped { exit_code: i32 },
    Error(String),
    ConsoleLine(String),
}

/// Instance actor for a single server.
pub struct InstanceActor {
    instance_id: InstanceId,
    cmd_rx: mpsc::Receiver<InstanceCommand>,
    event_tx: broadcast::Sender<InstanceEvent>,
    os_manager: Arc<dyn OSProcessManager>,
    process: Option<Box<dyn ProcessHandle>>,
}

impl InstanceActor {
    pub fn new(
        instance_id: InstanceId,
        cmd_rx: mpsc::Receiver<InstanceCommand>,
        event_tx: broadcast::Sender<InstanceEvent>,
        os_manager: Arc<dyn OSProcessManager>,
    ) -> Self {
        InstanceActor {
            instance_id,
            cmd_rx,
            event_tx,
            os_manager,
            process: None,
        }
    }

    pub async fn run(mut self) {
        loop {
            tokio::select! {
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        Some(InstanceCommand::Start) => self.handle_start().await,
                        Some(InstanceCommand::Stop { graceful }) => self.handle_stop(graceful).await,
                        Some(InstanceCommand::Kill) => self.handle_kill().await,
                        Some(InstanceCommand::SendCommand(cmd)) => self.handle_command(&cmd).await,
                        None => break,
                    }
                }
            }
        }
    }

    async fn handle_start(&mut self) {
        if self.process.is_some() {
            let _ = self.event_tx.send(InstanceEvent::Error("Already running".into()));
            return;
        }

        match self.os_manager.spawn("java -jar server.jar -nogui").await {
            Ok(handle) => {
                // Create channel for console output
                let (output_tx, mut output_rx) = mpsc::channel(100);
                
                // Start streaming PTY output
                if let Err(e) = handle.start_output_stream(output_tx.clone()) {
                    tracing::warn!("Failed to start PTY output stream: {}", e);
                }
                
                self.process = Some(handle);
                
                // Spawn output reader task
                let event_tx = self.event_tx.clone();
                tokio::spawn(async move {
                    while let Some(line) = output_rx.recv().await {
                        let _ = event_tx.send(InstanceEvent::ConsoleLine(line));
                    }
                });
                
                let _ = self.event_tx.send(InstanceEvent::Started);
            }
            Err(e) => {
                let _ = self.event_tx.send(InstanceEvent::Error(e.to_string()));
            }
        }
    }

    async fn handle_stop(&mut self, _graceful: bool) {
        if let Some(_) = self.process.take() {
            let _ = self.event_tx.send(InstanceEvent::Stopped { exit_code: 0 });
        }
    }

    async fn handle_kill(&mut self) {
        self.process = None;
        let _ = self.event_tx.send(InstanceEvent::Stopped { exit_code: -1 });
    }

    async fn handle_command(&mut self, cmd: &str) {
        if let Some(ref process) = self.process {
            let _ = process.send_input(cmd);
        }
    }
}
