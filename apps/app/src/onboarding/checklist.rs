use engine::Engine;
use engine::data::OnboardingChecklist;
use engine::emit_onboarding_checklist;

use super::state::{
    OnboardingChecklistItem, get_onboarding_checklist,
    mark_onboarding_checklist_item,
};

#[tracing::instrument(skip(state))]
pub async fn get(state: &Engine) -> engine::Result<OnboardingChecklist> {
    get_onboarding_checklist(crate::db::pool(state)).await
}

#[tracing::instrument(skip(state))]
pub async fn mark_created_instance(
    state: &engine::Engine,
) -> engine::Result<()> {
    mark(state, OnboardingChecklistItem::CreatedInstance).await
}

pub async fn mark_logged_into_minecraft(
    state: &engine::Engine,
) -> engine::Result<()> {
    mark(state, OnboardingChecklistItem::LoggedIntoMinecraft).await
}

pub async fn mark_logged_into_modrinth(
    state: &engine::Engine,
) -> engine::Result<()> {
    mark(state, OnboardingChecklistItem::LoggedIntoModrinth).await
}

async fn mark(
    state: &Engine,
    item: OnboardingChecklistItem,
) -> engine::Result<()> {
    if let Some(checklist) =
        mark_onboarding_checklist_item(item, crate::db::pool(state)).await?
    {
        emit_onboarding_checklist(&state.events, checklist).await?;
    }

    Ok(())
}
