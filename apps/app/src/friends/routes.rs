//! Serves `crate::server::endpoints::friends` over `crate::friends::api`.

use std::sync::Arc;

use ariadne::users::UserStatus;
use axum::{Router, extract::State};
use contracts::{
    ApiError, Empty, Json,
    backend::social::UserFriend,
    server::{Req, route},
};
use engine::{Engine, routes::api_error};

use super::api;
use crate::presence::Presence;

use crate::server::{
    AppState,
    endpoints::friends::{
        AddFriend, ListFriendStatuses, ListFriends, RemoveFriend,
    },
};

type Answer<T> = Result<T, ApiError>;

pub fn routes() -> Router<AppState> {
    [
        route::<ListFriends, _, _, _>(list_friends),
        route::<ListFriendStatuses, _, _, _>(list_friend_statuses),
        route::<AddFriend, _, _, _>(add_friend),
        route::<RemoveFriend, _, _, _>(remove_friend),
    ]
    .into_iter()
    .fold(Router::new(), |router, (path, method)| {
        router.route(path, method)
    })
}

async fn list_friends(
    State(engine): State<Arc<Engine>>,
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
    State(engine): State<Arc<Engine>>,
    Req { params, .. }: Req<AddFriend>,
) -> Answer<Empty> {
    api::add_friend(&engine, &params.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn remove_friend(
    State(engine): State<Arc<Engine>>,
    Req { params, .. }: Req<RemoveFriend>,
) -> Answer<Empty> {
    api::remove_friend(&engine, &params.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}
