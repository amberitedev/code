use crate::state::Desktop;
use std::sync::Arc;

use crate::accounts::state::ModrinthCredentials;
use crate::discord::DiscordGuard;
use crate::friends::state::FriendsSocket;

/// Long-running social integrations.
pub struct Presence {
    pub discord_rpc: DiscordGuard,
    pub friends_socket: FriendsSocket,
}

impl Presence {
    pub(crate) fn init() -> engine::Result<Self> {
        Ok(Self {
            discord_rpc: DiscordGuard::init()?,
            friends_socket: FriendsSocket::new(),
        })
    }
}

/// Refreshes Modrinth sign-ins, connects Discord and the friends socket, then keeps the socket
/// connected. The friends socket waits for the refresh, which may replace its session.
pub async fn start_presence(engine: Desktop, presence: Arc<Presence>) {
    let res = tokio::try_join!(
        presence.discord_rpc.clear_to_default(
            true,
            engine.db(),
            &engine.process_manager,
        ),
        ModrinthCredentials::refresh_all(
            engine.db(),
            engine.pool(),
            &engine.fetch_semaphore,
            &engine.sessions,
        ),
    );

    if let Err(e) = res {
        tracing::error!("Error running discord RPC: {e}");
    }

    let _ = presence
        .friends_socket
        .connect(
            engine.db(),
            &engine.api_semaphore,
            &engine.sessions,
            &engine.process_manager,
            engine.app_events(),
        )
        .await;
    let _ = FriendsSocket::socket_loop(engine, presence).await;
}

/// Shows the first running instance, or none, on Discord and to friends.
pub async fn show_presence(engine: &Desktop, presence: &Presence) {
    let _ = presence
        .discord_rpc
        .clear_to_default(true, engine.db(), &engine.process_manager)
        .await;
    let playing = engine
        .process_manager
        .get_all()
        .into_iter()
        .next()
        .map(|process| process.instance_name);
    let _ = presence.friends_socket.update_status(playing).await;
}
