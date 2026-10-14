//! Reaching Core and its servers from outside: TLS for the API, UPnP port mappings, and the
//! reachability diagnosis.

pub mod diagnosis;
pub mod firewall;
pub mod router;
pub mod tls;

use std::net::IpAddr;

/// Loopback, private network, and link-local addresses: not reachable from the internet.
#[allow(dead_code)] // Wired in step 8: reachability reporting.
pub fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback() || v4.is_private() || v4.is_link_local()
        }
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_private(IpAddr::V4(v4)),
            None => {
                v6.is_loopback()
                    || (v6.segments()[0] & 0xfe00) == 0xfc00
                    || (v6.segments()[0] & 0xffc0) == 0xfe80
            }
        },
    }
}
