use axum::Router;
use contracts::{
    ApiError, Json,
    server::{Mount, Req},
};
use engine::{jre, routes::api_error};

use crate::local_api::endpoints::desktop::{GetJavaAt, JavaAtPath, TestJavaAt};

pub fn routes() -> Router {
    Router::new()
        .endpoint::<GetJavaAt, _, _>(get_java_at)
        .endpoint::<TestJavaAt, _, _>(test_java_at)
}

async fn get_java_at(
    Req { query, .. }: Req<GetJavaAt>,
) -> Result<Json<JavaAtPath>, ApiError> {
    jre::check_jre(query.path)
        .await
        .map(|java| Json(JavaAtPath(java)))
        .map_err(api_error)
}

async fn test_java_at(
    Req { query, .. }: Req<TestJavaAt>,
) -> Result<Json<bool>, ApiError> {
    jre::test_jre(query.path, query.major_version)
        .await
        .map(Json)
        .map_err(api_error)
}
