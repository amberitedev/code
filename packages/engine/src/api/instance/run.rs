use super::content::get_projects;
use crate::ErrorKind;
use crate::server_address::ServerAddress;
use crate::state::{
    Engine, Hooks, InstanceLink, MemorySettings, ProcessMetadata, WindowSize,
};
use crate::util::fetch;
use crate::util::io::IOError;
use anyhow::{Context, bail};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum QuickPlayType {
    None,
    Singleplayer(String),
    Server(ServerAddress),
}

/// The Minecraft account a launch signs in as.
pub struct LaunchCredentials {
    pub username: String,
    pub uuid: Uuid,
    pub access_token: String,
}

/// The host's launch settings, used where the instance sets no override.
pub struct LaunchDefaults {
    pub extra_launch_args: Vec<String>,
    pub custom_env_vars: Vec<(String, String)>,
    pub memory: MemorySettings,
    pub game_resolution: WindowSize,
    pub hooks: Hooks,
}

pub struct LaunchRequest {
    pub credentials: LaunchCredentials,
    pub defaults: LaunchDefaults,
    pub quick_play: QuickPlayType,
}

/// Launches the instance. `before_spawn` runs where the host may still change the instance's
/// files: after every check, the pre-launch hook and any repair install, before the game starts.
#[tracing::instrument(skip(state, request, before_spawn))]
pub async fn launch(
    state: &Arc<Engine>,
    instance_id: &str,
    request: LaunchRequest,
    before_spawn: impl AsyncFnOnce() -> crate::Result<()>,
) -> crate::Result<ProcessMetadata> {
    let LaunchRequest {
        credentials,
        defaults,
        quick_play,
    } = request;
    let context =
        crate::state::instances::commands::get_instance_launch_context(
            instance_id,
            &state.pool,
        )
        .await?
        .context(format!(
            "tried to run a nonexistent instance {instance_id}"
        ))?;
    if crate::state::instances::adapters::sqlite::instance_rows::is_instance_quarantined(
        instance_id,
        &state.pool,
    )
    .await?
    {
        bail!(ErrorKind::InputError(
            "this instance has been quarantined".to_string(),
        ));
    }
    super::shared::check_shared_instance_availability_before_launch(
        instance_id,
        state,
    )
    .await?;

    let pre_launch_hook = context
        .launch_overrides
        .hooks
        .pre_launch
        .as_ref()
        .or(defaults.hooks.pre_launch.as_ref())
        .filter(|hook_command| !hook_command.is_empty());

    let java_args = context
        .launch_overrides
        .extra_launch_args
        .clone()
        .unwrap_or(defaults.extra_launch_args);

    let wrapper = context
        .launch_overrides
        .hooks
        .wrapper
        .clone()
        .or(defaults.hooks.wrapper)
        .filter(|hook_command| !hook_command.is_empty());

    let env_args = context
        .launch_overrides
        .custom_env_vars
        .clone()
        .unwrap_or(defaults.custom_env_vars);

    let post_exit_hook = context
        .launch_overrides
        .hooks
        .post_exit
        .clone()
        .or(defaults.hooks.post_exit)
        .filter(|hook_command| !hook_command.is_empty());

    let memory = context.launch_overrides.memory.unwrap_or(defaults.memory);
    let resolution = context
        .launch_overrides
        .game_resolution
        .unwrap_or(defaults.game_resolution);
    let has_hook_commands = pre_launch_hook.is_some()
        || wrapper.is_some()
        || post_exit_hook.is_some();
    let full_path = if has_hook_commands {
        Some(crate::util::io::canonicalize(
            state
                .directories
                .instances_dir()
                .join(&context.instance.path),
        )?)
    } else {
        None
    };
    let hook_environment = if has_hook_commands {
        let full_path = full_path
            .as_ref()
            .expect("hooked launches always resolve their instance path");
        let java_version =
            crate::launcher::resolve_java_for_launch(&context, state).await?;

        Some(crate::launcher::hooks::HookEnvironment::from_current_env(
            &env_args,
            crate::launcher::hooks::HookVariables {
                instance_name: context.instance.name.clone(),
                instance_id: context.instance.path.clone(),
                instance_dir: full_path.to_string_lossy().to_string(),
                java_path: java_version.path.clone(),
                java_args: crate::launcher::hooks::build_hook_java_args(
                    &java_args,
                    memory,
                    &java_version,
                ),
            },
        ))
    } else {
        None
    };
    let launch_env_args = hook_environment
        .as_ref()
        .map_or_else(|| env_args.clone(), |env| env.injected_envs());

    if let (Some(hook), Some(hook_environment), Some(full_path)) = (
        pre_launch_hook,
        hook_environment.as_ref(),
        full_path.as_ref(),
    ) {
        let expanded_hook = hook_environment.expand(hook);
        let mut cmd = shlex::split(&expanded_hook)
            .ok_or(ErrorKind::LauncherError(format!(
                "invalid pre-launch command {hook}",
            )))?
            .into_iter();

        if let Some(command) = cmd.next() {
            let result = Command::new(command)
                .args(cmd)
                .envs(launch_env_args.iter().cloned())
                .current_dir(full_path)
                .spawn()
                .map_err(|e| IOError::with_path(e, full_path))?
                .wait()
                .await
                .map_err(IOError::from)?;

            if !result.success() {
                bail!(ErrorKind::LauncherError(format!(
                    "non-zero exit code for pre-launch hook ({})",
                    result.code().unwrap_or(-1)
                )));
            }
        }
    }

    let wrapper = wrapper
        .map(|hook| {
            hook_environment
                .as_ref()
                .map_or(hook.clone(), |env| env.expand(&hook))
        })
        .filter(|hook_command| !hook_command.is_empty());
    let post_exit_hook = post_exit_hook
        .map(|hook| {
            hook_environment
                .as_ref()
                .map_or(hook.clone(), |env| env.expand(&hook))
        })
        .filter(|hook_command| !hook_command.is_empty());

    if let Some(project_id) = server_play_project_id(&context.link)
        && !project_id.trim().is_empty()
    {
        let server_id = uuid::Uuid::new_v4().to_string();
        let join_result = fetch::INSECURE_REQWEST_CLIENT
            .post("https://sessionserver.mojang.com/session/minecraft/join")
            .json(&json!({
                "accessToken": &credentials.access_token,
                "selectedProfile": credentials.uuid.simple().to_string(),
                "serverId": &server_id,
            }))
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        match join_result {
            Ok(resp) if resp.status().is_success() => {
                let result = fetch::post_json(
                    concat!(
                        env!("MODRINTH_API_BASE_URL"),
                        "analytics/minecraft-server-play"
                    ),
                    json!({
                        "project_id": project_id,
                        "username": &credentials.username,
                        "server_id": &server_id,
                    }),
                    &state.api_semaphore,
                    &state.sessions,
                )
                .await;

                match result {
                    Ok(()) => {
                        info!(
                            "Tracked server play for '{project_id}' in analytics"
                        )
                    }
                    Err(err) => warn!("Failed to report server play: {err:?}"),
                }
            }
            Ok(resp) => warn!(
                "Failed to join Mojang session server: HTTP {}",
                resp.status()
            ),
            Err(err) => warn!("Failed to join Mojang session server: {err:?}"),
        }
    }

    crate::launcher::launch_minecraft(
        &java_args,
        &launch_env_args,
        &wrapper,
        &memory,
        &resolution,
        &credentials,
        post_exit_hook,
        &context,
        quick_play,
        before_spawn,
        state,
    )
    .await
}

