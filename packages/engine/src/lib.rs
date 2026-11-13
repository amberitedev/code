/*!
# Theseus

Theseus is a library which provides utilities for launching minecraft, creating Modrinth mod packs,
and launching Modrinth mod packs
*/
#![warn(unused_import_braces)]
#![deny(unused_must_use)]
#![recursion_limit = "256"]

#[macro_use]
pub mod util;

pub mod access;
mod api;
pub mod endpoints;
mod error;
mod event;
pub mod install;
mod launcher;
pub mod routes;
pub mod self_hosted;
pub mod state;
#[cfg(test)]
mod ts;

pub use api::*;
pub use error::*;
pub use event::{
    DesktopEvent, Event, EventSink, FriendPayload,
    InstanceGroupsChangedPayload, InstancePayload, InstancePayloadType,
    LoadingBar, LoadingBarType, ProcessPayload, ProcessPayloadType,
    emit::emit_command, emit::emit_friend, emit::emit_instance,
    emit::emit_loading, emit::emit_notification,
    emit::emit_onboarding_checklist, emit::emit_warning, emit::init_loading,
};
pub use launcher::{
    read_game_version_metadata_from_jar, resolve_minecraft_manifest,
};
pub use self_hosted::accounts::AccountEndpoints;
pub use state::INSTANCES_FOLDER_NAME;
pub use state::instances::commands::server_jar;
pub use state::{
    BackendSession, BackendSessionKind, Engine, EngineConfig,
    GameOptionsPackSource, InstanceHooks, ModrinthSession, PackOverrides,
    PackServers, ProcessManager, SavedMinecraftAccount, SavedSecrets, Sessions,
};
pub use util::content_hash::encode_hex;
pub use util::fetch::{
    DownloadReason, FetchSemaphore, INSECURE_REQWEST_CLIENT, fetch_advanced,
    fetch_json,
};
pub use util::{archive, pack_hosts, sandbox};

/// Installs `ring` as the process-wide rustls crypto provider.
///
/// Must be called before any TLS connection is made: reqwest is built with
/// `rustls-no-provider` and the friends socket relies on the process default.
pub fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

pub fn launcher_user_agent() -> String {
    const LAUNCHER_BASE_USER_AGENT: &str =
        concat!("modrinth/theseus/", env!("CARGO_PKG_VERSION"),);

    format!(
        "{} ({}; support@modrinth.com)",
        LAUNCHER_BASE_USER_AGENT,
        std::env::consts::OS
    )
}
