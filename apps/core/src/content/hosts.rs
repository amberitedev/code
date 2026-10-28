//! Pack content is downloaded only from the hosts Modrinth allows in a pack. A pack can name any
//! URL, so the client checks the host of the first URL and of every redirect, resolves the name
//! itself, refuses private addresses, and connects to exactly the addresses it checked: a name
//! that resolves to the LAN, or changes its answer between check and connect, gets nowhere.

use std::{net::SocketAddr, sync::Arc};

use reqwest::{
    Url,
    dns::{Addrs, Name, Resolve, Resolving},
    redirect,
};

use sharing_storage::is_public;

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
