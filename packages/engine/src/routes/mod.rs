//! The engine's HTTP API. Each handler is a `pub fn`, so a host can mount one itself, wrapped in
//! its own hook. A host's state gives the engine and its [`Access`].

mod groups;
mod instances;
mod loading;
#[cfg(test)]
mod tests;
mod worlds;

use std::sync::Arc;

use axum::{Router, extract::FromRef};
use contracts::{ApiError, ErrorCode, server::route};
use serde::Deserialize;

pub use self::{groups::*, instances::*, loading::*, worlds::*};
use crate::{
    Engine,
    access::Access,
    endpoints::{
        groups::{
            CreateInstanceGroup, DeleteInstanceGroup, ListInstanceGroups,
            RenameInstanceGroup, SetInstanceGroupMemberships,
            SetInstanceGroupOrder,
        },
        instances::{GetInstance, GetInstances, ListInstances},
        loading::ListLoadingBars,
        worlds::{
            AddInstanceServer, BackupWorld, DeleteWorld, DesyncInstanceServer,
            EditInstanceServer, EnsureManagedInstanceServer,
            GetInstanceProtocolVersion, GetServerStatus, GetSingleplayerWorld,
            ListInstanceWorlds, ListRecentWorlds, RemoveInstanceServer,
            RenameWorld, ResetWorldIcon, SetWorldDisplayStatus,
        },
    },
};

pub fn routes<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    Arc<Engine>: FromRef<S>,
    Arc<dyn Access>: FromRef<S>,
{
    [
        route::<ListInstances, _, _, _>(list_instances),
        route::<GetInstances, _, _, _>(get_instances),
        route::<GetInstance, _, _, _>(get_instance),
        route::<ListInstanceGroups, _, _, _>(list_instance_groups),
        route::<CreateInstanceGroup, _, _, _>(create_instance_group),
        route::<RenameInstanceGroup, _, _, _>(rename_instance_group),
        route::<DeleteInstanceGroup, _, _, _>(delete_instance_group),
        route::<SetInstanceGroupOrder, _, _, _>(set_instance_group_order),
        route::<SetInstanceGroupMemberships, _, _, _>(
            set_instance_group_memberships,
        ),
        route::<ListLoadingBars, _, _, _>(list_loading_bars),
        route::<ListRecentWorlds, _, _, _>(list_recent_worlds),
        route::<GetServerStatus, _, _, _>(get_server_status),
        route::<ListInstanceWorlds, _, _, _>(list_instance_worlds),
        route::<SetWorldDisplayStatus, _, _, _>(set_world_display_status),
        route::<GetInstanceProtocolVersion, _, _, _>(
            get_instance_protocol_version,
        ),
        route::<GetSingleplayerWorld, _, _, _>(get_singleplayer_world),
        route::<DeleteWorld, _, _, _>(delete_world),
        route::<RenameWorld, _, _, _>(rename_world),
        route::<ResetWorldIcon, _, _, _>(reset_world_icon),
        route::<BackupWorld, _, _, _>(backup_world),
        route::<AddInstanceServer, _, _, _>(add_instance_server),
        route::<EnsureManagedInstanceServer, _, _, _>(
            ensure_managed_instance_server,
        ),
        route::<DesyncInstanceServer, _, _, _>(desync_instance_server),
        route::<EditInstanceServer, _, _, _>(edit_instance_server),
        route::<RemoveInstanceServer, _, _, _>(remove_instance_server),
    ]
    .into_iter()
    .fold(Router::new(), |router, (path, method)| {
        router.route(path, method)
    })
}

/// An engine error as the route's answer, coded as the Tauri commands code it.
pub fn api_error(error: crate::Error) -> ApiError {
    tracing::warn!("{error:?}");
    let code = crate::error_code(&error);
    let code = ErrorCode::deserialize(serde::de::value::StrDeserializer::<
        serde::de::value::Error,
    >::new(code))
    .unwrap_or(ErrorCode::Unknown);
    ApiError::new(code, format!("{error:#}"))
}
