use crate::state::{FriendsSocket, UserFriend};
use ariadne::users::UserStatus;

#[tracing::instrument(skip(state))]
pub async fn friends(state: &crate::State) -> crate::Result<Vec<UserFriend>> {
    let friends =
        FriendsSocket::friends(&state.api_semaphore, &state.sessions).await?;

    Ok(friends)
}

pub async fn friend_statuses(
    state: &crate::State,
) -> crate::Result<Vec<UserStatus>> {
    let statuses = state.presence.friends_socket.friend_statuses();

    Ok(statuses)
}

#[tracing::instrument(skip(state))]
pub async fn add_friend(
    state: &crate::State,
    user_id: &str,
) -> crate::Result<()> {
    FriendsSocket::add_friend(user_id, &state.api_semaphore, &state.sessions)
        .await?;

    Ok(())
}

#[tracing::instrument(skip(state))]
pub async fn remove_friend(
    state: &crate::State,
    user_id: &str,
) -> crate::Result<()> {
    FriendsSocket::remove_friend(
        user_id,
        &state.api_semaphore,
        &state.sessions,
    )
    .await?;

    Ok(())
}
