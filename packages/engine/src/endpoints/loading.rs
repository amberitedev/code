use std::collections::HashMap;

use contracts::{Empty, Json};
use uuid::Uuid;

use crate::LoadingBar;

contracts::endpoints! {
    group loadingBars;

    /// The loading bars open now, by id. `Loading` events carry their updates.
    ListLoadingBars: Get "/loading-bars", Session, (), (), Empty => Json<HashMap<Uuid, LoadingBar>>;
}