fn server_play_project_id(link: &InstanceLink) -> Option<&String> {
    match link {
        InstanceLink::ServerProject { project_id }
        | InstanceLink::ServerProjectModpack {
            server_project_id: project_id,
            ..
        } => Some(project_id),
        InstanceLink::Unmanaged
        | InstanceLink::ModrinthModpack { .. }
        | InstanceLink::ModrinthHosting { .. }
        | InstanceLink::ImportedModpack { .. }
        | InstanceLink::SharedInstance { .. } => None,
    }
}

pub async fn kill(state: &Engine, instance_id: &str) -> crate::Result<()> {
    let processes =
        crate::api::process::get_by_instance_id(state, instance_id).await?;

    for process in processes {
        state.process_manager.kill(process.uuid).await?;
    }

    Ok(())
}

#[tracing::instrument(skip(state))]
pub async fn try_update_playtime_by_instance_id(
    state: &Engine,
    instance_id: &str,
) -> crate::Result<()> {
    let context =
        crate::state::instances::commands::get_instance_launch_context(
            instance_id,
            &state.pool,
        )
        .await?
        .context(format!(
            "tried to update playtime for nonexistent instance {instance_id}"
        ))?;
    let updated_recent_playtime = context.instance.recent_time_played;
    let res = if updated_recent_playtime > 0 {
        let modrinth_pack_version_id = match &context.link {
            InstanceLink::ModrinthModpack { version_id, .. }
            | InstanceLink::ServerProjectModpack {
                content_version_id: version_id,
                ..
            }
            | InstanceLink::ImportedModpack {
                version_id: Some(version_id),
                ..
            } => Some(version_id.clone()),
            InstanceLink::Unmanaged
            | InstanceLink::ServerProject { .. }
            | InstanceLink::ModrinthHosting { .. }
            | InstanceLink::ImportedModpack { .. }
            | InstanceLink::SharedInstance { .. } => None,
        };
        let playtime_update_json = json!({
            "seconds": updated_recent_playtime,
            "loader": context.applied_content_set.loader.as_str(),
            "game_version": &context.applied_content_set.game_version,
            "parent": modrinth_pack_version_id,
        });
        let mut hashmap: HashMap<String, serde_json::Value> = HashMap::new();

        for (_, project) in get_projects(state, instance_id, None).await? {
            if let Some(metadata) = project.metadata {
                hashmap
                    .insert(metadata.version_id, playtime_update_json.clone());
            }
        }

        fetch::post_json(
            concat!(env!("MODRINTH_API_BASE_URL"), "analytics/playtime"),
            serde_json::to_value(hashmap)?,
            &state.api_semaphore,
            &state.sessions,
        )
        .await
    } else {
        Ok(())
    };

    if res.is_ok() {
        crate::state::instances::commands::mark_instance_playtime_submitted(
            &context.instance.id,
            updated_recent_playtime,
            &state.pool,
        )
        .await?;
    }

    res
}
