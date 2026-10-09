//! Placeholder. The delivery branch replaces this with the loop that polls the sharing backend
//! for newer instance versions and calls `server_source_service::request_source_update`.

use std::sync::Arc;

use crate::application::state::AppState;

pub async fn run(_state: Arc<AppState>) {}
