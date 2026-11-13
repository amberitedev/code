//! A server's `server.properties`. Editors and the owner only: the file can hold secrets, such as
//! a resource pack URL.

use std::sync::Arc;

use axum::extract::State;
use contracts::{
    ApiError, Json,
    core::{Action, ChangeProperties, GetProperties, ServerProperties},
    server::Req,
};

use super::{CoreState, internal};
use crate::{
    access::{Caller, Permission, activity},
    state::servers::adapters::filesystem::properties,
};

// The settings a content install takes, as `state::servers::commands` read them.
pub use crate::state::servers::model::PropertiesFields;

pub async fn get(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, .. }: Req<GetProperties>,
) -> Result<Json<ServerProperties>, ApiError> {
    state
        .access
        .require(&caller, &params.id, Permission::Edit)
        .await?;
    let dir = state.servers.paths().server(&params.id);
    Ok(Json(ServerProperties {
        properties: properties::read(&dir).map_err(internal)?,
    }))
}

/// Takes effect at the next start.
pub async fn change(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, body, .. }: Req<ChangeProperties>,
) -> Result<Json<ServerProperties>, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id, Permission::Edit).await?;
    let dir = state.servers.paths().server(&id);
    // Read, patched, and written under the lock installs and file edits take, so neither lands
    // between the read and the write and is lost.
    let busy = state.content.busy(&id)?;
    properties::patch(&dir, &body.properties).map_err(internal)?;
    drop(busy);
    activity::log(&state.db, &caller, Some(&id), Action::SettingsChanged).await;
    Ok(Json(ServerProperties {
        properties: properties::read(&dir).map_err(internal)?,
    }))
}

#[cfg(test)]
mod tests {
    use crate::state::servers::model::{KnownPropertiesFields, known_pairs};

    #[test]
    fn known_keys_have_dashes_in_the_file() {
        let known = KnownPropertiesFields {
            max_players: Some("10".into()),
            white_list: Some("true".into()),
            ..Default::default()
        };
        assert_eq!(
            known_pairs(&known),
            [
                ("max-players".to_owned(), "10".to_owned()),
                ("white-list".to_owned(), "true".to_owned())
            ]
        );
    }
}
