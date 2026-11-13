use crate::ErrorKind;
use crate::state::{Engine, accounts, hidden_instances};

#[tracing::instrument(skip(state))]
pub async fn get_hidden_instance_ids(
    state: &Engine,
) -> crate::Result<Vec<String>> {
    let Some(user_id) = accounts::active_modrinth_user_id(&state.pool).await?
    else {
        return Ok(Vec::new());
    };

    hidden_instances::get_hidden_instance_ids(&user_id, &state.pool).await
}

#[tracing::instrument(skip(state))]
pub async fn set_instance_hidden(
    state: &Engine,
    instance_id: &str,
    hidden: bool,
) -> crate::Result<()> {
    let user_id = accounts::active_modrinth_user_id(&state.pool)
        .await?
        .ok_or(ErrorKind::NotLoggedIn)?;

    hidden_instances::set_instance_hidden(
        &user_id,
        instance_id,
        hidden,
        &state.pool,
    )
    .await
}
