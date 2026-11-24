//! Serves `crate::local_api::endpoints::{game_options, synced_options, synced_packs}` over the
//! engine crate's synced-options code.

use crate::state::Desktop;
use std::collections::HashMap;

use axum::{Router, extract::State};
use contracts::{
    ApiError, Empty, Json,
    server::{Mount, Req},
};
use engine::{data::InstanceSyncedOptions, routes::api_error};

use super::{
    GameOptionsSourceCandidate, GameSettingLocaleLabels,
    GameSettingsEditorState, GlobalSyncedOptions, PackSyncPreview,
    SaveGameSettingsResult, SyncedOptionJoinPreview, SyncedOptionsOverview,
    SyncedServer,
};

use crate::local_api::{
    AppState,
    endpoints::{
        game_options::{
            GetGameSettingLabels, GetInstanceGameOptions, GetSyncedGameOptions,
            ListGameOptionsSources, PreviewInstanceGameOptions,
            PreviewSyncedGameOptions, SaveInstanceGameOptions,
            SaveSyncedGameOptions,
        },
        synced_options::{
            GetCommandHistory, GetGlobalSyncedOptions,
            GetInitializedSyncedOptions, GetInstanceSyncedOptions,
            ListInstancesSyncedOptions, ListSyncedServers,
            PreviewSyncedOptionJoin, RebuildSyncedOptions, RemoveSyncedServer,
            SetCommandHistory, SetGlobalSyncedOption, SetInstanceSyncedOption,
            UpdateSyncedServer,
        },
        synced_packs::{
            DesyncPack, ListSyncedPacks, PreviewPackSync, RemoveSyncedPack,
            SetSyncedPackEnabled, SyncPack, SyncedPacks, UploadSyncedPack,
        },
    },
};

type Answer<T> = Result<T, ApiError>;

pub fn routes() -> Router<AppState> {
    Router::new()
        .endpoint::<ListGameOptionsSources, _, _>(list_game_options_sources)
        .endpoint::<GetSyncedGameOptions, _, _>(get_synced_game_options)
        .endpoint::<PreviewSyncedGameOptions, _, _>(preview_synced_game_options)
        .endpoint::<SaveSyncedGameOptions, _, _>(save_synced_game_options)
        .endpoint::<GetInstanceGameOptions, _, _>(get_instance_game_options)
        .endpoint::<PreviewInstanceGameOptions, _, _>(
            preview_instance_game_options,
        )
        .endpoint::<SaveInstanceGameOptions, _, _>(save_instance_game_options)
        .endpoint::<GetGameSettingLabels, _, _>(get_game_setting_labels)
        .endpoint::<SetInstanceSyncedOption, _, _>(set_instance_synced_option)
        .endpoint::<PreviewSyncedOptionJoin, _, _>(preview_synced_option_join)
        .endpoint::<GetInstanceSyncedOptions, _, _>(get_instance_synced_options)
        .endpoint::<ListInstancesSyncedOptions, _, _>(
            list_instances_synced_options,
        )
        .endpoint::<GetGlobalSyncedOptions, _, _>(get_global_synced_options)
        .endpoint::<GetInitializedSyncedOptions, _, _>(
            get_initialized_synced_options,
        )
        .endpoint::<SetGlobalSyncedOption, _, _>(set_global_synced_option)
        .endpoint::<RebuildSyncedOptions, _, _>(rebuild_synced_options)
        .endpoint::<GetCommandHistory, _, _>(get_command_history)
        .endpoint::<SetCommandHistory, _, _>(set_command_history)
        .endpoint::<ListSyncedServers, _, _>(list_synced_servers)
        .endpoint::<UpdateSyncedServer, _, _>(update_synced_server)
        .endpoint::<RemoveSyncedServer, _, _>(remove_synced_server)
        .endpoint::<ListSyncedPacks, _, _>(list_synced_packs)
        .endpoint::<UploadSyncedPack, _, _>(upload_synced_pack)
        .endpoint::<SetSyncedPackEnabled, _, _>(set_synced_pack_enabled)
        .endpoint::<RemoveSyncedPack, _, _>(remove_synced_pack)
        .endpoint::<PreviewPackSync, _, _>(preview_pack_sync)
        .endpoint::<SyncPack, _, _>(sync_pack)
        .endpoint::<DesyncPack, _, _>(desync_pack)
}

