use std::{fmt, net::SocketAddr};

/// Where a Core serves its API: `https://ip:port`, as Copal reports it. Nothing else is a Core
/// address: no host name, no default port, no user, path or query, so a ticket or session never
/// leaves over plain HTTP or to a name someone else resolves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoreAddress(SocketAddr);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AddressError {
    #[error("a Core address must be https://ip:port")]
    Invalid,
}

impl CoreAddress {
    /// `address` as `https://ip:port`, with or without a trailing `/`.
    pub fn parse(address: &str) -> Result<Self, AddressError> {
        let socket = address
            .strip_prefix("https://")
            .map(|rest| rest.strip_suffix('/').unwrap_or(rest))
            .and_then(|rest| rest.parse::<SocketAddr>().ok())
            .ok_or(AddressError::Invalid)?;
        Ok(Self(socket))
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
        ] {
            assert!(CoreAddress::parse(address).is_ok(), "{address}");
        }
        for address in [
            "http://10.0.0.11:17007",
            "https://core.example:17007",
            "https://localhost:17007",
            "https://10.0.0.11",
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
    }
}
