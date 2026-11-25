use contracts::{Empty, Json};

use crate::onboarding::state::OnboardingChecklist;

contracts::endpoints! {
    group onboarding;

    GetOnboardingChecklist: Get "/app/onboarding/checklist", None, (), (), Empty => Json<OnboardingChecklist>;
    /// Checks off creating an instance without creating one.
    SkipCreateInstanceOnboarding: Post "/app/onboarding/checklist/skip-create-instance", None, (), (), Empty => Empty;
}
