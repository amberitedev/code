use axum::Router;
use contracts::{
    ApiError, Json,
    server::{Req, route},
};
use engine::{jre, routes::api_error};

use crate::server::endpoints::desktop::{GetJavaAt, JavaAtPath, TestJavaAt};

pub fn routes() -> Router {
    let (get_path, get) = route::<GetJavaAt, _, _, _>(get_java_at);
    let (test_path, test) = route::<TestJavaAt, _, _, _>(test_java_at);
    Router::new().route(get_path, get).route(test_path, test)
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
