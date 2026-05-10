use tokio::sync::broadcast;

use crate::domain::event::Event;

const CHANNEL_CAPACITY: usize = 512;

/// Thin wrapper around a broadcast channel for instance events.
#[derive(Clone)]
pub struct EventBroadcaster {
    sender: broadcast::Sender<Event>,
}

impl EventBroadcaster {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self { sender }
    }

    /// Broadcast an event to all current subscribers. Errors are ignored
    /// (no subscribers is normal when no WebSocket is open).
    pub fn send(&self, event: Event) {
        let _ = self.sender.send(event);
    }

    /// Subscribe to future events.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }
}

impl Default for EventBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}
