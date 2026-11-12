use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    extract::FromRef,
    http::{Request, StatusCode, request::Parts},
};
use contracts::{ApiError, ErrorCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::{
    Engine,
    access::{Access, BoxFuture, Caller, Permission},
    state::tests::{add_instance, engine},
};

/// Allows everything, or denies everything with 403.
struct FakeAccess {
    allow: bool,
}

impl Access for FakeAccess {
    fn caller<'a>(
        &'a self,
        _: &'a Parts,
    ) -> BoxFuture<'a, Result<Caller, ApiError>> {
        Box::pin(async { Ok(Caller::Local) })
    }

    fn require<'a>(
        &'a self,
        _: &'a Caller,
        _: Option<&'a str>,
        permission: Permission,
    ) -> BoxFuture<'a, Result<Permission, ApiError>> {
        Box::pin(async move {
            if self.allow {
                Ok(permission)
            } else {
                Err(ApiError::new(ErrorCode::Forbidden, "denied"))
            }
        })
    }
}

#[derive(Clone)]
struct Host {
    engine: Arc<Engine>,
    access: Arc<dyn Access>,
}

impl FromRef<Host> for Arc<Engine> {
    fn from_ref(host: &Host) -> Self {
        Arc::clone(&host.engine)
    }
}

impl FromRef<Host> for Arc<dyn Access> {
    fn from_ref(host: &Host) -> Self {
        Arc::clone(&host.access)
    }
}

async fn send(
    router: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let request = Request::builder().method(method).uri(uri);
    let request = match body {
        Some(body) => request
            .header("content-type", "application/json")
            .body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    }
    .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn host(dir: &tempfile::TempDir, allow: bool) -> Router {
    crate::install_crypto_provider();
    let engine = engine(dir.path().to_path_buf()).await;
    add_instance(&engine, "local:a", "Survival").await;
    super::routes().with_state(Host {
        engine,
        access: Arc::new(FakeAccess { allow }),
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn the_library_reads_and_groups_instances_in_the_helpers_shapes() {
    let dir = tempfile::tempdir().unwrap();
    let router = host(&dir, true).await;

    let (status, group) = send(
        &router,
        "POST",
        "/instance-groups",
        Some(json!({ "name": "Modded" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(group["name"], "Modded");
    let group_id = group["id"].as_str().unwrap();

    let (status, _) = send(
        &router,
        "PATCH",
        &format!("/instance-groups/{group_id}"),
        Some(json!({ "name": "Heavy" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(
        &router,
        "PUT",
        "/instance-groups/memberships",
        Some(json!([{ "instance_id": "local:a", "group_ids": [group_id] }])),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, groups) = send(&router, "GET", "/instance-groups", None).await;
    assert!(
        groups
            .as_array()
            .unwrap()
            .contains(&json!({ "id": group_id, "name": "Heavy" }))
    );
    let (_, instance) =
        send(&router, "GET", "/instances/local%3Aa", None).await;
    assert_eq!(instance["path"], "Survival");
    assert_eq!(instance["group_ids"], json!([group_id]));
    let (_, listed) = send(&router, "GET", "/instances", None).await;
    assert_eq!(listed[0]["id"], "local:a");
    let (_, many) = send(
        &router,
        "GET",
        "/instances/many?ids=%5B%22local%3Aa%22%2C%22local%3Ab%22%5D",
        None,
    )
    .await;
    assert_eq!(many.as_array().unwrap().len(), 1);
    let (_, missing) = send(&router, "GET", "/instances/local%3Ab", None).await;
    assert_eq!(missing, Value::Null);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_denied_caller_sees_no_instances_and_cannot_touch_groups() {
    let dir = tempfile::tempdir().unwrap();
    let router = host(&dir, false).await;

    let (status, error) =
        send(&router, "GET", "/instances/local%3Aa", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(error["error"], "forbidden");
    assert_eq!(
        send(&router, "GET", "/instances", None).await,
        (StatusCode::OK, json!([]))
    );
    let (status, _) = send(
        &router,
        "POST",
        "/instance-groups",
        Some(json!({ "name": "Modded" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
