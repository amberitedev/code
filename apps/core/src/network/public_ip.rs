//! This Core's address on the internet. Asked of the home router first (UPnP and NAT-PMP have a
//! call for it), then of a public STUN server. Never of the backend.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use tokio::net::UdpSocket;

use super::{is_private, router::Router};

const STUN_SERVERS: [&str; 2] =
    ["stun.cloudflare.com:3478", "stun.l.google.com:19302"];
const STUN_TIMEOUT: Duration = Duration::from_secs(3);
/// RFC 5389.
const MAGIC: u32 = 0x2112_A442;

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
    for server in STUN_SERVERS {
        if let Some(ip) = stun(server).await.filter(|ip| is_public(*ip)) {
            return Some(ip);
        }
    }
    None
}

/// Not private, and not the shared range carriers use behind CGNAT (100.64.0.0/10).
fn is_public(ip: IpAddr) -> bool {
    let shared = matches!(ip, IpAddr::V4(v4) if v4.octets()[0] == 100 && (v4.octets()[1] & 0xc0) == 64);
    !is_private(ip) && !shared && !ip.is_unspecified()
}

/// Ask a STUN server which address this machine's IPv4 traffic comes from.
async fn stun(server: &str) -> Option<IpAddr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await.ok()?;
    let server = tokio::net::lookup_host(server)
        .await
        .ok()?
        .find(SocketAddr::is_ipv4)?;
    let id: [u8; 12] = rand_id();
    socket.send_to(&request(&id), server).await.ok()?;
    let mut reply = [0; 512];
    let (len, from) =
        tokio::time::timeout(STUN_TIMEOUT, socket.recv_from(&mut reply))
            .await
            .ok()?
            .ok()?;
    (from == server)
        .then(|| mapped_address(&reply[..len], &id))
        .flatten()
}

fn rand_id() -> [u8; 12] {
    let mut id = [0; 12];
    id.copy_from_slice(&uuid::Uuid::new_v4().as_bytes()[..12]);
    id
}

/// A binding request: type, zero length, magic cookie, transaction id.
fn request(id: &[u8; 12]) -> [u8; 20] {
    let mut message = [0; 20];
    message[..2].copy_from_slice(&0x0001u16.to_be_bytes());
    message[4..8].copy_from_slice(&MAGIC.to_be_bytes());
    message[8..].copy_from_slice(id);
    message
}

/// The XOR-MAPPED-ADDRESS of a binding success for transaction `id`.
fn mapped_address(reply: &[u8], id: &[u8; 12]) -> Option<IpAddr> {
    let u16_at = |at: usize| {
        Some(u16::from_be_bytes(reply.get(at..at + 2)?.try_into().ok()?))
    };
    if u16_at(0)? != 0x0101
        || reply.get(4..8)? != MAGIC.to_be_bytes()
        || reply.get(8..20)? != id
    {
        return None;
    }
    let end = (20 + usize::from(u16_at(2)?)).min(reply.len());
    let mut at = 20;
    while at + 4 <= end {
        let (kind, len) = (u16_at(at)?, usize::from(u16_at(at + 2)?));
        let value = reply.get(at + 4..at + 4 + len)?;
        // IPv4 family: the address XORed with the magic cookie.
        if kind == 0x0020 && len == 8 && value[1] == 0x01 {
            let xored = u32::from_be_bytes(value[4..8].try_into().ok()?);
            return Some(IpAddr::V4(Ipv4Addr::from(xored ^ MAGIC)));
        }
        at += 4 + len.div_ceil(4) * 4;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_binding_success_gives_the_mapped_address() {
        let id = [9; 12];
        let ip = u32::from(Ipv4Addr::new(203, 0, 113, 5)) ^ MAGIC;
        let mut reply = vec![0x01, 0x01, 0, 20];
        reply.extend(MAGIC.to_be_bytes());
        reply.extend(id);
        // An attribute this Core does not read, padded to four bytes.
        reply.extend([0x80, 0x22, 0, 3, b'a', b'b', b'c', 0]);
        reply.extend([0x00, 0x20, 0, 8, 0, 0x01, 0x12, 0x34]);
        reply.extend(ip.to_be_bytes());
        assert_eq!(
            mapped_address(&reply, &id),
            Some(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 5)))
        );
        assert_eq!(
            mapped_address(&reply, &[8; 12]),
            None,
            "another transaction"
        );
        assert_eq!(request(&id)[..2], [0, 1]);
    }

    #[test]
    fn private_and_carrier_addresses_are_not_public() {
        assert!(is_public("203.0.113.5".parse().unwrap()));
        assert!(!is_public("192.168.1.2".parse().unwrap()));
        assert!(!is_public("100.72.0.1".parse().unwrap()));
        assert!(is_public("100.128.0.1".parse().unwrap()));
    }
}
