//! Serves `crate::local_api::endpoints::friends` over `crate::friends::api`.

use crate::state::Desktop;
use std::sync::Arc;

use ariadne::users::UserStatus;
use axum::{Router, extract::State};
use contracts::{
    ApiError, Empty, Json,
    backend::social::UserFriend,
    server::{Mount, Req},
};
use engine::routes::api_error;

use super::api;
use crate::presence::Presence;

use crate::local_api::{
    AppState,
    endpoints::friends::{
        AddFriend, ListFriendStatuses, ListFriends, RemoveFriend,
    },
};

type Answer<T> = Result<T, ApiError>;

pub fn routes() -> Router<AppState> {
    Router::new()
        .endpoint::<ListFriends, _, _>(list_friends)
        .endpoint::<ListFriendStatuses, _, _>(list_friend_statuses)
        .endpoint::<AddFriend, _, _>(add_friend)
        .endpoint::<RemoveFriend, _, _>(remove_friend)
}

async fn list_friends(
    State(engine): State<Desktop>,
    _: Req<ListFriends>,
) -> Answer<Json<Vec<UserFriend>>> {
    api::friends(&engine).await.map(Json).map_err(api_error)
}

async fn list_friend_statuses(
    State(presence): State<Arc<Presence>>,
    _: Req<ListFriendStatuses>,
) -> Answer<Json<Vec<UserStatus>>> {
    api::friend_statuses(&presence)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn add_friend(
    State(engine): State<Desktop>,
    Req { params, .. }: Req<AddFriend>,
) -> Answer<Empty> {
    api::add_friend(&engine, &params.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn remove_friend(
    State(engine): State<Desktop>,
    Req { params, .. }: Req<RemoveFriend>,
) -> Answer<Empty> {
    api::remove_friend(&engine, &params.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}
