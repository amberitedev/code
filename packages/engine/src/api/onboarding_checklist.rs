use crate::State;
use crate::event::emit::emit_onboarding_checklist;
use crate::state::{
    OnboardingChecklist, OnboardingChecklistItem, get_onboarding_checklist,
    mark_onboarding_checklist_item,
};

#[tracing::instrument(skip(state))]
pub async fn get(state: &State) -> crate::Result<OnboardingChecklist> {
    get_onboarding_checklist(&state.pool).await
}

#[tracing::instrument(skip(state))]
pub async fn mark_created_instance(state: &crate::State) -> crate::Result<()> {
    mark(state, OnboardingChecklistItem::CreatedInstance).await
}

pub(crate) async fn mark_logged_into_minecraft(
    state: &crate::State,
) -> crate::Result<()> {
    mark(state, OnboardingChecklistItem::LoggedIntoMinecraft).await
}

pub(crate) async fn mark_logged_into_modrinth(
    state: &crate::State,
) -> crate::Result<()> {
    mark(state, OnboardingChecklistItem::LoggedIntoModrinth).await
}

async fn mark(
    state: &State,
    item: OnboardingChecklistItem,
) -> crate::Result<()> {
    if let Some(checklist) =
        mark_onboarding_checklist_item(item, &state.pool).await?
    {
        emit_onboarding_checklist(&state.events, checklist).await?;
    }

    Ok(())
}
