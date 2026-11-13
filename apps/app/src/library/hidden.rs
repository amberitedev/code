use super::hidden_instances;
use crate::accounts;
use engine::ErrorKind;
use engine::state::Engine;

#[tracing::instrument(skip(state))]
pub async fn get_hidden_instance_ids(
    state: &Engine,
) -> engine::Result<Vec<String>> {
    let Some(user_id) =
        accounts::active_modrinth_user_id(crate::db::pool(state)).await?
    else {
        return Ok(Vec::new());
    };

    hidden_instances::get_hidden_instance_ids(&user_id, crate::db::pool(state))
        .await
}

#[tracing::instrument(skip(state))]
pub async fn set_instance_hidden(
    state: &Engine,
    instance_id: &str,
    hidden: bool,
) -> engine::Result<()> {
    let user_id = accounts::active_modrinth_user_id(crate::db::pool(state))
        .await?
        .ok_or(ErrorKind::NotLoggedIn)?;

    hidden_instances::set_instance_hidden(
        &user_id,
        instance_id,
        hidden,
        crate::db::pool(state),
    )
    .await
}
