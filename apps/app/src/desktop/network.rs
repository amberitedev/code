use axum::Router;
use contracts::{
    ApiError, Json,
    server::{Mount, Req},
};

use super::internal;
use crate::local_api::endpoints::desktop::CheckNetworkMetered;

pub fn routes() -> Router {
    Router::new().endpoint::<CheckNetworkMetered, _, _>(is_metered)
}

async fn is_metered(
    _: Req<CheckNetworkMetered>,
) -> Result<Json<bool>, ApiError> {
    is_network_metered().await.map(Json).map_err(internal)
}

#[cfg(windows)]
async fn is_network_metered() -> anyhow::Result<bool> {
    use windows::Networking::Connectivity::{
        NetworkCostType, NetworkInformation,
    };

    let cost_type = NetworkInformation::GetInternetConnectionProfile()?
        .GetConnectionCost()?
        .NetworkCostType()?;
    Ok(matches!(
        cost_type,
        NetworkCostType::Fixed | NetworkCostType::Variable
    ))
}

#[cfg(target_os = "macos")]
async fn is_network_metered() -> anyhow::Result<bool> {
    use anyhow::Context;
    use cidre::dispatch::Queue;
    use cidre::nw::PathMonitor;
    use std::time::Duration;
    use tokio::sync::mpsc;

    let (sender, mut receiver) = mpsc::channel(1);

    let queue = Queue::new();
    let mut monitor = PathMonitor::new();
    monitor.set_queue(&queue);
    monitor.set_update_handler(move |path| {
        let _ = sender.try_send(path.is_constrained() || path.is_expensive());
    });

    monitor.start();
    let result =
        tokio::time::timeout(Duration::from_millis(100), receiver.recv())
            .await
            .ok()
            .flatten();
    monitor.cancel();

    result.context("NWPathMonitor did not provide a network path in time")
}

#[cfg(target_os = "linux")]
async fn is_network_metered() -> anyhow::Result<bool> {
    // Thanks to https://github.com/Hakanbaban53/rclone-manager for showing how to do this
    use zbus::{Connection, Proxy};

    let connection = Connection::system().await?;
    let proxy = Proxy::new(
        &connection,
        "org.freedesktop.NetworkManager",
        "/org/freedesktop/NetworkManager",
        "org.freedesktop.NetworkManager",
    )
    .await?;
    let metered = proxy.get_property("Metered").await?;
    Ok(matches!(metered, 1 | 3))
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
async fn is_network_metered() -> anyhow::Result<bool> {
    tracing::warn!(
        "is_network_metered called on unsupported platform. Assuming unmetered."
    );
    Ok(false)
}
