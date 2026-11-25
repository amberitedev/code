use super::hidden_instances;
use crate::accounts;
use crate::state::Desktop;
use engine::ErrorKind;

#[tracing::instrument(skip(state))]
pub async fn get_hidden_instance_ids(
    state: &Desktop,
) -> engine::Result<Vec<String>> {
    let Some(user_id) = accounts::active_modrinth_user_id(state.db()).await?
    else {
        return Ok(Vec::new());
    };

    hidden_instances::get_hidden_instance_ids(&user_id, state.db()).await
}

#[tracing::instrument(skip(state))]
pub async fn set_instance_hidden(
    state: &Desktop,
    instance_id: &str,
    hidden: bool,
) -> engine::Result<()> {
    let user_id = accounts::active_modrinth_user_id(state.db())
        .await?
        .ok_or(ErrorKind::NotLoggedIn)?;

    hidden_instances::set_instance_hidden(
        &user_id,
        instance_id,
        hidden,
        state.db(),
    )
    .await
}
