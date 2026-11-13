use std::sync::Arc;

use super::{DiscordGuard, FriendsSocket, ModrinthCredentials};
use crate::Engine;

/// Long-running social integrations.
pub struct Presence {
    pub discord_rpc: DiscordGuard,
    pub friends_socket: FriendsSocket,
}

impl Presence {
    pub(crate) fn init() -> crate::Result<Self> {
        Ok(Self {
            discord_rpc: DiscordGuard::init()?,
            friends_socket: FriendsSocket::new(),
        })
    }
}

impl Engine {
    /// Refreshes Modrinth sign-ins, connects Discord and the friends socket, then keeps the socket
    /// connected. The friends socket waits for the refresh, which may replace its session.
    pub async fn start_presence(self: Arc<Self>) {
        let res = tokio::try_join!(
            self.presence.discord_rpc.clear_to_default(
                true,
                &self.pool,
                &self.process_manager,
            ),
            ModrinthCredentials::refresh_all(
                &self.pool,
                &self.fetch_semaphore,
                &self.sessions,
            ),
        );

        if let Err(e) = res {
            tracing::error!("Error running discord RPC: {e}");
        }

        let _ = self
            .presence
            .friends_socket
            .connect(
                &self.pool,
                &self.api_semaphore,
                &self.sessions,
                &self.process_manager,
                &self.events,
            )
            .await;
        let _ = FriendsSocket::socket_loop(self).await;
    }

    /// Shows the first running instance, or none, on Discord and to friends.
    pub async fn show_presence(&self) {
        let _ = self
            .presence
            .discord_rpc
            .clear_to_default(true, &self.pool, &self.process_manager)
            .await;
        let playing = self
            .process_manager
            .get_all()
            .into_iter()
            .next()
            .map(|process| process.instance_name);
        let _ = self.presence.friends_socket.update_status(playing).await;
    }
}
