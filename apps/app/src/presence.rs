use std::sync::Arc;

use engine::Engine;

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
pub async fn start_presence(engine: Arc<Engine>, presence: Arc<Presence>) {
    let res = tokio::try_join!(
        presence.discord_rpc.clear_to_default(
            true,
            crate::db::pool(&engine),
            &engine.process_manager,
        ),
        ModrinthCredentials::refresh_all(
            crate::db::pool(&engine),
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
            crate::db::pool(&engine),
            &engine.api_semaphore,
            &engine.sessions,
            &engine.process_manager,
            &engine.events,
        )
        .await;
    let _ = FriendsSocket::socket_loop(engine, presence).await;
}

/// Shows the first running instance, or none, on Discord and to friends.
pub async fn show_presence(engine: &Engine, presence: &Presence) {
    let _ = presence
        .discord_rpc
        .clear_to_default(
            true,
            crate::db::pool(engine),
            &engine.process_manager,
        )
        .await;
    let playing = engine
        .process_manager
        .get_all()
        .into_iter()
        .next()
        .map(|process| process.instance_name);
    let _ = presence.friends_socket.update_status(playing).await;
}
