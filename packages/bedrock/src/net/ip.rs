//! Which IP addresses are on the public internet. One rule for the store's pull sources, the
//! hosts Copal downloads from, and Copal's own public address.

use std::net::IpAddr;

/// Whether `ip` is on the public internet: not loopback, private, link-local, shared (CGNAT),
/// multicast, documentation, or otherwise reserved.
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, c, _] = v4.octets();
            !(v4.is_unspecified()
                || v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_multicast()
                || is_shared_range(ip)
                || a == 0
                // 192.0.0.0/24, protocol assignments.
                || (a == 192 && b == 0 && c == 0)
                // 198.18.0.0/15, benchmarking.
                || (a == 198 && (18..20).contains(&b))
                // 240.0.0.0/4, reserved.
                || a >= 240)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public(IpAddr::V4(v4));
            }
            let [first, second, ..] = v6.segments();
            !(v6.is_unspecified()
                || v6.is_loopback()
                || v6.is_multicast()
                // fc00::/7, unique local.
                || (first & 0xfe00) == 0xfc00
                // fe80::/10, link-local.
                || (first & 0xffc0) == 0xfe80
                // 2001:db8::/32, documentation.
                || (first == 0x2001 && second == 0xdb8)
                // 64:ff9b::/96 and the like embed IPv4 addresses that are checked nowhere else.
                || first == 0x64
                // 2002::/16, 6to4.
                || first == 0x2002)
        }
    }
}

/// Loopback, private network, and link-local addresses: this computer or its network. Copal's
/// reachability diagnosis tells these apart from the shared range.
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

/// 100.64.0.0/10, the range internet providers use when customers share an address (CGNAT).
pub fn is_shared_range(ip: IpAddr) -> bool {
    matches!(ip, IpAddr::V4(v4) if v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_and_reserved_addresses_are_not_public() {
        for address in [
            "127.0.0.1",
            "10.0.0.8",
            "172.16.4.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "100.72.0.1",
            "0.0.0.0",
            "224.0.0.1",
            "203.0.113.5",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:192.168.1.1",
            "64:ff9b::a00:1",
        ] {
            assert!(!is_public(address.parse().unwrap()), "{address}");
        }
        for address in
            ["1.1.1.1", "104.16.0.1", "100.128.0.1", "2606:4700::1111"]
        {
            assert!(is_public(address.parse().unwrap()), "{address}");
        }
    }

    #[test]
    fn the_shared_range_is_neither_private_nor_public() {
        let shared = "100.72.0.1".parse().unwrap();
        assert!(is_shared_range(shared));
        assert!(!is_private(shared));
        assert!(!is_public(shared));
        assert!(is_private("::ffff:10.0.0.1".parse().unwrap()));
    }
}
