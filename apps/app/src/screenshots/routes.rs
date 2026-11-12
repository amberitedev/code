//! Serves `crate::server::endpoints::screenshots` over the engine crate's screenshot code.

use std::sync::Arc;

use axum::{Router, extract::DefaultBodyLimit, extract::State};
use contracts::{
    ApiError, Empty, Json,
    server::{Req, route},
};
use engine::{
    Engine,
    instance::{ScreenshotGroup, ScreenshotKey},
    routes::api_error,
};

use crate::server::{
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
    let (edits_path, edits) =
        route::<SaveEditedScreenshot, _, _, _>(save_edited_screenshot);
    [
        route::<ListInstanceScreenshots, _, _, _>(list_instance_screenshots),
        route::<ListScreenshots, _, _, _>(list_screenshots),
        route::<ListSyncedScreenshots, _, _, _>(list_synced_screenshots),
        route::<DeleteScreenshots, _, _, _>(delete_screenshots),
        route::<ExportScreenshots, _, _, _>(export_screenshots),
        route::<MoveScreenshots, _, _, _>(move_screenshots),
        route::<ListScreenshotGroups, _, _, _>(list_screenshot_groups),
        route::<CreateScreenshotGroup, _, _, _>(create_screenshot_group),
        route::<RenameScreenshotGroup, _, _, _>(rename_screenshot_group),
        route::<DeleteScreenshotGroup, _, _, _>(delete_screenshot_group),
        route::<SetScreenshotGroupMemberships, _, _, _>(
            set_screenshot_group_memberships,
        ),
        route::<ImportScreenshotGroups, _, _, _>(import_screenshot_groups),
        // A full-size PNG is far past axum's 2 MB default.
        (edits_path, edits.layer(DefaultBodyLimit::disable())),
    ]
    .into_iter()
    .fold(Router::new(), |router, (path, method)| {
        router.route(path, method)
    })
}

fn files(
    screenshots: Vec<engine::instance::InstanceScreenshot>,
) -> Json<Vec<ScreenshotFile>> {
    Json(screenshots.into_iter().map(ScreenshotFile::from).collect())
}

async fn list_instance_screenshots(
    State(engine): State<Arc<Engine>>,
    Req { params, .. }: Req<ListInstanceScreenshots>,
) -> Answer<Json<Vec<ScreenshotFile>>> {
    engine::instance::list_screenshots(&engine, &params.id)
        .await
        .map(files)
        .map_err(api_error)
}

async fn list_screenshots(
    State(engine): State<Arc<Engine>>,
    _: Req<ListScreenshots>,
) -> Answer<Json<Vec<ScreenshotFile>>> {
    engine::instance::list_all_screenshots(&engine)
        .await
        .map(files)
        .map_err(api_error)
}

async fn list_synced_screenshots(
    State(engine): State<Arc<Engine>>,
    _: Req<ListSyncedScreenshots>,
) -> Answer<Json<Vec<ScreenshotFile>>> {
    engine::instance::list_synced_screenshots(&engine)
        .await
        .map(files)
        .map_err(api_error)
}

async fn save_edited_screenshot(
    State(engine): State<Arc<Engine>>,
    Req { query, body, .. }: Req<SaveEditedScreenshot>,
) -> Answer<Json<ScreenshotFile>> {
    let key = ScreenshotKey {
        instance_id: query.instance_id,
        file_name: query.file_name,
    };
    engine::instance::save_edited_screenshot(
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
    State(engine): State<Arc<Engine>>,
    Req { body, .. }: Req<DeleteScreenshots>,
) -> Answer<Empty> {
    engine::instance::delete_screenshots(&engine, &body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn export_screenshots(
    State(engine): State<Arc<Engine>>,
    Req { body, .. }: Req<ExportScreenshots>,
) -> Answer<Empty> {
    engine::instance::export_screenshots(&engine, &body.keys, body.export_path)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn move_screenshots(
    State(engine): State<Arc<Engine>>,
    Req { body, .. }: Req<MoveScreenshots>,
) -> Answer<Json<Vec<ScreenshotKey>>> {
    engine::instance::move_screenshots(
        &engine,
        &body.keys,
        &body.target_instance_id,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

async fn list_screenshot_groups(
    State(engine): State<Arc<Engine>>,
    _: Req<ListScreenshotGroups>,
) -> Answer<Json<Vec<ScreenshotGroup>>> {
    engine::instance::list_screenshot_groups(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn create_screenshot_group(
    State(engine): State<Arc<Engine>>,
    Req { body, .. }: Req<CreateScreenshotGroup>,
) -> Answer<Json<ScreenshotGroup>> {
    engine::instance::create_screenshot_group(
        &engine,
        body.name,
        body.screenshot_ids,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

async fn rename_screenshot_group(
    State(engine): State<Arc<Engine>>,
    Req { params, body, .. }: Req<RenameScreenshotGroup>,
) -> Answer<Json<ScreenshotGroup>> {
    engine::instance::rename_screenshot_group(&engine, params.id, body.name)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn delete_screenshot_group(
    State(engine): State<Arc<Engine>>,
    Req { params, .. }: Req<DeleteScreenshotGroup>,
) -> Answer<Empty> {
    engine::instance::delete_screenshot_group(&engine, params.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn set_screenshot_group_memberships(
    State(engine): State<Arc<Engine>>,
    Req { body, .. }: Req<SetScreenshotGroupMemberships>,
) -> Answer<Empty> {
    engine::instance::set_screenshot_group_memberships(&engine, body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn import_screenshot_groups(
    State(engine): State<Arc<Engine>>,
    Req { body, .. }: Req<ImportScreenshotGroups>,
) -> Answer<Empty> {
    engine::instance::import_screenshot_groups(&engine, body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}