async fn list_game_options_sources(
    State(engine): State<Desktop>,
    _: Req<ListGameOptionsSources>,
) -> Answer<Json<Vec<GameOptionsSourceCandidate>>> {
    crate::synced_options::list_game_options_sync_sources(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_synced_game_options(
    State(engine): State<Desktop>,
    _: Req<GetSyncedGameOptions>,
) -> Answer<Json<GameSettingsEditorState>> {
    crate::synced_options::get_synced_game_options_config(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn preview_synced_game_options(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<PreviewSyncedGameOptions>,
) -> Answer<Json<GameSettingsEditorState>> {
    crate::synced_options::preview_synced_game_option_changes(&engine, body)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn save_synced_game_options(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<SaveSyncedGameOptions>,
) -> Answer<Json<SaveGameSettingsResult>> {
    crate::synced_options::save_synced_game_option_changes(&engine, body)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_instance_game_options(
    State(engine): State<Desktop>,
    Req { params, .. }: Req<GetInstanceGameOptions>,
) -> Answer<Json<GameSettingsEditorState>> {
    crate::synced_options::get_local_game_options_config(&engine, &params.id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn preview_instance_game_options(
    State(engine): State<Desktop>,
    Req { params, body, .. }: Req<PreviewInstanceGameOptions>,
) -> Answer<Json<GameSettingsEditorState>> {
    crate::synced_options::preview_local_game_option_changes(
        &engine, &params.id, body,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

async fn save_instance_game_options(
    State(engine): State<Desktop>,
    Req { params, body, .. }: Req<SaveInstanceGameOptions>,
) -> Answer<Json<SaveGameSettingsResult>> {
    crate::synced_options::save_local_game_option_changes(
        &engine, &params.id, body,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

async fn get_game_setting_labels(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<GetGameSettingLabels>,
) -> Answer<Json<GameSettingLocaleLabels>> {
    crate::synced_options::get_game_setting_locale_labels(
        &engine,
        body.instance_id.as_deref(),
        &body.locale,
        body.option_ids,
        body.refresh_sources,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

async fn set_instance_synced_option(
    State(engine): State<Desktop>,
    Req { params, body, .. }: Req<SetInstanceSyncedOption>,
) -> Answer<Empty> {
    crate::synced_options::set_synced_option(
        &engine,
        &params.id,
        body.option,
        body.enabled,
        body.resolution,
    )
    .await
    .map(|_| Empty)
    .map_err(api_error)
}

async fn preview_synced_option_join(
    State(engine): State<Desktop>,
    Req { params, query, .. }: Req<PreviewSyncedOptionJoin>,
) -> Answer<Json<SyncedOptionJoinPreview>> {
    crate::synced_options::get_synced_option_join_preview(
        &engine,
        &params.id,
        query.option,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

async fn get_instance_synced_options(
    State(engine): State<Desktop>,
    Req { params, .. }: Req<GetInstanceSyncedOptions>,
) -> Answer<Json<SyncedOptionsOverview>> {
    crate::synced_options::get_synced_options_overview(&engine, &params.id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn list_instances_synced_options(
    State(engine): State<Desktop>,
    _: Req<ListInstancesSyncedOptions>,
) -> Answer<Json<HashMap<String, InstanceSyncedOptions>>> {
    crate::synced_options::state::all_instance_synced_options(engine.db())
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_global_synced_options(
    State(engine): State<Desktop>,
    _: Req<GetGlobalSyncedOptions>,
) -> Answer<Json<GlobalSyncedOptions>> {
    crate::synced_options::get_global_synced_options(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_initialized_synced_options(
    State(engine): State<Desktop>,
    _: Req<GetInitializedSyncedOptions>,
) -> Answer<Json<GlobalSyncedOptions>> {
    crate::synced_options::get_initialized_synced_options(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn set_global_synced_option(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<SetGlobalSyncedOption>,
) -> Answer<Json<GlobalSyncedOptions>> {
    crate::synced_options::set_global_synced_option(
        &engine,
        body.option,
        body.enabled,
        body.base_instance_id.as_deref(),
    )
    .await
    .map(Json)
    .map_err(api_error)
}

async fn rebuild_synced_options(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<RebuildSyncedOptions>,
) -> Answer<Empty> {
    match body.instance_id {
        Some(instance_id) => {
            crate::synced_options::reconcile_instance_synced_options(
                &engine,
                &instance_id,
            )
            .await
        }
        None => {
            crate::synced_options::reconcile_all_synced_options(&engine).await
        }
    }
    .map(|()| Empty)
    .map_err(api_error)
}

async fn get_command_history(
    State(engine): State<Desktop>,
    _: Req<GetCommandHistory>,
) -> Answer<Json<String>> {
    crate::synced_options::get_command_history(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn set_command_history(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<SetCommandHistory>,
) -> Answer<Json<String>> {
    crate::synced_options::set_command_history(&engine, &body.contents)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn list_synced_servers(
    State(engine): State<Desktop>,
    _: Req<ListSyncedServers>,
) -> Answer<Json<Vec<SyncedServer>>> {
    crate::synced_options::list_synced_servers(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn update_synced_server(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<UpdateSyncedServer>,
) -> Answer<Empty> {
    crate::synced_options::update_synced_server(&engine, body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn remove_synced_server(
    State(engine): State<Desktop>,
    Req { params, .. }: Req<RemoveSyncedServer>,
) -> Answer<Empty> {
    crate::synced_options::remove_synced_server(&engine, &params.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn list_synced_packs(
    State(engine): State<Desktop>,
    Req { query, .. }: Req<ListSyncedPacks>,
) -> Answer<Json<SyncedPacks>> {
    crate::synced_options::list_synced_packs(&engine, query.project_type.into())
        .await
        .map(|packs| Json(SyncedPacks(packs)))
        .map_err(api_error)
}

async fn upload_synced_pack(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<UploadSyncedPack>,
) -> Answer<Empty> {
    crate::synced_options::upload_synced_pack(
        &engine,
        &body.path,
        body.project_type.into(),
        body.game_versions,
    )
    .await
    .map(|()| Empty)
    .map_err(api_error)
}

async fn set_synced_pack_enabled(
    State(engine): State<Desktop>,
    Req { params, body, .. }: Req<SetSyncedPackEnabled>,
) -> Answer<Empty> {
    crate::synced_options::set_synced_pack_enabled(
        &engine,
        &params.id,
        body.enabled,
    )
    .await
    .map(|()| Empty)
    .map_err(api_error)
}

async fn remove_synced_pack(
    State(engine): State<Desktop>,
    Req { params, .. }: Req<RemoveSyncedPack>,
) -> Answer<Empty> {
    crate::synced_options::remove_synced_pack(&engine, &params.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn preview_pack_sync(
    State(engine): State<Desktop>,
    Req { params, query, .. }: Req<PreviewPackSync>,
) -> Answer<Json<PackSyncPreview>> {
    crate::synced_options::get_pack_sync_preview(
        &engine,
        &params.id,
        &query.project_path,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

async fn sync_pack(
    State(engine): State<Desktop>,
    Req { params, body, .. }: Req<SyncPack>,
) -> Answer<Empty> {
    crate::synced_options::sync_pack(&engine, &params.id, &body.project_path)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn desync_pack(
    State(engine): State<Desktop>,
    Req { params, query, .. }: Req<DesyncPack>,
) -> Answer<Empty> {
    crate::synced_options::desync_pack(
        &engine,
        &params.id,
        &params.pack_id,
        query.mode,
    )
    .await
    .map(|()| Empty)
    .map_err(api_error)
}
