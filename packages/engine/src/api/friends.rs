use crate::state::{FriendsSocket, UserFriend};
use ariadne::users::UserStatus;

#[tracing::instrument]
pub async fn friends() -> crate::Result<Vec<UserFriend>> {
    let state = crate::State::get().await?;
    let friends =
        FriendsSocket::friends(&state.api_semaphore, &state.sessions).await?;

    Ok(friends)
}

pub async fn friend_statuses() -> crate::Result<Vec<UserStatus>> {
    let state = crate::State::get().await?;
    let statuses = state.presence.friends_socket.friend_statuses();

    Ok(statuses)
}

#[tracing::instrument]
pub async fn add_friend(user_id: &str) -> crate::Result<()> {
    let state = crate::State::get().await?;
    FriendsSocket::add_friend(user_id, &state.api_semaphore, &state.sessions)
        .await?;

    Ok(())
}

#[tracing::instrument]
pub async fn remove_friend(user_id: &str) -> crate::Result<()> {
    let state = crate::State::get().await?;
    FriendsSocket::remove_friend(
        user_id,
        &state.api_semaphore,
        &state.sessions,
    )
    .await?;

    Ok(())
}
