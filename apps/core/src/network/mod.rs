//! Reaching Core and its servers from outside: TLS for the API, port mappings that belong to
//! servers (`forward`), and the reachability diagnosis.

pub mod diagnosis;
pub mod firewall;
pub mod forward;
pub mod public_ip;
pub mod router;
pub mod tls;

/// Where every reqwest client in Core starts. reqwest is built without a TLS provider so Core
/// stays on ring, and it panics without one installed; installing it here rather than only in
/// `main` covers tests too.
pub fn http() -> reqwest::ClientBuilder {
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
}

use std::net::IpAddr;

/// Loopback, private network, and link-local addresses: not reachable from the internet.
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
