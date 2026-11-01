//! Which side runs a Modrinth version, as a pack index says it.

use crate::content::model::modrinth::VersionEnvironment;
use crate::content::model::mrpack::{Env, SideType};

/// The pack `env` of a version that runs in `environment`. Upstream's rule
/// (app-lib `api/instance/export_mrpack.rs`, `get_mrpack_environment`): a version runs on both
/// sides unless Modrinth says it is for players only or for dedicated servers only.
pub fn mrpack_env(environment: Option<VersionEnvironment>) -> Env {
    use VersionEnvironment::*;
    let (client, server) = match environment.unwrap_or(Unknown) {
        ClientAndServer
        | ClientOnlyServerOptional
        | ServerOnly
        | ServerOnlyClientOptional
        | ClientOrServer
        | ClientOrServerPrefersBoth
        | Unknown => (SideType::Required, SideType::Required),
        ClientOnly | SingleplayerOnly => {
            (SideType::Required, SideType::Unsupported)
        }
        DedicatedServerOnly => (SideType::Unsupported, SideType::Required),
    };
    Env { client, server }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn players_only_and_server_only_versions_skip_the_other_side() {
        assert_eq!(
            mrpack_env(Some(VersionEnvironment::ClientOnly)).server,
            SideType::Unsupported
        );
        assert_eq!(
            mrpack_env(Some(VersionEnvironment::SingleplayerOnly)).server,
            SideType::Unsupported
        );
        assert_eq!(
            mrpack_env(Some(VersionEnvironment::DedicatedServerOnly)).client,
            SideType::Unsupported
        );
        let both = mrpack_env(None);
        assert_eq!(
            (both.client, both.server),
            (SideType::Required, SideType::Required)
        );
    }
}
