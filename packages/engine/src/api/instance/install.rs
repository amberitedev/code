use crate::state::{Engine, JavaVersion};
use anyhow::Context;

pub async fn get_optimal_jre_key(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<Option<JavaVersion>> {
    let context =
        crate::state::instances::commands::get_instance_launch_context(
            instance_id,
            &state.pool,
        )
        .await?
        .context(format!(
            "tried to resolve a nonexistent instance {instance_id}"
        ))?;
    let (minecraft, version_index) =
        crate::launcher::resolve_minecraft_manifest(
            &context.applied_content_set.game_version,
            state,
        )
        .await?;
    let version = &minecraft.versions[version_index];
    let loader_version = crate::launcher::get_loader_version_from_profile(
        state,
        &context.applied_content_set.game_version,
        context.applied_content_set.loader,
        context.applied_content_set.loader_version.as_deref(),
        &state.directories,
    )
    .await?;
    let version_info = crate::launcher::download::download_version_info(
        state,
        version,
        loader_version.as_ref(),
        None,
        None,
        None,
    )
    .await?;

    crate::launcher::get_java_version_from_launch_context(
        &context,
        &version_info,
        &state.pool,
    )
    .await
}
