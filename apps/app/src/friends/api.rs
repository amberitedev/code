use crate::friends::state::FriendsSocket;
use crate::presence::Presence;
use ariadne::users::UserStatus;
use contracts::backend::social::UserFriend;

#[tracing::instrument(skip(state))]
pub async fn friends(
    state: &engine::Engine,
) -> engine::Result<Vec<UserFriend>> {
    let friends =
        FriendsSocket::friends(&state.api_semaphore, &state.sessions).await?;

    Ok(friends)
}

pub async fn friend_statuses(
    presence: &Presence,
) -> engine::Result<Vec<UserStatus>> {
    let statuses = presence.friends_socket.friend_statuses();

    Ok(statuses)
}

#[tracing::instrument(skip(state))]
pub async fn add_friend(
    state: &engine::Engine,
    user_id: &str,
) -> engine::Result<()> {
    FriendsSocket::add_friend(user_id, &state.api_semaphore, &state.sessions)
        .await?;

    Ok(())
}

#[tracing::instrument(skip(state))]
pub async fn remove_friend(
    state: &engine::Engine,
    user_id: &str,
) -> engine::Result<()> {
    FriendsSocket::remove_friend(
        user_id,
        &state.api_semaphore,
        &state.sessions,
    )
    .await?;

    Ok(())
}
