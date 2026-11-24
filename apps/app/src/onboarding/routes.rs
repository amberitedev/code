use axum::{Router, extract::State};
use contracts::{
    ApiError, Empty, Json,
    server::{Mount, Req},
};
use engine::routes::api_error;

use super::state::OnboardingChecklist;
use crate::local_api::{
    AppState,
    endpoints::onboarding::{
        GetOnboardingChecklist, SkipCreateInstanceOnboarding,
    },
};
use crate::state::Desktop;

pub fn routes() -> Router<AppState> {
    Router::new()
        .endpoint::<GetOnboardingChecklist, _, _>(get_onboarding_checklist)
        .endpoint::<SkipCreateInstanceOnboarding, _, _>(
            skip_create_instance_onboarding,
        )
}

async fn get_onboarding_checklist(
    State(engine): State<Desktop>,
    _: Req<GetOnboardingChecklist>,
) -> Result<Json<OnboardingChecklist>, ApiError> {
    super::get(&engine).await.map(Json).map_err(api_error)
}

async fn skip_create_instance_onboarding(
    State(engine): State<Desktop>,
    _: Req<SkipCreateInstanceOnboarding>,
) -> Result<Empty, ApiError> {
    super::mark_created_instance(&engine)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}
