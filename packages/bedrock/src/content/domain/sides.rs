//! Which side runs a Modrinth version, as a pack index says it.

use crate::content::model::modrinth::VersionEnvironment;
use crate::content::model::mrpack::{Env, SideType};
use crate::upstream::jar::Environment as JarEnvironment;

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

/// Whether a server takes a file of a pack, by the first of these that knows:
///
/// 1. `env`, the pack index's word: a server leaves out a file whose server side is
///    unsupported. A file of `overrides/` has none.
/// 2. `modrinth`, where Modrinth says the version with the file's hash runs, through
///    [`mrpack_env`].
/// 3. `jar`, the sides a jar Modrinth does not know names in its Fabric or Quilt metadata.
///    Forge and NeoForge metadata names none.
///
/// A file none of them rules out is taken: guessing a server-side mod is client only breaks the
/// server, while an extra client mod on a server only costs a warning in its log.
pub fn server_takes(
    env: Option<&Env>,
    modrinth: Option<VersionEnvironment>,
    jar: Option<JarEnvironment>,
) -> bool {
    if let Some(env) = env {
        return env.server != SideType::Unsupported;
    }
    if modrinth.is_some() {
        return mrpack_env(modrinth).server != SideType::Unsupported;
    }
    jar != Some(JarEnvironment::ClientOnly)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_server_takes_a_file_unless_the_first_that_knows_rules_it_out() {
        let client_only = Env {
            client: SideType::Required,
            server: SideType::Unsupported,
        };
        let both = Env {
            client: SideType::Required,
            server: SideType::Optional,
        };
        // The index decides when it says.
        assert!(!server_takes(
            Some(&client_only),
            Some(VersionEnvironment::ClientAndServer),
            None
        ));
        assert!(server_takes(
            Some(&both),
            Some(VersionEnvironment::ClientOnly),
            Some(JarEnvironment::ClientOnly)
        ));
        // Then Modrinth, then the jar.
        assert!(!server_takes(
            None,
            Some(VersionEnvironment::ClientOnly),
            None
        ));
        assert!(server_takes(
            None,
            Some(VersionEnvironment::ClientAndServer),
            Some(JarEnvironment::ClientOnly)
        ));
        assert!(!server_takes(None, None, Some(JarEnvironment::ClientOnly)));
        // A Forge jar names no sides, and nothing else knows: taken.
        assert!(server_takes(None, None, None));
    }

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
