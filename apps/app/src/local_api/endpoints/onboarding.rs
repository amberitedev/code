use contracts::{Empty, Json};
use engine::data::OnboardingChecklist;
use serde::Serialize;

contracts::endpoints! {
    group onboarding;

    GetOnboardingChecklist: Get "/app/onboarding/checklist", None, (), (), Empty => Json<OnboardingProgress>;
    /// Checks off creating an instance without creating one.
    SkipCreateInstanceOnboarding: Post "/app/onboarding/checklist/skip-create-instance", None, (), (), Empty => Empty;
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct OnboardingProgress(
    #[cfg_attr(feature = "export-ts", ts(type = "OnboardingChecklist"))]
    pub  OnboardingChecklist,
);
