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
    state::{
        CacheValue, CachedEntry,
        tests::{add_instance, engine},
    },
};

/// Grants up to `granted` on everything, and denies the rest with 403.
struct FakeAccess {
    granted: Option<Permission>,
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
            match self.granted {
                Some(granted) if permission <= granted => Ok(permission),
                _ => Err(ApiError::new(ErrorCode::Forbidden, "denied")),
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
    host_granting(dir, allow.then_some(Permission::Owner)).await
}

async fn host_granting(
    dir: &tempfile::TempDir,
    granted: Option<Permission>,
) -> Router {
    crate::install_crypto_provider();
    let engine = engine(dir.path().to_path_buf()).await;
    add_instance(&engine, "local:a", "Survival").await;
    super::routes().with_state(Host {
        engine,
        access: Arc::new(FakeAccess { granted }),
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
async fn a_denied_caller_sees_no_instances_and_touches_no_groups_or_store() {
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
    for (method, uri, body) in [
        ("GET", "/content-store/usage", None),
        ("POST", "/content-store/cleanup", None),
        ("PUT", "/content-store/cache-limit", Some(json!(0))),
        ("GET", "/tags/loaders", None),
        ("GET", "/metadata/loaders/fabric", None),
        ("GET", "/cache/projects/sodium", None),
        ("POST", "/cache/purge", Some(json!(["project"]))),
    ] {
        let (status, _) = send(&router, method, uri, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn worlds_and_servers_are_read_and_edited_through_their_routes() {
    let dir = tempfile::tempdir().unwrap();
    let router = host(&dir, true).await;
    let save = dir
        .path()
        .join(crate::INSTANCES_FOLDER_NAME)
        .join("Survival/saves/World1");
    std::fs::create_dir_all(&save).unwrap();
    let mut data = quartz_nbt::NbtCompound::new();
    data.insert("LevelName", "Old");
    let mut root = quartz_nbt::NbtCompound::new();
    root.insert("Data", data);
    let mut level = Vec::new();
    quartz_nbt::io::write_nbt(
        &mut level,
        None,
        &root,
        quartz_nbt::io::Flavor::GzCompressed,
    )
    .unwrap();
    std::fs::write(save.join("level.dat"), level).unwrap();

    let (status, index) = send(
        &router,
        "POST",
        "/instances/local%3Aa/servers",
        Some(json!({
            "name": "Hub",
            "address": "hub.example.com",
            "pack_status": "prompt",
            "project_id": null,
            "content_kind": null,
        })),
    )
    .await;
    assert_eq!((status, index), (StatusCode::OK, json!(0)));
    let (status, _) = send(
        &router,
        "PUT",
        "/instances/local%3Aa/servers/0",
        Some(json!({
            "name": "Lobby",
            "address": "hub.example.com",
            "pack_status": "enabled",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = send(
        &router,
        "PUT",
        "/instances/local%3Aa/worlds/display-status",
        Some(json!({
            "world_type": "server",
            "world_id": "hub.example.com",
            "display_status": "hidden",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = send(
        &router,
        "PUT",
        "/instances/local%3Aa/saves/World1/name",
        Some(json!({ "name": "New" })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, world) =
        send(&router, "GET", "/instances/local%3Aa/saves/World1", None).await;
    assert_eq!(world["name"], "New");
    let (_, worlds) =
        send(&router, "GET", "/instances/local%3Aa/worlds", None).await;
    let server = worlds
        .as_array()
        .unwrap()
        .iter()
        .find(|world| world["type"] == "server")
        .unwrap();
    assert_eq!(server["name"], "Lobby");
    assert_eq!(server["pack_status"], "enabled");
    assert_eq!(server["display_status"], "hidden");

    let (_, shown) = send(
        &router,
        "GET",
        "/worlds/recent?limit=10&display_statuses=%5B%22normal%22%2C%22favorite%22%5D",
        None,
    )
    .await;
    assert_eq!(shown, json!([world_with_instance(&world)]));
    let (_, all) = send(&router, "GET", "/worlds/recent?limit=10", None).await;
    assert_eq!(all.as_array().unwrap().len(), 2);

    let (status, _) =
        send(&router, "DELETE", "/instances/local%3Aa/servers/0", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, worlds) =
        send(&router, "GET", "/instances/local%3Aa/worlds", None).await;
    assert_eq!(worlds, json!([world]));
}

#[tokio::test(flavor = "multi_thread")]
async fn world_routes_refuse_a_folder_outside_saves() {
    let dir = tempfile::tempdir().unwrap();
    let router = host(&dir, true).await;
    let instance = dir
        .path()
        .join(crate::INSTANCES_FOLDER_NAME)
        .join("Survival");
    std::fs::create_dir_all(instance.join("saves/World1")).unwrap();
    let outside = instance.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("keep.txt"), "").unwrap();
    let absolute = outside
        .to_string_lossy()
        .replace('%', "%25")
        .replace(['/', '\\'], "%2F")
        .replace(':', "%3A")
        .replace(' ', "%20");

    for world in ["World1%2F..%2F..%2Foutside", &absolute] {
        let (status, error) = send(
            &router,
            "DELETE",
            &format!("/instances/local%3Aa/saves/{world}"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{world}");
        assert_eq!(error["error"], "invalid_input");
    }
    assert!(outside.join("keep.txt").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn desyncing_a_server_needs_owner_over_the_engine() {
    let dir = tempfile::tempdir().unwrap();
    let router = host_granting(&dir, Some(Permission::Edit)).await;

    let (status, _) = send(
        &router,
        "POST",
        "/instances/local%3Aa/servers/desync",
        Some(json!({
            "server_id": "synced",
            "mode": "remove_from_other_instances",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

fn world_with_instance(world: &Value) -> Value {
    let mut world = world.clone();
    world["instance_id"] = json!("local:a");
    world
}

#[tokio::test(flavor = "multi_thread")]
async fn cache_reads_take_the_helpers_query_names() {
    crate::install_crypto_provider();
    let dir = tempfile::tempdir().unwrap();
    let engine = engine(dir.path().to_path_buf()).await;
    let user = serde_json::from_value(json!({
        "id": "u1",
        "username": "Ilai",
        "avatar_url": null,
        "bio": null,
        "created": "2024-01-01T00:00:00Z",
        "role": "developer",
    }))
    .unwrap();
    CachedEntry::upsert_many(
        [&CacheValue::User(user).get_entry()],
        &engine.pool,
    )
    .await
    .unwrap();
    let router = super::routes().with_state(Host {
        engine,
        access: Arc::new(FakeAccess { allow: true }),
    });

    let (status, user) = send(
        &router,
        "GET",
        "/cache/users/u1?cacheBehaviour=must_revalidate",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(user["username"], "Ilai");
    let (status, users) = send(
        &router,
        "GET",
        "/cache/users?ids=%5B%22u1%22%5D&cacheBehaviour=stale_while_revalidate",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(users[0]["id"], "u1");
}
