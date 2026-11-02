//! Which sides an addon runs on: what its file on disk and its recorded sides say together.

use bedrock::state::ContentRequirement;

use crate::state::servers::{
    EntrySides, adapters::filesystem::content_files::Listed,
};

/// Where an addon runs, from its file and its recorded sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sides {
    /// Off for both sides.
    pub disabled: bool,
    pub disabled_server: bool,
    pub disabled_player: bool,
}

impl Sides {
    /// A `.disabled` file is off for the server alone only while its sides say so; a file
    /// renamed back by hand runs again.
    pub fn of(listed: &Listed, sides: Option<&EntrySides>) -> Self {
        let disabled_server = listed.disabled
            && sides.is_some_and(|sides| {
                sides.server_requirement == ContentRequirement::Unsupported
            });
        Self {
            disabled: listed.disabled && !disabled_server,
            disabled_server,
            disabled_player: sides.is_some_and(|sides| {
                sides.client_requirement == ContentRequirement::Unsupported
            }),
        }
    }

    /// The server's share carries it, for players.
    pub fn for_players(self) -> bool {
        !self.disabled && !self.disabled_player
    }
}
