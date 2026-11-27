//! This Core's address on the internet. Asked of the home router first (UPnP and NAT-PMP have a
//! call for it), then of a public STUN server. Never of the backend.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use connect::stun;
use tokio::net::UdpSocket;

use super::{is_public, router::Router};

const STUN_TIMEOUT: Duration = Duration::from_secs(3);

/// This machine's address on its own network: the one it sends internet traffic from.
pub fn local_ipv4() -> Option<Ipv4Addr> {
    netdev::get_default_interface()
        .ok()?
        .ipv4
        .first()
        .map(|net| net.addr())
}

/// The public address, `None` when neither the router nor STUN gives one.
pub async fn find(local: Option<Ipv4Addr>) -> Option<IpAddr> {
    if let Some(local) = local
        && let Some(router) = Router::discover(local).await
        && let Some(ip) = router.wan_ip().await
        && is_public(ip)
    {
        return Some(ip);
    }
    for server in stun::SERVERS {
        if let Some(ip) = stun(server).await.filter(|ip| is_public(*ip)) {
            return Some(ip);
        }
    }
    None
}

/// Ask a STUN server which address this machine's IPv4 traffic comes from.
async fn stun(server: &str) -> Option<IpAddr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await.ok()?;
    let server = tokio::net::lookup_host(server)
        .await
        .ok()?
        .find(SocketAddr::is_ipv4)?;
    let id = stun::transaction();
    socket.send_to(&stun::request(&id), server).await.ok()?;
    let mut reply = [0; 512];
    let (len, from) =
        tokio::time::timeout(STUN_TIMEOUT, socket.recv_from(&mut reply))
            .await
            .ok()?
            .ok()?;
    (from == server)
        .then(|| stun::mapped_address(&reply[..len]))
        .flatten()
        .filter(|(answered, _)| *answered == id)
        .map(|(_, address)| address.ip())
}
