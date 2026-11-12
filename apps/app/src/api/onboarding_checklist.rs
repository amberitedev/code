use crate::api::Result;
use crate::engine_handle::EngineHandle;
use engine::prelude::*;
use tauri::plugin::TauriPlugin;

pub fn init<R: tauri::Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("onboarding-checklist")
        .invoke_handler(tauri::generate_handler![
            get_onboarding_checklist,
            skip_create_instance_onboarding
        ])
        .build()
}

#[tauri::command]
pub async fn get_onboarding_checklist(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<OnboardingChecklist> {
    let state = handle.get().await;
    Ok(onboarding_checklist::get(&state).await?)
}

#[tauri::command]
pub async fn skip_create_instance_onboarding(
    handle: tauri::State<'_, EngineHandle>,
) -> Result<()> {
    let state = handle.get().await;
    Ok(onboarding_checklist::mark_created_instance(&state).await?)
}
