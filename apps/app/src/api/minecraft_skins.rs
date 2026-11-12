use crate::api::Result;
use crate::engine_handle::EngineHandle;

use engine::minecraft_skins::{
    self, Bytes, Cape, MinecraftSkinVariant, Skin, UrlOrBlob,
};

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("minecraft-skins")
        .invoke_handler(tauri::generate_handler![
            get_available_capes,
            get_available_skins,
            add_and_equip_custom_skin,
            equip_skin,
            remove_custom_skin,
            save_custom_skin,
            set_custom_skin_order,
            unequip_skin,
            flush_pending_skin_change,
            flush_pending_skin_change_for_profile,
            normalize_skin_texture,
        ])
        .build()
}

/// `invoke('plugin:minecraft-skins|get_available_capes')`
///
/// See also: [minecraft_skins::get_available_capes]
#[tauri::command]
pub async fn get_available_capes(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<Vec<Cape>> {
    let state = handle.get().await;
    Ok(minecraft_skins::get_available_capes(&state).await?)
}

/// `invoke('plugin:minecraft-skins|get_available_skins')`
///
/// See also: [minecraft_skins::get_available_skins]
#[tauri::command]
pub async fn get_available_skins(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<Vec<Skin>> {
    let state = handle.get().await;
    Ok(minecraft_skins::get_available_skins(&state).await?)
}

/// `invoke('plugin:minecraft-skins|add_and_equip_custom_skin', texture_blob, variant, cape)`
///
/// See also: [minecraft_skins::add_and_equip_custom_skin]
#[tauri::command]
pub async fn add_and_equip_custom_skin(
    handle: tauri::State<'_, EngineHandle>,
    texture_blob: Bytes,
    variant: MinecraftSkinVariant,
    cape: Option<Cape>,
) -> Result<Skin> {
    let state = handle.get().await;
    Ok(minecraft_skins::add_and_equip_custom_skin(
        &state,
        texture_blob,
        variant,
        cape,
    )
    .await?)
}

/// `invoke('plugin:minecraft-skins|equip_skin', skin)`
///
/// See also: [minecraft_skins::equip_skin]
#[tauri::command]
pub async fn equip_skin(
    handle: tauri::State<'_, EngineHandle>,
    skin: Skin,
) -> Result<()> {
    let state = handle.get().await;
    Ok(minecraft_skins::equip_skin(&state, skin).await?)
}

/// `invoke('plugin:minecraft-skins|remove_custom_skin', skin)`
///
/// See also: [minecraft_skins::remove_custom_skin]
#[tauri::command]
pub async fn remove_custom_skin(
    handle: tauri::State<'_, EngineHandle>,
    skin: Skin,
) -> Result<()> {
    let state = handle.get().await;
    Ok(minecraft_skins::remove_custom_skin(&state, skin).await?)
}

/// `invoke('plugin:minecraft-skins|save_custom_skin', skin, texture_blob, variant, cape, replace_texture)`
///
/// See also: [minecraft_skins::save_custom_skin]
#[tauri::command]
pub async fn save_custom_skin(
    handle: tauri::State<'_, EngineHandle>,
    skin: Skin,
    texture_blob: Bytes,
    variant: MinecraftSkinVariant,
    cape: Option<Cape>,
    replace_texture: bool,
) -> Result<Skin> {
    let state = handle.get().await;
    Ok(minecraft_skins::save_custom_skin(
        &state,
        skin,
        texture_blob,
        variant,
        cape,
        replace_texture,
    )
    .await?)
}

/// `invoke('plugin:minecraft-skins|set_custom_skin_order', texture_keys)`
///
/// See also: [minecraft_skins::set_custom_skin_order]
#[tauri::command]
pub async fn set_custom_skin_order(
    handle: tauri::State<'_, EngineHandle>,
    texture_keys: Vec<String>,
) -> Result<()> {
    let state = handle.get().await;
    Ok(minecraft_skins::set_custom_skin_order(&state, texture_keys).await?)
}

/// `invoke('plugin:minecraft-skins|unequip_skin')`
///
/// See also: [minecraft_skins::unequip_skin]
#[tauri::command]
pub async fn unequip_skin(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<()> {
    let state = handle.get().await;
    Ok(minecraft_skins::unequip_skin(&state).await?)
}

/// `invoke('plugin:minecraft-skins|flush_pending_skin_change')`
///
/// See also: [minecraft_skins::flush_pending_skin_change]
#[tauri::command]
pub async fn flush_pending_skin_change(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<()> {
    let state = handle.get().await;
    Ok(minecraft_skins::flush_pending_skin_change(&state).await?)
}

/// `invoke('plugin:minecraft-skins|flush_pending_skin_change_for_profile', profile_id)`
///
/// See also: [minecraft_skins::flush_pending_skin_change_for_profile]
#[tauri::command]
pub async fn flush_pending_skin_change_for_profile(
    handle: tauri::State<'_, EngineHandle>,
    profile_id: uuid::Uuid,
) -> Result<()> {
    let state = handle.get().await;
    Ok(minecraft_skins::flush_pending_skin_change_for_profile(
        &state, profile_id,
    )
    .await?)
}

/// `invoke('plugin:minecraft-skins|normalize_skin_texture')`
///
/// See also: [minecraft_skins::normalize_skin_texture]
#[tauri::command]
pub async fn normalize_skin_texture(texture: UrlOrBlob) -> Result<Bytes> {
    Ok(minecraft_skins::normalize_skin_texture(&texture).await?)
}
