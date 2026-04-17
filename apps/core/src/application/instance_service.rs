//! Instance service - Bridge between HTTP handlers and isolated actors.
//! Uses DashMap for concurrent access without lock contention.

use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::{mpsc, broadcast};
use thiserror::Error;
use crate::domain::instances::{InstanceId};
use crate::domain::ports::{InstanceRepository, OSProcessManager};
use crate::application::instance_actor::{InstanceActor, InstanceCommand, InstanceEvent};

/// Instance service error.
#[derive(Error, Debug)]
pub enum InstanceServiceError {
    #[error("Instance not found: {0}")]
    NotFound(InstanceId),
    #[error("Instance already running")]
    AlreadyRunning,
    #[error("Instance not running")]
    NotRunning,
}

/// Instance service managing all server instances.
pub struct InstanceService {
    active_actors: DashMap<InstanceId, mpsc::Sender<InstanceCommand>>,
    event_channels: DashMap<InstanceId, broadcast::Sender<InstanceEvent>>,
    instance_repo: Arc<dyn InstanceRepository>,
    os_manager: Arc<dyn OSProcessManager>,
}

impl InstanceService {
    pub fn new(
        instance_repo: Arc<dyn InstanceRepository>,
        os_manager: Arc<dyn OSProcessManager>,
    ) -> Self {
        InstanceService {
            active_actors: DashMap::new(),
            event_channels: DashMap::new(),
            instance_repo,
            os_manager,
        }
    }

    pub async fn start_instance(&self, id: InstanceId) -> Result<(), InstanceServiceError> {
        if self.active_actors.contains_key(&id) {
            return Err(InstanceServiceError::AlreadyRunning);
        }

        let (cmd_tx, cmd_rx) = mpsc::channel(128);
        let (event_tx, _event_rx) = broadcast::channel(256);

        // Store event channel for WebSocket subscriptions
        self.event_channels.insert(id, event_tx.clone());

        let actor = InstanceActor::new(
            id,
            cmd_rx,
            event_tx,
            self.os_manager.clone(),
        );

        tokio::spawn(actor.run());
        self.active_actors.insert(id, cmd_tx.clone());

        let _ = cmd_tx.send(InstanceCommand::Start).await;
        Ok(())
    }

    pub async fn stop_instance(&self, id: InstanceId, graceful: bool) -> Result<(), InstanceServiceError> {
        let actor = self.active_actors.get(&id)
            .ok_or(InstanceServiceError::NotFound(id))?;

        let _ = actor.send(InstanceCommand::Stop { graceful }).await;
        
        // Clean up event channel
        self.event_channels.remove(&id);
        
        Ok(())
    }

    pub async fn kill_instance(&self, id: InstanceId) -> Result<(), InstanceServiceError> {
        let actor = self.active_actors.get(&id)
            .ok_or(InstanceServiceError::NotFound(id))?;

        let _ = actor.send(InstanceCommand::Kill).await;
        
        // Clean up event channel
        self.event_channels.remove(&id);
        
        Ok(())
    }

    pub async fn send_command(&self, id: InstanceId, cmd: &str) -> Result<(), InstanceServiceError> {
        let actor = self.active_actors.get(&id)
            .ok_or(InstanceServiceError::NotFound(id))?;

        let _ = actor.send(InstanceCommand::SendCommand(cmd.to_string())).await;
        Ok(())
    }

    pub fn subscribe_to_events(&self, id: InstanceId) -> Option<broadcast::Receiver<InstanceEvent>> {
        self.event_channels.get(&id).map(|tx| tx.subscribe())
    }

    pub fn is_running(&self, id: InstanceId) -> bool {
        self.active_actors.contains_key(&id)
    }

    pub fn running_count(&self) -> usize {
        self.active_actors.len()
    }
}
