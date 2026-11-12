use super::*;

/// The icon of the server a player came to a shared instance through. The backend keeps no
/// server icons, so there is none to cache.
#[tracing::instrument(skip(_state))]
pub async fn cache_shared_instance_server_icon(
    _state: &Engine,
    _instance_id: &str,
) -> crate::Result<Option<String>> {
    Ok(None)
}
