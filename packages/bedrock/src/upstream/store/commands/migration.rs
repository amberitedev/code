// Amberite: replaces upstream's `commands/migration.rs`, which moves app-lib's old instance
// copies and app directories into the store. A host opening the store (`glue::open`) runs the
// part that applies to any host: the layout is current, and staging and unregistered files an
// interrupted run left are recovered.

use crate::State;
use crate::state::content_store::ContentStore;
use crate::state::content_store::adapters::sqlite as catalog;

pub(crate) async fn migrate(
    store: &ContentStore,
    state: &State,
) -> crate::Result<()> {
    catalog::set_setting(&state.pool, "store_layout_version", "1").await?;
    store.remove_abandoned_staging().await?;
    store.recover_unregistered_files().await?;
    Ok(())
}

/// A host without instances has no instance copies to migrate.
pub(crate) async fn migrate_instance_copies(
    _state: &State,
) -> crate::Result<()> {
    Ok(())
}
