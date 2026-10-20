//! Pack content is downloaded only from the hosts Modrinth allows in a pack. A pack can name any
//! URL, so the client checks the host of the first URL and of every redirect, resolves the name
//! itself, refuses private addresses, and connects to exactly the addresses it checked: a name
//! that resolves to the LAN, or changes its answer between check and connect, gets nowhere.

use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use reqwest::{
    Url,
    dns::{Addrs, Name, Resolve, Resolving},
    redirect,
};

use crate::installs::download;

/// Where a pack's files may come from, as Modrinth's pack format allows.
const HOSTS: [&str; 4] = [
    "cdn.modrinth.com",
    "github.com",
    "raw.githubusercontent.com",
    "gitlab.com",
];

/// github.com serves every release download from these, so they are allowed after a redirect
/// from an allowed host, never as the URL a pack names.
const REDIRECT_HOSTS: [&str; 2] = [
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];

const MAX_REDIRECTS: usize = 5;

/// Whether a pack may download from `url`: HTTPS on the default port, no credentials, and a
/// host on the list. `redirected` also allows the hosts only redirects may reach.
pub fn allowed(url: &Url, redirected: bool) -> bool {
    url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(url.host(), Some(url::Host::Domain(host)) if host_allowed(host, redirected))
}

fn host_allowed(host: &str, redirected: bool) -> bool {
    let host = host.trim_end_matches('.');
    HOSTS
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(host))
        || (redirected
            && REDIRECT_HOSTS
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(host)))
}

/// Whether a redirect to `next` may be followed, after `previous` redirects.
fn redirect_allowed(next: &Url, previous: usize) -> Result<(), String> {
    if previous >= MAX_REDIRECTS {
        return Err("too many redirects".into());
    }
    if !allowed(next, true) {
        return Err(format!(
            "a redirect to {} is not allowed for pack content",
            next.host_str().unwrap_or("an address")
        ));
    }
    Ok(())
}

/// Whether `ip` is on the public internet: not loopback, private, link-local, shared (CGNAT),
/// multicast, documentation, or otherwise reserved.
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, ..] = ip.octets();
            !(ip.is_unspecified()
                || ip.is_loopback()
                || ip.is_private()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_multicast()
                || a == 0
                // 100.64.0.0/10, shared address space.
                || (a == 100 && (64..128).contains(&b))
                // 192.0.0.0/24, protocol assignments.
                || (a == 192 && b == 0 && ip.octets()[2] == 0)
                // 198.18.0.0/15, benchmarking.
                || (a == 198 && (18..20).contains(&b))
                // 240.0.0.0/4, reserved.
                || a >= 240)
        }
        IpAddr::V6(ip) => {
            if let Some(v4) = ip.to_ipv4_mapped() {
                return is_public(IpAddr::V4(v4));
            }
            let first = ip.segments()[0];
            !(ip.is_unspecified()
                || ip.is_loopback()
                || ip.is_multicast()
                // fc00::/7, unique local.
                || (first & 0xfe00) == 0xfc00
                // fe80::/10, link-local.
                || (first & 0xffc0) == 0xfe80
                // 2001:db8::/32, documentation.
                || (first == 0x2001 && ip.segments()[1] == 0xdb8)
                // 64:ff9b::/96 and the like embed IPv4 addresses that are checked nowhere else.
                || first == 0x64
                // 2002::/16, 6to4.
                || first == 0x2002)
        }
    }
}

/// Keep the public addresses a name resolved to; refuse the name when none is left.
fn public_only(
    name: &str,
    addresses: impl Iterator<Item = SocketAddr>,
) -> Result<Vec<SocketAddr>, String> {
    let public: Vec<SocketAddr> = addresses
        .filter(|address| is_public(address.ip()))
        .collect();
    if public.is_empty() {
        return Err(format!("{name} does not resolve to a public address"));
    }
    Ok(public)
}

/// Resolves only allowed names, and hands the client only their public addresses, so it
/// connects to what was checked.
struct PublicResolver;

