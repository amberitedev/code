use crate::api::Result;
use engine::users::SearchUser;
use serde_json::Value;

#[tauri::command]
pub async fn search_user(query: &str) -> Result<Vec<SearchUser>> {
    Ok(engine::users::search_user(query).await?)
}

#[tauri::command]
pub async fn get_user_profile(user_id: &str) -> Result<Value> {
    Ok(engine::users::get_user_profile(user_id).await?)
}

#[tauri::command]
pub async fn get_user_projects(user_id: &str) -> Result<Value> {
    Ok(engine::users::get_user_projects(user_id).await?)
}

#[tauri::command]
pub async fn get_user_organizations(user_id: &str) -> Result<Value> {
    Ok(engine::users::get_user_organizations(user_id).await?)
}

#[tauri::command]
pub async fn get_user_collections(user_id: &str) -> Result<Value> {
    Ok(engine::users::get_user_collections(user_id).await?)
}

#[tauri::command]
pub async fn get_user_preferences(user_id: &str) -> Result<Value> {
    Ok(engine::users::get_user_preferences(user_id).await?)
}

#[tauri::command]
pub async fn patch_user_preferences(
    user_id: &str,
    preferences: Value,
) -> Result<Value> {
    Ok(engine::users::patch_user_preferences(user_id, preferences).await?)
}

#[tauri::command]
pub async fn patch_user(user_id: &str, patch: Value) -> Result<()> {
    Ok(engine::users::patch_user(user_id, patch).await?)
}

#[tauri::command]
pub async fn change_user_avatar(
    user_id: &str,
    image: Vec<u8>,
    extension: &str,
) -> Result<()> {
    Ok(
        engine::users::change_user_avatar(user_id, image.into(), extension)
            .await?,
    )
}

#[tauri::command]
pub async fn delete_user_avatar(user_id: &str) -> Result<()> {
    Ok(engine::users::delete_user_avatar(user_id).await?)
}

#[tauri::command]
pub async fn block_user(user_id: &str) -> Result<()> {
    Ok(engine::users::block_user(user_id).await?)
}

#[tauri::command]
pub async fn unblock_user(user_id: &str) -> Result<()> {
    Ok(engine::users::unblock_user(user_id).await?)
}

#[tauri::command]
pub async fn get_blocked_users() -> Result<Vec<String>> {
    Ok(engine::users::get_blocked_users().await?)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("users")
        .invoke_handler(tauri::generate_handler![
            search_user,
            get_user_profile,
            get_user_projects,
            get_user_organizations,
            get_user_collections,
            get_user_preferences,
            patch_user_preferences,
            patch_user,
            change_user_avatar,
            delete_user_avatar,
            block_user,
            unblock_user,
            get_blocked_users,
        ])
        .build()
}
