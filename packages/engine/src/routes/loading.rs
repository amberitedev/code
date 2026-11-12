use std::{collections::HashMap, sync::Arc};

use axum::extract::State;
use contracts::{Json, server::Req};
use uuid::Uuid;

use crate::{
    Engine, LoadingBar,
    access::{EngineAccess, Owner},
    endpoints::loading::ListLoadingBars,
};

pub async fn list_loading_bars(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    _: Req<ListLoadingBars>,
) -> Json<HashMap<Uuid, LoadingBar>> {
    Json(engine.events.loading_bars().into_iter().collect())
}
