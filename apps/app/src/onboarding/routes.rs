use std::sync::Arc;

use axum::{Router, extract::State};
use contracts::{
    ApiError, Empty, Json,
    server::{Req, route},
};
use engine::{Engine, routes::api_error};

use crate::server::{
    AppState,
    endpoints::onboarding::{
        GetOnboardingChecklist, OnboardingProgress,
        SkipCreateInstanceOnboarding,
    },
};

pub fn routes() -> Router<AppState> {
    let (get_path, get) =
        route::<GetOnboardingChecklist, _, _, _>(get_onboarding_checklist);
    let (skip_path, skip) = route::<SkipCreateInstanceOnboarding, _, _, _>(
        skip_create_instance_onboarding,
    );
    Router::new().route(get_path, get).route(skip_path, skip)
}

async fn get_onboarding_checklist(
    State(engine): State<Arc<Engine>>,
    _: Req<GetOnboardingChecklist>,
) -> Result<Json<OnboardingProgress>, ApiError> {
    super::get(&engine)
        .await
        .map(|checklist| Json(OnboardingProgress(checklist)))
        .map_err(api_error)
}

async fn skip_create_instance_onboarding(
    State(engine): State<Arc<Engine>>,
    _: Req<SkipCreateInstanceOnboarding>,
) -> Result<Empty, ApiError> {
    super::mark_created_instance(&engine)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}
