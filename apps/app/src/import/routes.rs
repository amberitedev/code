use crate::state::Desktop;
use std::path::PathBuf;

use axum::{Router, extract::State};
use contracts::{
    ApiError, Json,
    server::{Mount, Req},
};
use engine::{pack::import, routes::api_error};

use crate::local_api::{
    AppState,
    endpoints::import::{
        CheckImportableInstance, GetDefaultLauncherPath, ImportInstance,
        ImportJob, ListImportableInstances,
    },
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .endpoint::<ListImportableInstances, _, _>(list_importable_instances)
        .endpoint::<CheckImportableInstance, _, _>(check_importable_instance)
        .endpoint::<GetDefaultLauncherPath, _, _>(get_default_launcher_path)
        .endpoint::<ImportInstance, _, _>(import_instance)
}

async fn list_importable_instances(
    Req { query, .. }: Req<ListImportableInstances>,
) -> Result<Json<Vec<String>>, ApiError> {
    import::get_importable_instances(query.launcher_type, query.base_path)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn check_importable_instance(
    Req { query, .. }: Req<CheckImportableInstance>,
) -> Json<bool> {
    Json(
        import::is_valid_importable_instance(
            query.instance_folder,
            query.launcher_type,
        )
        .await,
    )
}

async fn get_default_launcher_path(
    Req { query, .. }: Req<GetDefaultLauncherPath>,
) -> Json<Option<PathBuf>> {
    Json(import::get_default_launcher_path(query.launcher_type))
}

async fn import_instance(
    State(engine): State<Desktop>,
    Req { body, .. }: Req<ImportInstance>,
) -> Result<Json<ImportJob>, ApiError> {
    engine::install::import_instance(
        body.launcher_type,
        body.base_path,
        body.instance_folder,
        engine.engine(),
    )
    .await
    .map(|job| Json(ImportJob(job)))
    .map_err(api_error)
}
