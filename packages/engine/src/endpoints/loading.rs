use std::collections::HashMap;

use contracts::{Empty, Json};
use uuid::Uuid;

use super::endpoints;
use crate::LoadingBar;

endpoints! {
    /// The loading bars open now, by id. `Loading` events carry their updates.
    ListLoadingBars: Get "/loading-bars", (), (), Empty => Json<HashMap<Uuid, LoadingBar>>;
}
