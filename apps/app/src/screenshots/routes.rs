//! Serves `crate::local_api::endpoints::screenshots` over the engine crate's screenshot code.

use crate::state::Desktop;

use axum::{
    Router,
    extract::{DefaultBodyLimit, State},
    handler::Handler,
};
use contracts::{
    ApiError, Empty, Json,
    server::{Mount, Req},
};
use engine::routes::api_error;

use super::{ScreenshotGroup, ScreenshotKey};

use crate::local_api::{
    AppState,
    endpoints::screenshots::{
        CreateScreenshotGroup, DeleteScreenshotGroup, DeleteScreenshots,
        ExportScreenshots, ImportScreenshotGroups, ListInstanceScreenshots,
        ListScreenshotGroups, ListScreenshots, ListSyncedScreenshots,
        MoveScreenshots, RenameScreenshotGroup, SaveEditedScreenshot,
        ScreenshotFile, SetScreenshotGroupMemberships,
    },
};

type Answer<T> = Result<T, ApiError>;

pub fn routes() -> Router<AppState> {
    Router::new()
        .endpoint::<ListInstanceScreenshots, _, _>(list_instance_screenshots)
        .endpoint::<ListScreenshots, _, _>(list_screenshots)
        .endpoint::<ListSyncedScreenshots, _, _>(list_synced_screenshots)
        .endpoint::<DeleteScreenshots, _, _>(delete_screenshots)
        .endpoint::<ExportScreenshots, _, _>(export_screenshots)
        .endpoint::<MoveScreenshots, _, _>(move_screenshots)
        .endpoint::<ListScreenshotGroups, _, _>(list_screenshot_groups)
        .endpoint::<CreateScreenshotGroup, _, _>(create_screenshot_group)
        .endpoint::<RenameScreenshotGroup, _, _>(rename_screenshot_group)
        .endpoint::<DeleteScreenshotGroup, _, _>(delete_screenshot_group)
        .endpoint::<SetScreenshotGroupMemberships, _, _>(
            set_screenshot_group_memberships,
        )
        .endpoint::<ImportScreenshotGroups, _, _>(import_screenshot_groups)
        // A full-size PNG is far past axum's 2 MB default.
        .endpoint::<SaveEditedScreenshot, _, _>(
            save_edited_screenshot.layer(DefaultBodyLimit::disable()),
        )
}

fn files(
    screenshots: Vec<crate::screenshots::InstanceScreenshot>,
) -> Json<Vec<ScreenshotFile>> {
    Json(screenshots.into_iter().map(ScreenshotFile::from).collect())
}

async fn list_instance_screenshots(
    State(engine): State<Desktop>,
    Req { params, .. }: Req<ListInstanceScreenshots>,
) -> Answer<Json<Vec<ScreenshotFile>>> {
    crate::screenshots::list_screenshots(&engine, &params.id)
        .await
        .map(files)
        .map_err(api_error)
}

async fn list_screenshots(
    State(engine): State<Desktop>,
    _: Req<ListScreenshots>,
) -> Answer<Json<Vec<ScreenshotFile>>> {
    crate::screenshots::list_all_screenshots(&engine)
        .await
        .map(files)
        .map_err(api_error)
}

async fn list_synced_screenshots(
    State(engine): State<Desktop>,
    _: Req<ListSyncedScreenshots>,
) -> Answer<Json<Vec<ScreenshotFile>>> {
    crate::screenshots::list_synced_screenshots(&engine)
        .await
        .map(files)
        .map_err(api_error)
}

async fn save_edited_screenshot(
    State(engine): State<Desktop>,
    Req { query, body, .. }: Req<SaveEditedScreenshot>,
) -> Answer<Json<ScreenshotFile>> {
    let key = ScreenshotKey {
        instance_id: query.instance_id,
        file_name: query.file_name,
    };
    crate::screenshots::save_edited_screenshot(
        &engine,
        key,
        body.bytes.into(),
        query.mode,
    )
    .await
    .map(|screenshot| Json(screenshot.into()))
    .map_err(api_error)
}

async fn delete_screenshots(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<DeleteScreenshots>,
) -> Answer<Empty> {
    crate::screenshots::delete_screenshots(&engine, &body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn export_screenshots(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<ExportScreenshots>,
) -> Answer<Empty> {
    crate::screenshots::export_screenshots(
        &engine,
        &body.keys,
        body.export_path,
    )
    .await
    .map(|()| Empty)
    .map_err(api_error)
}

async fn move_screenshots(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<MoveScreenshots>,
) -> Answer<Json<Vec<ScreenshotKey>>> {
    crate::screenshots::move_screenshots(
        &engine,
        &body.keys,
        &body.target_instance_id,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

async fn list_screenshot_groups(
    State(engine): State<Desktop>,
    _: Req<ListScreenshotGroups>,
) -> Answer<Json<Vec<ScreenshotGroup>>> {
    crate::screenshots::list_screenshot_groups(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn create_screenshot_group(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<CreateScreenshotGroup>,
) -> Answer<Json<ScreenshotGroup>> {
    crate::screenshots::create_screenshot_group(
        &engine,
        body.name,
        body.screenshot_ids,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

async fn rename_screenshot_group(
    State(engine): State<Desktop>,
    Req { params, body, .. }: Req<RenameScreenshotGroup>,
) -> Answer<Json<ScreenshotGroup>> {
    crate::screenshots::rename_screenshot_group(&engine, params.id, body.name)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn delete_screenshot_group(
    State(engine): State<Desktop>,
    Req { params, .. }: Req<DeleteScreenshotGroup>,
) -> Answer<Empty> {
    crate::screenshots::delete_screenshot_group(&engine, params.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn set_screenshot_group_memberships(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<SetScreenshotGroupMemberships>,
) -> Answer<Empty> {
    crate::screenshots::set_screenshot_group_memberships(&engine, body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn import_screenshot_groups(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<ImportScreenshotGroups>,
) -> Answer<Empty> {
    crate::screenshots::import_screenshot_groups(&engine, body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}