impl Resolve for PublicResolver {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let name = name.as_str().to_owned();
            if !host_allowed(&name, true) {
                return Err(format!("{name} is not an allowed host").into());
            }
            let resolved =
                tokio::net::lookup_host((name.as_str(), 443)).await?;
            let addresses = public_only(&name, resolved)?;
            let addrs: Addrs = Box::new(addresses.into_iter());
            Ok(addrs)
        })
    }
}

/// The client for pack content. Every URL still goes through [`allowed`] first, because the
/// resolver never sees a URL that names an IP address.
pub fn client() -> reqwest::Result<reqwest::Client> {
    download::builder()
        .https_only(true)
        .dns_resolver(Arc::new(PublicResolver))
        .redirect(redirect::Policy::custom(|attempt| {
            match redirect_allowed(attempt.url(), attempt.previous().len()) {
                Ok(()) => attempt.follow(),
                Err(reason) => attempt.error(reason),
            }
        }))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    #[test]
    fn only_listed_hosts_over_https_are_allowed() {
        assert!(allowed(
            &url("https://cdn.modrinth.com/data/a/b.jar"),
            false
        ));
        assert!(allowed(
            &url("https://GitHub.com/a/b/releases/x.jar"),
            false
        ));
        assert!(allowed(&url("https://raw.githubusercontent.com/a"), false));
        assert!(allowed(&url("https://gitlab.com/a"), false));
        for refused in [
            "http://cdn.modrinth.com/a.jar",
            "https://cdn.modrinth.com:8443/a.jar",
            "https://user:pass@cdn.modrinth.com/a.jar",
            "https://cdn.modrinth.com.evil.example/a.jar",
            "https://evil.example/cdn.modrinth.com/a.jar",
            "https://modrinth.com/a.jar",
            "https://127.0.0.1/a.jar",
            "https://[::1]/a.jar",
            "file:///etc/passwd",
        ] {
            assert!(!allowed(&url(refused), false), "{refused}");
        }
        // GitHub's download hosts only after a redirect.
        let assets = url("https://objects.githubusercontent.com/a");
        assert!(!allowed(&assets, false));
        assert!(allowed(&assets, true));
    }

    #[test]
    fn every_redirect_is_checked_again() {
        assert!(
            redirect_allowed(
                &url("https://release-assets.githubusercontent.com/x"),
                0
            )
            .is_ok()
        );
        assert!(
            redirect_allowed(&url("https://cdn.modrinth.com/x"), 1).is_ok()
        );
        assert!(
            redirect_allowed(&url("https://192.168.1.1/admin"), 0).is_err()
        );
        assert!(
            redirect_allowed(&url("http://cdn.modrinth.com/x"), 0).is_err()
        );
        assert!(redirect_allowed(&url("https://evil.example/x"), 0).is_err());
        assert!(
            redirect_allowed(&url("https://cdn.modrinth.com/x"), MAX_REDIRECTS)
                .is_err()
        );
    }

    #[test]
    fn private_addresses_are_refused() {
        for private in [
            "127.0.0.1",
            "10.0.0.8",
            "172.16.4.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "224.0.0.1",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:192.168.1.1",
            "64:ff9b::a00:1",
        ] {
            assert!(!is_public(private.parse().unwrap()), "{private}");
        }
        for public in ["1.1.1.1", "104.16.0.1", "2606:4700::1111"] {
            assert!(is_public(public.parse().unwrap()), "{public}");
        }
    }

    #[test]
    fn a_name_resolves_to_its_public_addresses_only() {
        let addresses = ["10.0.0.1:443", "1.1.1.1:443"]
            .map(|address| address.parse::<SocketAddr>().unwrap());
        assert_eq!(
            public_only("cdn.modrinth.com", addresses.into_iter()).unwrap(),
            [addresses[1]]
        );
        let lan = ["192.168.0.2:443".parse::<SocketAddr>().unwrap()];
        assert!(public_only("cdn.modrinth.com", lan.into_iter()).is_err());
    }

    #[tokio::test]
    async fn the_client_refuses_a_url_off_the_list() {
        // localhost is never resolved: the resolver refuses names off the list first.
        let error = client()
            .unwrap()
            .get("https://localhost/a.jar")
            .send()
            .await
            .unwrap_err();
        assert!(error.is_connect() || error.is_request(), "{error}");
    }
}
