use std::{
    fmt,
    net::{IpAddr, SocketAddr},
};

/// Where a Core serves its API: `https://ip:port`, as Copal reports it. Nothing else is a Core
/// address: no host name, user, path or query, so a ticket or session never leaves over plain
/// HTTP or to a name someone else resolves. A missing port is 443, because the backend stores a
/// port-443 address without it (`URL.origin` drops the default port).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoreAddress(SocketAddr);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AddressError {
    #[error("a Core address must be https://ip:port")]
    Invalid,
}

impl CoreAddress {
    /// `address` as `https://ip:port` or `https://ip`, with or without a trailing `/`.
    pub fn parse(address: &str) -> Result<Self, AddressError> {
        let rest = address
            .strip_prefix("https://")
            .ok_or(AddressError::Invalid)?;
        let rest = rest.strip_suffix('/').unwrap_or(rest);
        let socket = rest.parse::<SocketAddr>().ok().or_else(|| {
            let ip = rest
                .strip_prefix('[')
                .and_then(|ip| ip.strip_suffix(']'))
                .unwrap_or(rest);
            // A bare IPv6 address would read its last group as a port, so only the bracketed
            // form is accepted for IPv6.
            match ip.parse::<IpAddr>().ok()? {
                IpAddr::V6(_) if ip.len() == rest.len() => None,
                ip => Some(SocketAddr::new(ip, 443)),
            }
        });
        socket.map(Self).ok_or(AddressError::Invalid)
    }

    pub fn socket(self) -> SocketAddr {
        self.0
    }

    /// `ip:port`, with an IPv6 address in brackets, as a `Host` header names it.
    pub fn authority(self) -> String {
        self.0.to_string()
    }
}

impl fmt::Display for CoreAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "https://{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_ip_port_is_a_core_address() {
        for address in [
            "https://10.0.0.11:17007",
            "https://10.0.0.11:17007/",
            "https://[::1]:17007",
            "https://8.8.8.8:443",
            "https://1.2.3.4",
            "https://1.2.3.4/",
            "https://[::1]",
        ] {
            assert!(CoreAddress::parse(address).is_ok(), "{address}");
        }
        for address in [
            "http://10.0.0.11:17007",
            "https://core.example:17007",
            "https://localhost:17007",
            "https://::1",
            "https://user@10.0.0.11:17007",
            "https://10.0.0.11:17007/path",
            "https://10.0.0.11:17007?query",
            "10.0.0.11:17007",
        ] {
            assert!(CoreAddress::parse(address).is_err(), "{address}");
        }
    }

    #[test]
    fn an_address_prints_as_it_parses() {
        let address = CoreAddress::parse("https://[::1]:17007/").unwrap();
        assert_eq!(address.to_string(), "https://[::1]:17007");
        assert_eq!(address.authority(), "[::1]:17007");
        let address = CoreAddress::parse("https://1.2.3.4").unwrap();
        assert_eq!(address.to_string(), "https://1.2.3.4:443");
    }
}
