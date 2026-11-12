use crate::state::{InstanceMetadata, State};

#[tracing::instrument(skip(state))]
pub async fn get(
    state: &State,
    instance_id: &str,
) -> crate::Result<Option<InstanceMetadata>> {
    crate::state::get_instance(instance_id, &state.pool).await
}

#[tracing::instrument(skip(state))]
pub async fn get_many(
    state: &State,
    instance_ids: &[&str],
) -> crate::Result<Vec<InstanceMetadata>> {
    crate::state::get_instances_metadata(instance_ids, &state.pool).await
}

#[tracing::instrument(skip(state))]
pub async fn list(state: &State) -> crate::Result<Vec<InstanceMetadata>> {
    crate::state::list_instances(&state.pool).await
}
