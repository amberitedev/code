//! A server's `server.properties`.

use std::{collections::BTreeMap, sync::Arc};

use axum::extract::State;
use contracts::{
    ApiError, Json,
    core::{
        Action, ChangeProperties, GetProperties, PatchPropertiesFields,
        PropertiesFields,
    },
    server::Req,
};

use super::{CoreState, internal};
use crate::{
    access::{Caller, activity},
    servers::properties,
};

pub async fn get(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, .. }: Req<GetProperties>,
) -> Result<Json<PropertiesFields>, ApiError> {
    state.access.require(&caller, &params.id).await?;
    let dir = state.servers.paths().server(&params.id);
    Ok(Json(fields(properties::read(&dir).map_err(internal)?)))
}

/// Takes effect at the next start.
pub async fn change(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, body, .. }: Req<ChangeProperties>,
) -> Result<Json<PropertiesFields>, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id).await?;
    let dir = state.servers.paths().server(&id);
    properties::patch(&dir, &changes(body)).map_err(internal)?;
    activity::log(&state.db, &caller, Some(&id), Action::SettingsChanged).await;
    Ok(Json(fields(properties::read(&dir).map_err(internal)?)))
}

/// Every key of the file, split into the known fields and the rest.
fn fields(file: BTreeMap<String, String>) -> PropertiesFields {
    let mut fields = PropertiesFields::default();
    for (key, value) in file {
        if !fields.known.set(&key, value.clone()) {
            fields.custom.insert(key, value);
        }
    }
    fields
}

/// The file's keys to change: a known field by its key, and the custom keys as they are.
fn changes(patch: PatchPropertiesFields) -> BTreeMap<String, Option<String>> {
    let mut changes: BTreeMap<String, Option<String>> = patch
        .known
        .unwrap_or_default()
        .entries()
        .into_iter()
        .map(|(key, value)| (key, Some(value.to_owned())))
        .collect();
    changes.extend(patch.custom.unwrap_or_default());
    changes
}

#[cfg(test)]
mod tests {
    use contracts::core::KnownPropertiesFields;

    use super::*;

    #[test]
    fn known_keys_are_fields_and_the_rest_stay_as_the_file_names_them() {
        let read = fields(BTreeMap::from([
            ("max-players".to_owned(), "8".to_owned()),
            ("white-list".to_owned(), "true".to_owned()),
            ("spawn-monsters".to_owned(), "false".to_owned()),
        ]));
        assert_eq!(read.known.max_players.as_deref(), Some("8"));
        assert_eq!(read.known.white_list.as_deref(), Some("true"));
        assert_eq!(
            read.custom,
            BTreeMap::from([("spawn-monsters".to_owned(), "false".to_owned())])
        );

        let written = changes(PatchPropertiesFields {
            known: Some(KnownPropertiesFields {
                motd: Some("hi".to_owned()),
                ..Default::default()
            }),
            custom: Some(BTreeMap::from([("spawn-monsters".to_owned(), None)])),
        });
        assert_eq!(
            written,
            BTreeMap::from([
                ("motd".to_owned(), Some("hi".to_owned())),
                ("spawn-monsters".to_owned(), None),
            ])
        );
    }
}
