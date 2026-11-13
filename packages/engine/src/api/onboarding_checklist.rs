use crate::Engine;
use crate::event::emit::emit_onboarding_checklist;
use crate::state::{
    OnboardingChecklist, OnboardingChecklistItem, get_onboarding_checklist,
    mark_onboarding_checklist_item,
};

#[tracing::instrument(skip(state))]
pub async fn get(state: &Engine) -> crate::Result<OnboardingChecklist> {
    get_onboarding_checklist(&state.pool).await
}

#[tracing::instrument(skip(state))]
pub async fn mark_created_instance(state: &crate::Engine) -> crate::Result<()> {
    mark(state, OnboardingChecklistItem::CreatedInstance).await
}

pub async fn mark_logged_into_minecraft(
    state: &crate::Engine,
) -> crate::Result<()> {
    mark(state, OnboardingChecklistItem::LoggedIntoMinecraft).await
}

pub async fn mark_logged_into_modrinth(
    state: &crate::Engine,
) -> crate::Result<()> {
    mark(state, OnboardingChecklistItem::LoggedIntoModrinth).await
}

async fn mark(
    state: &Engine,
    item: OnboardingChecklistItem,
) -> crate::Result<()> {
    if let Some(checklist) =
        mark_onboarding_checklist_item(item, &state.pool).await?
    {
        emit_onboarding_checklist(&state.events, checklist).await?;
    }

    Ok(())
}
