use std::{path::PathBuf, sync::Arc};

use axum::{Router, extract::State};
use contracts::{
    ApiError, Json,
    server::{Req, route},
};
use engine::{Engine, pack::import, routes::api_error};

use crate::server::{
    AppState,
    endpoints::import::{
        CheckImportableInstance, GetDefaultLauncherPath, ImportInstance,
        ImportJob, ListImportableInstances,
    },
};

pub fn routes() -> Router<AppState> {
    [
        route::<ListImportableInstances, _, _, _>(list_importable_instances),
        route::<CheckImportableInstance, _, _, _>(check_importable_instance),
        route::<GetDefaultLauncherPath, _, _, _>(get_default_launcher_path),
        route::<ImportInstance, _, _, _>(import_instance),
    ]
    .into_iter()
    .fold(Router::new(), |router, (path, method)| {
        router.route(path, method)
    })
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
    State(engine): State<Arc<Engine>>,
    Req { body, .. }: Req<ImportInstance>,
) -> Result<Json<ImportJob>, ApiError> {
    engine::install::import_instance(
        body.launcher_type,
        body.base_path,
        body.instance_folder,
        &engine,
    )
    .await
    .map(|job| Json(ImportJob(job)))
    .map_err(api_error)
}
