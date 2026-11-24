//! The signed-in Modrinth user's friends and their presence.

use ariadne::users::UserStatus;
use contracts::{Empty, Json, backend::social::UserFriend};

use super::IdParams;

endpoints! {
    ListFriends: Get "/app/friends", (), (), Empty => Json<Vec<UserFriend>>;
    /// The friends who are online and what they are playing.
    ListFriendStatuses: Get "/app/friend-statuses", (), (), Empty
        => Json<Vec<UserStatus>>;
    /// Sends a request to the user with this id or username, or accepts theirs.
    AddFriend: Post "/app/friends/{id}", IdParams, (), Empty => Empty;
    /// Removes a friend, or declines or cancels a request.
    RemoveFriend: Delete "/app/friends/{id}", IdParams, (), Empty => Empty;
}
