use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    extract::{DefaultBodyLimit, FromRef},
    http::{Request, StatusCode, request::Parts},
};
use contracts::{ApiError, ErrorCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::{
    Engine,
    access::{Access, BoxFuture, Caller},
    install::{InstallJobStatus, InstallRequest, model::InstallJobState},
    state::{
        CacheValue, CachedEntry,
        tests::{add_instance, engine, link_dir},
    },
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
    ) -> BoxFuture<'a, Result<(), ApiError>> {
        Box::pin(async move {
            match self.allow {
                true => Ok(()),
                false => Err(ApiError::new(ErrorCode::Forbidden, "denied")),
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
async fn the_library_reads_instances_in_the_helpers_shapes() {
    let dir = tempfile::tempdir().unwrap();
    let router = host(&dir, true).await;

    let (_, instance) =
        send(&router, "GET", "/instances/local%3Aa", None).await;
    assert_eq!(instance["path"], "Survival");
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
async fn a_denied_caller_sees_no_instances_and_touches_no_store() {
    let dir = tempfile::tempdir().unwrap();
    let router = host(&dir, false).await;

    let (status, error) =
        send(&router, "GET", "/instances/local%3Aa", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(error["error"], "forbidden");
    for list in ["/instances", "/processes"] {
        assert_eq!(
            send(&router, "GET", list, None).await,
            (StatusCode::OK, json!([])),
            "{list}"
        );
    }
    for (method, uri, body) in [
        ("GET", "/content-store/usage", None),
        ("POST", "/content-store/cleanup", None),
        ("PUT", "/content-store/cache-limit", Some(json!(0))),
        ("GET", "/tags/loaders", None),
        ("GET", "/metadata/loaders/fabric", None),
        ("GET", "/cache/projects/sodium", None),
        ("POST", "/cache/purge", Some(json!(["project"]))),
        ("GET", "/instances/local%3Aa/logs", None),
        ("DELETE", "/instances/local%3Aa/live-log", None),
        ("GET", "/java/versions", None),
        (
            "PUT",
            "/java/versions",
            Some(json!({
                "parsed_version": 17,
                "version": "17.0.1",
                "architecture": "x86_64",
                "path": "/bin/java",
            })),
        ),
        ("GET", "/java/installs?version=17", None),
        ("POST", "/java/installs", Some(json!(17))),
        ("GET", "/system/memory", None),
    ] {
        let (status, _) = send(&router, method, uri, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn worlds_and_servers_are_read_and_edited_through_their_routes() {
    crate::install_crypto_provider();
    let dir = tempfile::tempdir().unwrap();
    let engine = engine(dir.path().to_path_buf()).await;
    add_instance(&engine, "local:a", "Survival").await;
    let router = super::routes().with_state(Host {
        engine: Arc::clone(&engine),
        access: Arc::new(FakeAccess { allow: true }),
    });
    // The host keeps join times and lists worlds with them.
    let joined: chrono::DateTime<chrono::Utc> =
        "2026-01-02T03:04:05Z".parse().unwrap();
    let joins = crate::worlds::ServerJoins::from([(
        ("hub.example.com".to_string(), 25565),
        joined,
    )]);
    let list = async || {
        let worlds =
            crate::worlds::get_instance_worlds(&engine, "local:a", &joins)
                .await
                .unwrap();
        serde_json::to_value(worlds).unwrap()
    };
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
    let worlds = list().await;
    let server = worlds
        .as_array()
        .unwrap()
        .iter()
        .find(|world| world["type"] == "server")
        .unwrap();
    assert_eq!(server["name"], "Lobby");
    assert_eq!(server["pack_status"], "enabled");
    assert_eq!(server["display_status"], "hidden");
    assert_eq!(server["last_played"], json!(joined));

    let all_joins = std::collections::HashMap::from([(
        "local:a".to_string(),
        joins.clone(),
    )]);
    let recent = async |statuses| {
        let worlds =
            crate::worlds::get_recent_worlds(&engine, 10, statuses, &all_joins)
                .await
                .unwrap();
        serde_json::to_value(worlds).unwrap()
    };
    let shown = recent(
        crate::worlds::DisplayStatus::Normal
            | crate::worlds::DisplayStatus::Favorite,
    )
    .await;
    assert_eq!(shown, json!([world_with_instance(&world)]));
    let all = recent(enumset::EnumSet::all()).await;
    assert_eq!(all.as_array().unwrap().len(), 2);

    let (status, _) =
        send(&router, "DELETE", "/instances/local%3Aa/servers/0", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(list().await, json!([world]));
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
    // Many ids go in a body: a project's every version id overflows a request line.
    let (status, users) = send(
        &router,
        "POST",
        "/cache/users",
        Some(json!({ "ids": ["u1"], "cacheBehaviour": "stale_while_revalidate" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(users[0]["id"], "u1");
}

#[tokio::test(flavor = "multi_thread")]
async fn logs_are_read_and_deleted_by_file_name_only() {
    crate::install_crypto_provider();
    let dir = tempfile::tempdir().unwrap();
    let engine = engine(dir.path().to_path_buf()).await;
    add_instance(&engine, "local:a", "Survival").await;
    let logs_dir = engine.directories.instance_logs_dir("Survival");
    let secret = engine.directories.instances_dir().join("secret.log");
    std::fs::create_dir_all(&logs_dir).unwrap();
    std::fs::write(logs_dir.join("latest.log"), "[main] ready").unwrap();
    std::fs::write(&secret, "secret").unwrap();
    let router = super::routes().with_state(Host {
        engine,
        access: Arc::new(FakeAccess { allow: true }),
    });

    let (status, logs) = send(
        &router,
        "GET",
        "/instances/local%3Aa/logs?clearContents=true",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(logs[0]["filename"], "latest.log");
    assert_eq!(logs[0]["log_type"], "InfoLog");
    assert_eq!(logs[0]["output"], Value::Null);

    let file = "/instances/local%3Aa/logs/InfoLog/latest.log";
    let (status, output) = send(&router, "GET", file, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(output.as_str().unwrap().contains("ready"));

    let escape = "/instances/local%3Aa/logs/InfoLog/..%2F..%2Fsecret.log";
    for method in ["GET", "DELETE"] {
        let (status, error) = send(&router, method, escape, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{method}");
        assert_eq!(error["error"], "invalid_input");
    }
    assert!(secret.exists());

    let (status, _) = send(&router, "DELETE", file, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!logs_dir.join("latest.log").exists());
}

async fn send_raw(
    router: &Router,
    method: &str,
    uri: &str,
    body: Vec<u8>,
) -> (StatusCode, Vec<u8>) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::from(body))
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, bytes.to_vec())
}

fn has_file_named(dir: &std::path::Path, name: &str) -> bool {
    std::fs::read_dir(dir).unwrap().any(|entry| {
        let entry = entry.unwrap();
        entry.file_name() == name
            || (entry.file_type().unwrap().is_dir()
                && has_file_named(&entry.path(), name))
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn the_files_tab_writes_reads_moves_and_deletes_raw_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let routes = host(&dir, true).await;
    let files = "/instances/local%3Aa/files";
    let capped = routes.clone().layer(DefaultBodyLimit::max(1024));
    let (status, _) = send_raw(
        &capped,
        "PUT",
        &format!("{files}/content?path=capped.bin&create_only=false"),
        vec![7u8; 2048],
    )
    .await;
    // The host's cap holds: contracts answers a body past it as invalid input.
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // Past axum's 2 MB default, which the App lifts.
    let router = routes.layer(DefaultBodyLimit::disable());
    let bytes = vec![7u8; 3 * 1024 * 1024];

    let write = format!("{files}/content?path=config%2Fbig.bin&create_only=");
    let (status, _) =
        send_raw(&router, "PUT", &format!("{write}false"), bytes.clone()).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) =
        send_raw(&router, "PUT", &format!("{write}true"), vec![1]).await;
    assert!(!status.is_success());
    let (status, read) = send_raw(
        &router,
        "GET",
        &format!("{files}/content?path=config%2Fbig.bin"),
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(read == bytes);

    let (status, _) = send(
        &router,
        "POST",
        &format!("{files}/directories?path=notes"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = send(
        &router,
        "POST",
        &format!("{files}/move"),
        Some(json!({ "source": "config/big.bin", "destination": "notes/big.bin" })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, listed) =
        send(&router, "GET", &format!("{files}?path=notes"), None).await;
    assert_eq!(listed[0]["path"], "notes/big.bin");
    assert_eq!(listed[0]["type"], "file");
    assert_eq!(listed[0]["size"], bytes.len());

    let (status, _) = send(
        &router,
        "DELETE",
        &format!("{files}?path=notes&recursive=true"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, root) = send(&router, "GET", &format!("{files}?path="), None).await;
    let root: Vec<_> = root
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["path"].as_str().unwrap())
        .collect();
    assert!(root.contains(&"config") && !root.contains(&"notes"));
}

#[tokio::test(flavor = "multi_thread")]
async fn file_and_content_routes_refuse_paths_that_leave_the_instance() {
    let dir = tempfile::tempdir().unwrap();
    let router = host(&dir, true).await;
    let files = "/instances/local%3Aa/files";
    let (status, _) = send_raw(
        &router,
        "PUT",
        &format!("{files}/content?path=inside.txt&create_only=false"),
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    for path in [
        "..%2Fescape.txt",
        "a%2F..%2F..%2Fescape.txt",
        "C%3A%2Fescape.txt",
    ] {
        let (status, _) = send_raw(
            &router,
            "PUT",
            &format!("{files}/content?path={path}&create_only=false"),
            vec![1],
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path} was written");
        let (status, _) = send(
            &router,
            "POST",
            &format!("{files}/move"),
            Some(json!({ "source": "inside.txt", "destination": path.replace("%2F", "/").replace("%3A", ":") })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path} was a move target");
        let (status, _) = send(
            &router,
            "POST",
            "/instances/local%3Aa/projects/remove",
            Some(json!({ "project_path": path.replace("%2F", "/").replace("%3A", ":") })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path} was removed");
    }
    let (status, _) =
        send(&router, "GET", &format!("{files}?path=.."), None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    archive
        .start_file("../escape.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    std::io::Write::write_all(&mut archive, b"out").unwrap();
    let archive = archive.finish().unwrap().into_inner();
    send_raw(
        &router,
        "PUT",
        &format!("{files}/content?path=slip.zip&create_only=false"),
        archive,
    )
    .await;
    let (status, _) = send(
        &router,
        "POST",
        &format!("{files}/extract"),
        Some(json!({ "path": "slip.zip", "override_conflicts": true, "dry_run": false })),
    )
    .await;
    assert!(!status.is_success());

    #[cfg(unix)]
    {
        let outside = dir.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("secret.txt"), "secret").unwrap();
        std::os::unix::fs::symlink(
            &outside,
            dir.path().join("profiles/Survival/link"),
        )
        .unwrap();
        let (status, _) = send_raw(
            &router,
            "GET",
            &format!("{files}/content?path=link%2Fsecret.txt"),
            Vec::new(),
        )
        .await;
        assert!(!status.is_success());
    }

    assert!(!has_file_named(dir.path(), "escape.txt"));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_instance_is_edited_then_removed() {
    let dir = tempfile::tempdir().unwrap();
    let router = host(&dir, true).await;
    let instance = "/instances/local%3Aa";

    let (status, _) = send(
        &router,
        "PATCH",
        instance,
        Some(json!({ "name": "Creative", "java_path": null })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, edited) = send(&router, "GET", instance, None).await;
    assert_eq!(edited["name"], "Creative");
    let (status, _) = send(&router, "DELETE", instance, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, removed) = send(&router, "GET", instance, None).await;
    assert_eq!(removed, Value::Null);
}

#[tokio::test(flavor = "multi_thread")]
async fn install_jobs_are_read_and_controlled_through_their_instance() {
    crate::install_crypto_provider();
    let dir = tempfile::tempdir().unwrap();
    let engine = engine(dir.path().to_path_buf()).await;
    add_instance(&engine, "local:a", "Survival").await;
    let request = InstallRequest::InstallExistingInstance {
        instance_id: "local:a".to_owned(),
        force: false,
    };
    let id = uuid::Uuid::new_v4();
    crate::install::store::insert(
        id,
        &request,
        &InstallJobState::new(&request),
        InstallJobStatus::Failed,
        &engine.pool,
    )
    .await
    .unwrap();
    let router = |allow: bool| {
        super::routes().with_state(Host {
            engine: Arc::clone(&engine),
            access: Arc::new(FakeAccess { allow }),
        })
    };
    let job = format!("/install-jobs/{id}");

    let denied = router(false);
    assert_eq!(
        send(&denied, "GET", "/install-jobs?include_finished=true", None).await,
        (StatusCode::OK, json!([]))
    );
    assert_eq!(
        send(&denied, "GET", &job, None).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(&denied, "DELETE", &job, None).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(&denied, "GET", "/shared-history", None).await.0,
        StatusCode::FORBIDDEN
    );

    let allowed = router(true);
    let (_, listed) =
        send(&allowed, "GET", "/install-jobs?include_finished=true", None)
            .await;
    assert_eq!(listed[0]["job_id"], id.to_string());
    let (status, read) = send(&allowed, "GET", &job, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read["instance_id"], "local:a");
    assert_eq!(read["status"], "failed");
    assert_eq!(
        send(&allowed, "DELETE", &job, None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        send(&allowed, "GET", "/install-jobs?include_finished=true", None)
            .await,
        (StatusCode::OK, json!([]))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unshared_instance_reads_empty() {
    let shared = "/instances/local%3Aa/shared";
    let dir = tempfile::tempdir().unwrap();
    let owner = host(&dir, true).await;
    assert_eq!(
        send(&owner, "GET", &format!("{shared}/users"), None).await,
        (
            StatusCode::OK,
            json!({ "user_ids": [], "users": [], "tokens": 0 })
        )
    );
    for (uri, empty) in [
        ("invites", json!([])),
        ("publish-preview", Value::Null),
        ("publish-support-details", Value::Null),
    ] {
        assert_eq!(
            send(&owner, "GET", &format!("{shared}/{uri}"), None).await,
            (StatusCode::OK, empty),
            "{uri}"
        );
    }
}

async fn server_host(dir: &tempfile::TempDir) -> (Router, std::path::PathBuf) {
    crate::install_crypto_provider();
    let engine = Engine::new(crate::state::EngineConfig {
        side: contracts::Side::Server,
        ..crate::state::tests::config(dir.path().to_path_buf())
    })
    .await
    .unwrap();
    add_instance(&engine, "local:a", "Survival").await;
    let root = engine.directories.instances_dir().join("Survival");
    std::fs::create_dir_all(&root).unwrap();
    let router = super::routes().with_state(Host {
        engine,
        access: Arc::new(FakeAccess { allow: true }),
    });
    (router, root)
}

#[tokio::test(flavor = "multi_thread")]
async fn an_icon_is_set_from_an_image_or_the_editor_then_removed() {
    let dir = tempfile::tempdir().unwrap();
    let router = host(&dir, true).await;
    let icon = "/instances/local%3Aa/icon";
    let mut wide = Vec::new();
    image::RgbaImage::new(1024, 512)
        .write_to(
            &mut std::io::Cursor::new(&mut wide),
            image::ImageFormat::Png,
        )
        .unwrap();

    let (status, _) =
        send_raw(&router, "PUT", icon, b"not an image".to_vec()).await;
    assert!(!status.is_success());
    let (status, _) = send_raw(&router, "PUT", icon, wide.clone()).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, read) = send_raw(&router, "GET", icon, Vec::new()).await;
    assert_eq!(status, StatusCode::OK);
    let read = image::load_from_memory(&read).unwrap();
    assert_eq!((read.width(), read.height()), (512, 256), "shape kept");
    let (_, instance) =
        send(&router, "GET", "/instances/local%3Aa", None).await;
    assert_eq!(instance["icon_config"], Value::Null);

    let config = json!({ "background": { "type": "color", "value": "#c78aff" }, "symbol": "dusk_block" });
    let (status, _) = send(
        &router,
        "PUT",
        &format!("{icon}/edited"),
        Some(json!({ "config": config, "image": wide })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, instance) =
        send(&router, "GET", "/instances/local%3Aa", None).await;
    assert_eq!(instance["icon_config"], config);

    assert_eq!(
        send(&router, "DELETE", icon, None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        send(&router, "GET", icon, None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(&router, "DELETE", icon, None).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_servers_files_stay_in_its_folder_and_never_replace_what_it_owns() {
    let dir = tempfile::tempdir().unwrap();
    let (router, root) = server_host(&dir).await;
    let files = "/instances/local%3Aa/files";
    let write = |path: &str, create_only: bool| {
        format!("{files}/content?path={path}&create_only={create_only}")
    };

    let (status, _) = send_raw(
        &router,
        "PUT",
        &write("config%2Fa.toml", true),
        b"x = 1".to_vec(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) =
        send_raw(&router, "PUT", &write("config%2Fa.toml", true), vec![1])
            .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, listed) =
        send(&router, "GET", &format!("{files}?path=config"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed[0]["path"], "config/a.toml");
    assert_eq!(listed[0]["size"], 5);
    let (status, read) = send_raw(
        &router,
        "GET",
        &format!("{files}/content?path=config%2Fa.toml"),
        Vec::new(),
    )
    .await;
    assert_eq!((status, &read[..]), (StatusCode::OK, &b"x = 1"[..]));
    let (status, stat) =
        send(&router, "GET", &format!("{files}/stat?path=config"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(stat["type"], "directory");
    assert_eq!(stat["full_path"], "/config", "Kyros's stat shape");
    for path in [
        "..%2Fescape.txt",
        "a%2F..%2F..%2Fescape.txt",
        "C%3A%2Fescape.txt",
    ] {
        let (status, _) =
            send_raw(&router, "PUT", &write(path, false), vec![1]).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
    }
    assert!(!has_file_named(dir.path(), "escape.txt"));

    // A write is an explicit edit and may change a server-owned file; nothing else replaces one.
    let (status, _) = send_raw(
        &router,
        "PUT",
        &write("server.properties", false),
        b"motd=Edited\n".to_vec(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in [
        ("server.properties", "motd=Theirs\n"),
        ("mods/a.jar", "jar"),
    ] {
        archive
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut archive, body.as_bytes()).unwrap();
    }
    let archive = archive.finish().unwrap().into_inner();
    send_raw(&router, "PUT", &write("upload.zip", false), archive).await;
    let extract = |dry_run: bool| {
        Some(
            json!({ "path": "upload.zip", "target": "", "override_conflicts": true, "dry_run": dry_run }),
        )
    };
    let (status, dry) =
        send(&router, "POST", &format!("{files}/extract"), extract(true)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dry["conflicting_files"], json!(["server.properties"]));
    let (status, _) =
        send(&router, "POST", &format!("{files}/extract"), extract(false))
            .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        std::fs::read_to_string(root.join("mods/a.jar")).unwrap(),
        "jar"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("server.properties")).unwrap(),
        "motd=Edited\n"
    );

    let uploads = "/instances/local%3Aa/uploads";
    let open =
        || send(&router, "POST", uploads, Some(json!({ "scope": "files" })));
    let (_, upload) = open().await;
    let upload = upload["upload_id"].as_str().unwrap().to_owned();
    for path in ["config%2Fb.toml", "server.properties"] {
        let (status, _) = send_raw(
            &router,
            "POST",
            &format!("{uploads}/{upload}/files?path={path}"),
            b"y = 2".to_vec(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{path}");
    }
    let (status, _) = send(
        &router,
        "POST",
        &format!("{uploads}/{upload}/finalize"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(!root.join("config/b.toml").exists(), "all or nothing");
    let (_, upload) = open().await;
    let upload = upload["upload_id"].as_str().unwrap().to_owned();
    send_raw(
        &router,
        "POST",
        &format!("{uploads}/{upload}/files?path=config%2Fb.toml"),
        b"y = 2".to_vec(),
    )
    .await;
    let (status, finalized) = send(
        &router,
        "POST",
        &format!("{uploads}/{upload}/finalize"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(finalized["status"], "finalized");
    assert_eq!(
        std::fs::read_to_string(root.join("config/b.toml")).unwrap(),
        "y = 2"
    );

    let (status, _) = send(
        &router,
        "POST",
        &format!("{files}/zip"),
        Some(json!({ "target_type": "Directory", "path": "config" })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let mut zipped = zip::ZipArchive::new(
        std::fs::File::open(root.join("config.zip")).unwrap(),
    )
    .unwrap();
    let mut names: Vec<_> = (0..zipped.len())
        .map(|index| {
            zipped.by_index(index).unwrap().name().unwrap().into_owned()
        })
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["config/", "config/a.toml", "config/b.toml"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_servers_file_routes_follow_no_link() {
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.txt"), "secret").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (router, root) = server_host(&dir).await;
    let link = root.join("linked");
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), &link).unwrap();
    #[cfg(windows)]
    assert!(
        std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(outside.path())
            .output()
            .unwrap()
            .status
            .success()
    );
    let file = "/instances/local%3Aa/files/content?path=linked%2Fsecret.txt";
    let (status, _) = send_raw(&router, "GET", file, Vec::new()).await;
    assert!(!status.is_success());
    let (status, _) = send_raw(
        &router,
        "PUT",
        &format!("{file}&create_only=false"),
        b"overwritten".to_vec(),
    )
    .await;
    assert!(!status.is_success());
    assert_eq!(
        std::fs::read_to_string(outside.path().join("secret.txt")).unwrap(),
        "secret"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_apps_files_tab_leaves_managed_content_to_the_content_tab() {
    crate::install_crypto_provider();
    let dir = tempfile::tempdir().unwrap();
    let engine = engine(dir.path().to_path_buf()).await;
    add_instance(&engine, "local:a", "Survival").await;
    let root = engine.directories.instances_dir().join("Survival");
    std::fs::create_dir_all(root.join("mods")).unwrap();
    let pack = dir.path().join("pack.zip");
    std::fs::write(&pack, "pack").unwrap();
    crate::instance::add_project_from_path(
        &engine,
        "local:a",
        &pack,
        Some(crate::state::ProjectType::ResourcePack),
    )
    .await
    .unwrap();
    let router = super::routes().with_state(Host {
        engine,
        access: Arc::new(FakeAccess { allow: true }),
    });
    let files = "/instances/local%3Aa/files";
    let write =
        |path: &str| format!("{files}/content?path={path}&create_only=false");
    let (status, _) =
        send_raw(&router, "PUT", &write("config%2Fa.txt"), b"a".to_vec()).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let read_only = |listed: &Value, path: &str| {
        listed
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["path"] == path)
            .unwrap_or_else(|| panic!("{path} is listed"))["readOnly"]
            .clone()
    };
    let (_, listed) =
        send(&router, "GET", &format!("{files}?path="), None).await;
    assert_eq!(read_only(&listed, "mods"), true);
    assert_eq!(read_only(&listed, "resourcepacks"), true);
    assert_eq!(read_only(&listed, "config"), false);
    let (_, listed) =
        send(&router, "GET", &format!("{files}?path=resourcepacks"), None)
            .await;
    assert_eq!(read_only(&listed, "resourcepacks/pack.zip"), true);

    for path in ["resourcepacks%2Fpack.zip", "mods%2Fa.jar"] {
        let (status, _) =
            send_raw(&router, "PUT", &write(path), b"x".to_vec()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path} was written");
    }
    for (method, uri, body) in [
        (
            "DELETE",
            format!("{files}?path=resourcepacks%2Fpack.zip"),
            None,
        ),
        ("DELETE", format!("{files}?path=mods&recursive=true"), None),
        ("POST", format!("{files}/directories?path=mods%2Fsub"), None),
        (
            "POST",
            format!("{files}/move"),
            Some(
                json!({ "source": "resourcepacks/pack.zip", "destination": "config/pack.zip" }),
            ),
        ),
        (
            "POST",
            format!("{files}/move"),
            Some(
                json!({ "source": "config/a.txt", "destination": "mods/a.jar" }),
            ),
        ),
    ] {
        let (status, _) = send(&router, method, &uri, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{method} {uri}");
    }

    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    archive
        .start_file("mods/b.jar", zip::write::SimpleFileOptions::default())
        .unwrap();
    std::io::Write::write_all(&mut archive, b"jar").unwrap();
    let archive = archive.finish().unwrap().into_inner();
    send_raw(&router, "PUT", &write("drop.zip"), archive).await;
    let (status, _) = send(
        &router,
        "POST",
        &format!("{files}/extract"),
        Some(json!({ "path": "drop.zip", "target": "", "override_conflicts": true, "dry_run": false })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // An upload's files are placed as a write would be.
    let uploads = "/instances/local%3Aa/uploads";
    let (_, upload) =
        send(&router, "POST", uploads, Some(json!({ "scope": "files" }))).await;
    let upload = upload["upload_id"].as_str().unwrap().to_owned();
    send_raw(
        &router,
        "POST",
        &format!("{uploads}/{upload}/files?path=mods%2Fc.jar"),
        b"jar".to_vec(),
    )
    .await;
    let (status, _) = send(
        &router,
        "POST",
        &format!("{uploads}/{upload}/finalize"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    assert_eq!(
        std::fs::read_to_string(root.join("resourcepacks/pack.zip")).unwrap(),
        "pack"
    );
    assert!(root.join("mods").is_dir());
    assert!(
        ["a.jar", "b.jar", "c.jar"]
            .iter()
            .all(|jar| !root.join("mods").join(jar).exists())
    );
    assert!(root.join("config/a.txt").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_apps_files_follow_a_linked_data_folder_but_no_link_in_an_instance()
{
    crate::install_crypto_provider();
    let dir = tempfile::tempdir().unwrap();
    // The App's instances live on another drive, reached through a link.
    let external = tempfile::tempdir().unwrap();
    let instances = dir.path().join("profiles");
    link_dir(external.path(), &instances);
    let engine = Engine::new(crate::state::EngineConfig {
        instances_dir: instances.clone(),
        ..crate::state::tests::config(dir.path().to_path_buf())
    })
    .await
    .unwrap();
    add_instance(&engine, "local:a", "Survival").await;
    let root = external.path().join("Survival");
    std::fs::create_dir_all(&root).unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.txt"), "secret").unwrap();
    link_dir(outside.path(), &root.join("linked"));
    let router = super::routes().with_state(Host {
        engine,
        access: Arc::new(FakeAccess { allow: true }),
    });
    let files = "/instances/local%3Aa/files";

    let (status, _) = send_raw(
        &router,
        "PUT",
        &format!("{files}/content?path=config%2Fa.txt&create_only=false"),
        b"a".to_vec(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        std::fs::read_to_string(root.join("config/a.txt")).unwrap(),
        "a"
    );
    let (status, read) = send_raw(
        &router,
        "GET",
        &format!("{files}/content?path=config%2Fa.txt"),
        Vec::new(),
    )
    .await;
    assert_eq!((status, &read[..]), (StatusCode::OK, &b"a"[..]));
    let (status, listed) =
        send(&router, "GET", &format!("{files}?path="), None).await;
    assert_eq!(status, StatusCode::OK);
    let linked = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["path"] == "linked")
        .expect("the link is listed");
    assert_eq!(linked["type"], "symlink");

    // A link inside the instance is neither followed nor changed.
    for (method, uri) in [
        ("GET", format!("{files}?path=linked")),
        ("GET", format!("{files}/content?path=linked%2Fsecret.txt")),
        (
            "PUT",
            format!(
                "{files}/content?path=linked%2Fsecret.txt&create_only=false"
            ),
        ),
        (
            "PUT",
            format!("{files}/content?path=linked&create_only=false"),
        ),
        ("DELETE", format!("{files}?path=linked%2Fsecret.txt")),
        ("DELETE", format!("{files}?path=linked&recursive=true")),
        ("POST", format!("{files}/directories?path=linked%2Fsub")),
    ] {
        let (status, _) =
            send_raw(&router, method, &uri, b"overwritten".to_vec()).await;
        assert!(!status.is_success(), "{method} {uri}");
    }
    let (status, _) = send(
        &router,
        "POST",
        &format!("{files}/move"),
        Some(json!({ "source": "linked", "destination": "moved" })),
    )
    .await;
    assert!(!status.is_success());
    assert_eq!(
        std::fs::read_to_string(outside.path().join("secret.txt")).unwrap(),
        "secret"
    );
    assert!(!outside.path().join("sub").exists());
    assert!(std::fs::symlink_metadata(root.join("linked")).is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_servers_logs_are_read_and_deleted_through_no_link() {
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("crash.txt"), "secret").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (router, root) = server_host(&dir).await;
    std::fs::create_dir_all(root.join("logs")).unwrap();
    std::fs::write(root.join("logs/latest.log"), "[main] ready").unwrap();
    link_dir(outside.path(), &root.join("crash-reports"));
    // A FIFO a mod made would hang a reader that opened it.
    #[cfg(unix)]
    nix::unistd::mkfifo(
        &root.join("logs/fifo.log"),
        nix::sys::stat::Mode::S_IRWXU,
    )
    .unwrap();
    let logs = "/instances/local%3Aa/logs";

    let (status, listed) = send(&router, "GET", logs, None).await;
    assert_eq!(status, StatusCode::OK);
    let names: Vec<_> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|log| log["filename"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["latest.log"]);
    let (status, output) =
        send(&router, "GET", &format!("{logs}/InfoLog/latest.log"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(output.as_str().unwrap().contains("ready"));

    let behind_link = format!("{logs}/CrashReport/crash.txt");
    for method in ["GET", "DELETE"] {
        let (status, _) = send(&router, method, &behind_link, None).await;
        assert!(!status.is_success(), "{method}");
    }
    #[cfg(unix)]
    {
        let fifo = format!("{logs}/InfoLog/fifo.log");
        let (status, _) = send(&router, "GET", &fifo, None).await;
        assert!(!status.is_success());
    }
    assert!(outside.path().join("crash.txt").exists());
    let (status, _) = send(
        &router,
        "DELETE",
        &format!("{logs}/InfoLog/latest.log"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!root.join("logs/latest.log").exists());
}
