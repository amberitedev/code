use crate::local_api::events::AppEvent;
use crate::state::Desktop;

use super::state::{
    OnboardingChecklist, OnboardingChecklistItem, get_onboarding_checklist,
    mark_onboarding_checklist_item,
};

#[tracing::instrument(skip(state))]
pub async fn get(state: &Desktop) -> engine::Result<OnboardingChecklist> {
    get_onboarding_checklist(state.db()).await
}

#[tracing::instrument(skip(state))]
pub async fn mark_created_instance(state: &Desktop) -> engine::Result<()> {
    mark(state, OnboardingChecklistItem::CreatedInstance).await
}

pub async fn mark_logged_into_minecraft(state: &Desktop) -> engine::Result<()> {
    mark(state, OnboardingChecklistItem::LoggedIntoMinecraft).await
}

pub async fn mark_logged_into_modrinth(state: &Desktop) -> engine::Result<()> {
    mark(state, OnboardingChecklistItem::LoggedIntoModrinth).await
}

async fn mark(
    state: &Desktop,
    item: OnboardingChecklistItem,
) -> engine::Result<()> {
    if let Some(checklist) =
        mark_onboarding_checklist_item(item, state.db()).await?
    {
        state.emit(AppEvent::OnboardingChecklist(checklist));
    }

    Ok(())
}
