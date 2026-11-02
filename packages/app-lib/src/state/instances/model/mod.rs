#![allow(dead_code)]

mod content_set_remote_ref;
pub use self::content_set_remote_ref::*;

// content_entry.rs, content_set.rs, content_set_sync_state.rs, file.rs, link.rs and
// manifest.rs are compiled by bedrock so Copal shares them.
pub use bedrock::state::instances::model::*;

mod game_options;
pub use self::game_options::{CanonicalValue, GameOptionKind};
pub(crate) use self::game_options::{StoredOption, StoredPreference};

mod instance;
pub use self::instance::*;

mod install_candidate;
pub use self::install_candidate::*;

mod launch;
pub use self::launch::*;

mod update_check;
pub use self::update_check::*;

fn unknown_value(kind: &str, value: &str) -> crate::Error {
    crate::ErrorKind::InputError(format!("unknown {kind} {value}")).into()
}
