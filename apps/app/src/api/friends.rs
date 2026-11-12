use crate::engine_handle::EngineHandle;
use engine::prelude::{UserFriend, UserStatus};
use tauri::plugin::TauriPlugin;

pub fn init<R: tauri::Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("friends")
        .invoke_handler(tauri::generate_handler![
            friends,
            friend_statuses,
            add_friend,
            remove_friend
        ])
        .build()
}

#[tauri::command]
pub async fn friends(
    handle: tauri::State<'_, EngineHandle>,
) -> crate::api::Result<Vec<UserFriend>> {
    let state = handle.get().await;
    Ok(engine::friends::friends(&state).await?)
}

#[tauri::command]
pub async fn friend_statuses(
    handle: tauri::State<'_, EngineHandle>,
) -> crate::api::Result<Vec<UserStatus>> {
    let state = handle.get().await;
    Ok(engine::friends::friend_statuses(&state).await?)
}

#[tauri::command]
pub async fn add_friend(
    handle: tauri::State<'_, EngineHandle>,
    user_id: &str,
) -> crate::api::Result<()> {
    let state = handle.get().await;
    Ok(engine::friends::add_friend(&state, user_id).await?)
}

#[tauri::command]
pub async fn remove_friend(
    handle: tauri::State<'_, EngineHandle>,
    user_id: &str,
) -> crate::api::Result<()> {
    let state = handle.get().await;
    Ok(engine::friends::remove_friend(&state, user_id).await?)
}
