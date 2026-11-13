use contracts::{Auth, Empty, Endpoint, Json, Method};
use engine::data::OnboardingChecklist;
use serde::Serialize;

pub struct GetOnboardingChecklist;

impl Endpoint for GetOnboardingChecklist {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/app/onboarding/checklist";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Empty;
    type Response = Json<OnboardingProgress>;
}

/// Checks off creating an instance without creating one.
pub struct SkipCreateInstanceOnboarding;

impl Endpoint for SkipCreateInstanceOnboarding {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/app/onboarding/checklist/skip-create-instance";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Empty;
    type Response = Empty;
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct OnboardingProgress(
    #[cfg_attr(test, ts(type = "OnboardingChecklist"))] pub OnboardingChecklist,
);
