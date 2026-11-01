//! Reaching Core and its servers from outside: TLS for the API, port mappings that belong to
//! servers (`forward`), and the reachability diagnosis.

pub mod diagnosis;
pub mod firewall;
pub mod forward;
pub mod public_ip;
pub mod router;
pub mod tls;

/// One rule for public and private addresses, shared with the store and the App.
pub use bedrock::net::ip::{is_private, is_public, is_shared_range};

/// Where every reqwest client in Core starts. reqwest is built without a TLS provider so Core
/// stays on ring, and it panics without one installed; installing it here rather than only in
/// `main` covers tests too.
pub fn http() -> reqwest::ClientBuilder {
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
}
