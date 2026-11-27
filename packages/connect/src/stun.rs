//! Just enough STUN (RFC 5389) to ask a public server which address a socket's packets come from.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

/// Answer on the standard STUN port and take no account.
pub const SERVERS: [&str; 2] =
    ["stun.cloudflare.com:3478", "stun.l.google.com:19302"];
const MAGIC: u32 = 0x2112_A442;

/// Names one question, so its answer is told from any other packet.
pub type Transaction = [u8; 12];

pub fn transaction() -> Transaction {
    rand::random()
}

/// A binding request: type, zero length, magic cookie, transaction id.
pub fn request(id: &Transaction) -> [u8; 20] {
    let mut message = [0; 20];
    message[..2].copy_from_slice(&0x0001u16.to_be_bytes());
    message[4..8].copy_from_slice(&MAGIC.to_be_bytes());
    message[8..].copy_from_slice(id);
    message
}

/// The transaction a binding success answers, and the IPv4 address in its XOR-MAPPED-ADDRESS.
pub fn mapped_address(reply: &[u8]) -> Option<(Transaction, SocketAddr)> {
    let u16_at = |at: usize| {
        Some(u16::from_be_bytes(reply.get(at..at + 2)?.try_into().ok()?))
    };
    if u16_at(0)? != 0x0101 || reply.get(4..8)? != MAGIC.to_be_bytes() {
        return None;
    }
    let id = reply.get(8..20)?.try_into().ok()?;
    let end = (20 + usize::from(u16_at(2)?)).min(reply.len());
    let mut at = 20;
    while at + 4 <= end {
        let (kind, len) = (u16_at(at)?, usize::from(u16_at(at + 2)?));
        let value = reply.get(at + 4..at + 4 + len)?;
        // IPv4 family: the port and address XORed with the magic cookie.
        if kind == 0x0020 && len == 8 && value[1] == 0x01 {
            let port =
                u16::from_be_bytes([value[2], value[3]]) ^ (MAGIC >> 16) as u16;
            let ip = u32::from_be_bytes(value[4..8].try_into().ok()?) ^ MAGIC;
            let address = SocketAddrV4::new(Ipv4Addr::from(ip), port);
            return Some((id, address.into()));
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
        let port = 40_000 ^ (MAGIC >> 16) as u16;
        let mut reply = vec![0x01, 0x01, 0, 20];
        reply.extend(MAGIC.to_be_bytes());
        reply.extend(id);
        // An attribute nothing here reads, padded to four bytes.
        reply.extend([0x80, 0x22, 0, 3, b'a', b'b', b'c', 0]);
        reply.extend([0x00, 0x20, 0, 8, 0, 0x01]);
        reply.extend(port.to_be_bytes());
        reply.extend(ip.to_be_bytes());
        assert_eq!(
            mapped_address(&reply),
            Some((id, "203.0.113.5:40000".parse().unwrap()))
        );
        assert_eq!(request(&id)[..2], [0, 1]);
        assert_eq!(mapped_address(&request(&id)), None, "a request");
    }
}
