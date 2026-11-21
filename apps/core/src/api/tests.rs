//! Routes as the App calls them: dangerous actions, sessions, servers, and their files, against
//! a real router and an in-memory database.

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::{CoreState, router};
use crate::{
    access::{
        self, CoreAccess,
        local::Local,
        now, sessions,
        tickets::test_support::{Signer, claims},
    },
    daemon::{folders, paths::Paths},
    db,
    servers::Servers,
    update::Updater,
};

struct Core {
    _dir: tempfile::TempDir,
    state: Arc<CoreState>,
    router: Router,
    signer: Signer,
    core_id: String,
    server: String,
}

/// A Core owned by `owner`, with one server.
async fn core() -> Core {
    core_with_backend(None).await
}

/// [`core`], with its engine calling the backend at `backend` as the Core.
async fn core_with_backend(backend: Option<&str>) -> Core {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    folders::create(&paths, None).unwrap();
    let engine = crate::daemon::start_engine(&paths, backend).await.unwrap();
    if let Some(backend) = backend {
        engine
            .sessions
            .set_backend_session(Some(engine::BackendSession {
                base_url: backend.parse().unwrap(),
                bearer: "amc_key".into(),
                kind: engine::BackendSessionKind::CoreKey,
            }));
    }
    let db = db::memory().await;
    let core_id = db::core_id(&db).await.unwrap();
    sqlx::query("UPDATE core SET owner_user_id = 'owner'")
        .execute(&db)
        .await
        .unwrap();
    let servers = Servers::open(
        paths.clone(),
        db.clone(),
        Arc::clone(&engine),
        Some(std::net::Ipv4Addr::LOCALHOST.into()),
    )
    .await
    .unwrap();
    let server = servers.create("A").await.unwrap().id;
    let signer = Signer::new("k1");
    let keys = access::keys::load(
        db.clone(),
        crate::network::http().build().unwrap(),
        None,
    )
    .await
    .unwrap();
    keys.set(&signer.public_keys()).await;
    let backups = crate::backups::Backups::open(
        servers.paths().clone(),
        db.clone(),
        Arc::clone(&servers),
    )
    .await
    .unwrap();
    let storage = Arc::new(crate::storage::Storage::new(
        db.clone(),
        paths.clone(),
        core_id.clone(),
        None,
        false,
        Arc::new(tokio::sync::Notify::new()),
    ));
    let update = Arc::new(Updater::new(
        paths,
        db.clone(),
        crate::network::http().build().unwrap(),
        String::new(),
    ));
    let state = Arc::new(CoreState {
        servers,
        engine,
        access: Arc::new(CoreAccess::new(db.clone(), keys)),
        network: Arc::new(crate::network::forward::Network::off(db.clone())),
        db,
        backups,
        update,
        storage,
        consoles: Arc::default(),
    });
    Core {
        _dir: dir,
        router: router(Arc::clone(&state)),
        state,
        signer,
        core_id,
        server,
    }
}

impl Core {
    fn ticket(&self, user: &str) -> String {
        self.signer.sign(&claims(user, &self.core_id, now()))
    }

    async fn sign_in(&self, user: &str) -> String {
        sessions::sign_in(
            &self.state.db,
            &self.state.access.keys,
            &self.ticket(user),
            "Laptop",
            now(),
        )
        .await
        .unwrap()
        .token
    }

    /// Call a route with a session token, a ticket, and a body, and read the answer.
    async fn call(
        &self,
        method: Method,
        uri: &str,
        token: Option<&str>,
        ticket: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder().method(method).uri(uri);
        if let Some(token) = token {
            request =
                request.header("authorization", format!("Bearer {token}"));
        }
        if let Some(ticket) = ticket {
            request = request.header(contracts::core::TICKET_HEADER, ticket);
        }
        let request = match body {
            Some(body) => request
                .header("content-type", "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        };
        self.send(request.unwrap()).await
    }

    /// A download link to `path` made with `token`'s session, and the URI that uses it.
    async fn link(&self, token: &str, path: &str) -> (StatusCode, String) {
        let body = Some(json!({ "path": path }));
        let (status, link) = self
            .call(Method::POST, "/links", Some(token), None, body)
            .await;
        let separator = if path.contains('?') { '&' } else { '?' };
        let token = link["token"].as_str().unwrap_or_default();
        (status, format!("{path}{separator}link={token}"))
    }

    async fn send(&self, request: Request<Body>) -> (StatusCode, Value) {
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    async fn count(&self, sql: &str) -> i64 {
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_owned()))
            .fetch_one(&self.state.db)
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn servers_wait_for_the_eula_and_every_route_for_a_session() {
    let core = core().await;
    let server = core.server.clone();
    let owner = core.sign_in("owner").await;

    let get = format!("/servers/{server}");
    let (status, detail) =
        core.call(Method::GET, &get, Some(&owner), None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["eula_accepted"], false);
    let (_, error) = core
        .call(
            Method::POST,
            &format!("/servers/{server}/power"),
            Some(&owner),
            None,
            Some(json!({ "action": "start" })),
        )
        .await;
    assert_eq!(error["error"], "eula_not_accepted");
    let create = Some(json!({ "name": "B" }));
    let (_, error) = core
        .call(Method::POST, "/servers", Some(&owner), None, create.clone())
        .await;
    assert_eq!(
        error["error"], "eula_not_accepted",
        "no new server before it"
    );
    core.call(Method::POST, "/eula", Some(&owner), None, None)
        .await;
    let (_, detail) =
        core.call(Method::GET, &get, Some(&owner), None, None).await;
    assert_eq!(detail["eula_accepted"], true);

    let (status, _) = core
        .call(Method::POST, "/servers", Some(&owner), None, create)
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, page) = core
        .call(Method::GET, "/activity", Some(&owner), None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["entries"][0]["action"]["type"], "server_created");
    let (_, list) = core
        .call(Method::GET, "/servers", Some(&owner), None, None)
        .await;
    assert_eq!(list.as_array().unwrap().len(), 2);

    assert_eq!(
        core.call(Method::GET, "/activity", None, None, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        core.call(Method::GET, "/activity", Some("forged"), None, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn dangerous_actions_need_a_fresh_ticket_of_the_owner() {
    let core = core().await;
    let owner = core.sign_in("owner").await;
    let delete = format!("/servers/{}", core.server);

    let (status, error) = core
        .call(Method::DELETE, &delete, Some(&owner), None, None)
        .await;
    assert_eq!(
        (status, error["error"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("ticket_required"))
    );
    let (_, error) = core
        .call(
            Method::DELETE,
            &delete,
            Some(&owner),
            Some(&core.ticket("stranger")),
            None,
        )
        .await;
    assert_eq!(
        error["error"], "ticket_invalid",
        "a ticket for someone else"
    );

    let ticket = core.ticket("owner");
    let (status, _) = core
        .call(Method::DELETE, &delete, Some(&owner), Some(&ticket), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!core.state.servers.paths().server(&core.server).exists());
    let (_, error) = core
        .call(Method::DELETE, &delete, Some(&owner), Some(&ticket), None)
        .await;
    assert_eq!(error["error"], "ticket_reused");
}

#[tokio::test]
async fn sessions_are_revoked_and_signed_out() {
    use sha2::{Digest, Sha256};

    let core = core().await;
    let owner = core.sign_in("owner").await;
    // Sign-in refuses anyone but the owner, so their session is made here.
    sqlx::query(
        "INSERT INTO sessions (id, token_hash, user_id, device_name, created_at, \
         last_used_at, expires_at) VALUES ('theirs', ?1, 'stranger', 'Phone', ?2, ?2, ?2 + 3600)",
    )
    .bind(Sha256::digest(b"stranger").to_vec())
    .bind(now())
    .execute(&core.state.db)
    .await
    .unwrap();

    let (status, _) = core
        .call(
            Method::GET,
            &format!("/servers/{}", core.server),
            Some("stranger"),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "no owner reads as no server");
    let (_, all) = core
        .call(Method::GET, "/sessions", Some(&owner), None, None)
        .await;
    assert_eq!(all.as_array().unwrap().len(), 2);

    // Someone else's session is a dangerous action for the owner.
    assert_eq!(
        core.call(Method::DELETE, "/sessions/theirs", Some(&owner), None, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, _) = core
        .call(
            Method::DELETE,
            "/sessions/theirs",
            Some(&owner),
            Some(&core.ticket("owner")),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        core.call(Method::GET, "/sessions", Some("stranger"), None, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );

    // One's own session needs no ticket.
    assert_eq!(
        core.call(
            Method::DELETE,
            "/sessions/current",
            Some(&owner),
            None,
            None
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        core.call(Method::GET, "/sessions", Some(&owner), None, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        core.count(
            "SELECT COUNT(*) FROM activity WHERE action = 'session_revoked'"
        )
        .await,
        2
    );
}

#[tokio::test]
async fn sign_in_answers_with_a_session() {
    let core = core().await;
    let body =
        json!({ "ticket": core.ticket("owner"), "device_name": "Phone" });
    let (status, signed) = core
        .call(Method::POST, "/sessions", None, None, Some(body))
        .await;
    assert_eq!(status, StatusCode::OK);
    let token = signed["token"].as_str().unwrap();
    let (_, sessions) = core
        .call(Method::GET, "/sessions", Some(token), None, None)
        .await;
    assert_eq!(sessions[0]["user"]["name"], "owner name");
    let stranger =
        json!({ "ticket": core.ticket("stranger"), "device_name": "Phone" });
    let (status, error) = core
        .call(Method::POST, "/sessions", None, None, Some(stranger))
        .await;
    assert_eq!(
        (status, error["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("forbidden"))
    );
}

#[tokio::test]
async fn sign_ins_are_limited_per_address() {
    let core = core().await;
    let address = "203.0.113.5".parse().unwrap();
    for _ in 0..10 {
        assert!(core.state.access.allow_sign_in(address));
    }
    assert!(!core.state.access.allow_sign_in(address));
    assert!(
        core.state
            .access
            .allow_sign_in("203.0.113.6".parse().unwrap())
    );
}

/// A request with a session and a raw body, answered as raw bytes.
async fn raw(
    core: &Core,
    method: Method,
    uri: &str,
    token: &str,
    body: &'static str,
) -> (StatusCode, bytes::Bytes) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap();
    let response = core.router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    (
        status,
        response.into_body().collect().await.unwrap().to_bytes(),
    )
}

#[tokio::test]
async fn content_and_files_stay_inside_the_server() {
    let core = core().await;
    let server = core.server.clone();
    let owner = core.sign_in("owner").await;
    let instance = format!("/instances/{server}");
    let files = format!("{instance}/files");

    // Engine routes a Core does not serve: their bodies or answers carry host paths, or they
    // reach past the server.
    for (method, uri) in [
        (Method::PATCH, instance.clone()),
        (Method::GET, "/instances".to_owned()),
        (Method::POST, format!("{instance}/install")),
    ] {
        let (status, _) =
            core.call(method, &uri, Some(&owner), None, None).await;
        assert!(
            matches!(
                status,
                StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
            ),
            "{uri}: {status}"
        );
    }

    // The files are used inside the server folder only.
    let content = format!("{files}/content");
    let write = format!("{content}?path=config/a.toml&create_only=true");
    assert_eq!(
        raw(&core, Method::PUT, &write, &owner, "x = 1").await.0,
        StatusCode::NO_CONTENT
    );
    let (status, list) = core
        .call(
            Method::GET,
            &format!("{files}?path=config"),
            Some(&owner),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list[0]["path"], "config/a.toml");
    let (status, body) = raw(
        &core,
        Method::GET,
        &format!("{content}?path=config/a.toml"),
        &owner,
        "",
    )
    .await;
    assert_eq!((status, &body[..]), (StatusCode::OK, &b"x = 1"[..]));
    for path in ["../../db/core.db", "config/../../db/core.db"] {
        let (status, _) = raw(
            &core,
            Method::GET,
            &format!("{content}?path={path}"),
            &owner,
            "",
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
    }

    // A new file never replaces one; an edit may, and the server may run meanwhile.
    let _shim =
        crate::servers::shim::try_lock(core.state.servers.paths(), &server)
            .unwrap()
            .unwrap();
    let properties = format!("{content}?path=server.properties");
    let new_file = format!("{properties}&create_only=true");
    assert_eq!(
        raw(&core, Method::PUT, &new_file, &owner, "motd=Mine\n")
            .await
            .0,
        StatusCode::NO_CONTENT,
        "nothing to replace yet"
    );
    assert_eq!(
        raw(&core, Method::PUT, &new_file, &owner, "motd=Theirs\n")
            .await
            .0,
        StatusCode::CONFLICT
    );
    let edit = format!("{properties}&create_only=false");
    assert_eq!(
        raw(&core, Method::PUT, &edit, &owner, "motd=Edited\n")
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        core.count(
            "SELECT COUNT(*) FROM activity WHERE action LIKE '%content_changed%'"
        )
        .await,
        3,
        "each edit is logged"
    );
    let (status, read) = core
        .call(
            Method::GET,
            &format!("/servers/{server}/properties"),
            Some(&owner),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read["properties"]["motd"], "Edited");
}

#[tokio::test]
async fn content_changes_wait_for_a_stopped_server_and_a_pack_update_backs_up_first()
 {
    let core = core().await;
    let server = core.server.clone();
    let owner = core.sign_in("owner").await;
    let instance = format!("/instances/{server}");
    let unlink = format!("{instance}/modpack/unlink");

    // A running server's content stays as it is.
    let shim =
        crate::servers::shim::try_lock(core.state.servers.paths(), &server)
            .unwrap()
            .unwrap();
    let (_, error) = core
        .call(Method::POST, &unlink, Some(&owner), None, None)
        .await;
    assert_eq!(error["error"], "already_running");
    // Share changes touch no files of the server.
    let (status, error) = core
        .call(
            Method::DELETE,
            &format!("{instance}/shared"),
            Some(&owner),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{error}");
    let (_, error) = core
        .call(
            Method::DELETE,
            &format!("{instance}/shared/history/1"),
            Some(&owner),
            None,
            None,
        )
        .await;
    assert_ne!(error["error"], "already_running");
    drop(shim);

    let (status, _) = core
        .call(Method::POST, &unlink, Some(&owner), None, None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        core.count(
            "SELECT COUNT(*) FROM activity WHERE action LIKE '%content_changed%'"
        )
        .await,
        2,
        "the unshare and the unlink"
    );

    // The backup is made before the engine sees the update, which it refuses: the server
    // follows no pack.
    let world = core.state.servers.paths().server(&server).join("world");
    std::fs::create_dir_all(&world).unwrap();
    std::fs::write(world.join("level.dat"), "level").unwrap();
    let mut events = core.state.engine.events.subscribe();
    let (status, _) = core
        .call(
            Method::POST,
            &format!("{instance}/modpack/update"),
            Some(&owner),
            None,
            Some(json!({ "version_id": "v" })),
        )
        .await;
    assert!(!status.is_success());
    assert_eq!(
        core.count("SELECT COUNT(*) FROM backups WHERE kind = 'pre_update'")
            .await,
        1
    );
    // The refusal ends the change for a panel that saw the server held.
    assert!(
        std::iter::from_fn(|| events.try_recv().ok()).any(|event| matches!(
            event,
            engine::Event::Instance(payload) if payload.instance_id == server
                && matches!(payload.event, engine::InstancePayloadType::Edited)
        )),
        "the server is announced once let go"
    );
}

#[tokio::test]
async fn restores_need_the_owners_fresh_ticket_and_downloads_a_link_or_a_session()
 {
    let core = core().await;
    let server = core.server.clone();
    let owner = core.sign_in("owner").await;
    let queue = format!("/servers/{server}/backups");

    let name = Some(json!({ "name": "First" }));
    let (status, created) = core
        .call(Method::POST, &queue, Some(&owner), None, name)
        .await;
    assert_eq!(status, StatusCode::OK);
    let backup = created["id"].as_str().unwrap().to_owned();
    let operation =
        crate::backups::records::operations(&core.state.db, &server)
            .await
            .unwrap()[0]
            .id;
    core.state.backups.wait(operation).await.unwrap();
    let (_, list) = core
        .call(Method::GET, &queue, Some(&owner), None, None)
        .await;
    assert_eq!(list["backups"][0]["state"], "done");
    assert_eq!(list["backups"][0]["operations"][0]["state"], "done");

    let restore = format!("{queue}/{backup}/restore");
    let (status, error) = core
        .call(Method::POST, &restore, Some(&owner), None, None)
        .await;
    assert_eq!(
        (status, error["error"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("ticket_required"))
    );
    let (status, _) = core
        .call(
            Method::POST,
            &restore,
            Some(&owner),
            Some(&core.ticket("owner")),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);

    // A link works once, for the exact path it was made for.
    let download = format!(
        "/servers/{server}/v1/worlds/{server}/backups/{backup}/download"
    );
    let (status, link) = core.link(&owner, &download).await;
    assert_eq!(status, StatusCode::OK);
    let get =
        |uri: &str| Request::builder().uri(uri).body(Body::empty()).unwrap();
    let response = core.router.clone().oneshot(get(&link)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert!(bytes.starts_with(b"PK"), "a zip");
    let (status, _) = core.send(get(&link)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "a second use");
    let (_, link) = core.link(&owner, &download).await;
    let token = link.rsplit_once("link=").unwrap().1;
    let world =
        format!("/servers/{server}/v1/worlds/{server}/files/download-full-zip");
    let (status, _) = core.send(get(&format!("{world}?link={token}"))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "another path");
    let (status, _) = core.send(get(&link)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "spent on the other path");

    // One file of the server, named in the query.
    std::fs::write(
        core.state
            .servers
            .paths()
            .server(&server)
            .join("server.properties"),
        "motd=Hi\n",
    )
    .unwrap();
    let (status, link) = core
        .link(
            &owner,
            &format!(
                "/instances/{server}/files/content?path=server.properties"
            ),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let response = core.router.clone().oneshot(get(&link)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&bytes[..], b"motd=Hi\n");
    let (status, _) = core.send(get(&link)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "a file downloads once");

    // Deleting a backup takes a ticket too.
    let delete = format!("{queue}/{backup}");
    core.state
        .backups
        .wait(
            // The restore, not the backup it makes of the world first.
            crate::backups::records::operations(&core.state.db, &server)
                .await
                .unwrap()
                .iter()
                .find(|op| op.kind == crate::backups::records::OpKind::Restore)
                .unwrap()
                .id,
        )
        .await
        .unwrap();
    let (status, _) = core
        .call(Method::DELETE, &delete, Some(&owner), None, None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = core
        .call(
            Method::DELETE,
            &delete,
            Some(&owner),
            Some(&core.ticket("owner")),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn updating_core_is_the_owners_dangerous_action() {
    let core = core().await;
    let owner = core.sign_in("owner").await;

    // Turning auto-update off is plain; a new channel or turning it on takes a ticket.
    let patch = |body| {
        core.call(Method::PATCH, "/update", Some(&owner), None, Some(body))
    };
    let (status, update) = patch(json!({ "auto_update": false })).await;
    assert_eq!(
        (status, &update["auto_update"]),
        (StatusCode::OK, &json!(false))
    );
    for risky in [
        json!({ "channel": "alpha" }),
        json!({ "auto_update": true }),
    ] {
        let (status, error) = patch(risky).await;
        assert_eq!(
            (status, error["error"].as_str()),
            (StatusCode::UNAUTHORIZED, Some("ticket_required"))
        );
    }
    let (status, update) = core
        .call(
            Method::PATCH,
            "/update",
            Some(&owner),
            Some(&core.ticket("owner")),
            Some(json!({ "channel": "beta" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(update["channel"], "beta");
    assert_eq!(update["auto_update"], false);

    let (status, error) = core
        .call(
            Method::POST,
            "/update/apply",
            Some(&owner),
            None,
            Some(json!({})),
        )
        .await;
    assert_eq!(
        (status, error["error"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("ticket_required"))
    );
    let (status, _) = core
        .call(
            Method::POST,
            "/update/apply",
            Some(&owner),
            Some(&core.ticket("owner")),
            Some(json!({ "allow_downgrade": true })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "only the CLI downgrades");

    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/update/rollback")
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(Local);
    let (status, _) = core.send(request).await;
    assert_eq!(status, StatusCode::CONFLICT, "nothing to roll back to");
}

#[tokio::test]
async fn removing_the_core_from_its_account_ends_every_session() {
    let core = core().await;
    let owner = core.sign_in("owner").await;
    let remove = |token, ticket| {
        core.call(Method::DELETE, "/pairing", Some(token), ticket, None)
    };
    assert_eq!(remove(&owner, None).await.0, StatusCode::UNAUTHORIZED);

    let mut changes = core.state.access.changed.subscribe();
    let ticket = core.ticket("owner");
    assert_eq!(
        remove(&owner, Some(&ticket)).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(changes.recv().await.unwrap(), access::Changed::Everyone);
    for (what, sql) in [
        (
            "owner",
            "SELECT COUNT(*) FROM core WHERE owner_user_id IS NOT NULL",
        ),
        ("sessions", "SELECT COUNT(*) FROM sessions"),
    ] {
        assert_eq!(core.count(sql).await, 0, "{what} left");
    }
    assert_eq!(
        core.count(
            "SELECT COUNT(*) FROM activity WHERE action = 'core_removed'"
        )
        .await,
        1
    );
    let (status, _) = core
        .call(Method::GET, "/sessions", Some(&owner), None, None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn slot_options_rotate_scheduled_backups_as_the_owners_dangerous_action()
{
    use crate::backups::records::Kind;

    let core = core().await;
    let server = core.server.clone();
    let owner = core.sign_in("owner").await;
    let backups = format!("/servers/{server}/backups");
    let schedule = format!("{backups}/schedule");
    let get = || core.call(Method::GET, &backups, Some(&owner), None, None);
    let patch = |token, ticket: Option<String>, body| {
        let core = &core;
        let schedule = &schedule;
        async move {
            core.call(
                Method::PATCH,
                schedule,
                Some(token),
                ticket.as_deref(),
                Some(body),
            )
            .await
        }
    };

    // Defaults: daily, 7 of the 15 slots for scheduled backups, due as soon as it can be.
    let (status, body) = get().await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["schedule"],
        json!({ "interval_hours": 24, "keep": 7, "slots": 15 })
    );
    assert!(!body["next_scheduled_at"].is_null());

    // Two scheduled backups, as the schedule makes them, each holding a slot.
    for _ in 0..2 {
        let (_, operation) = core
            .state
            .backups
            .create(&server, "Scheduled backup", Kind::Scheduled, None)
            .await
            .unwrap();
        core.state.backups.wait(operation).await.unwrap();
    }
    let (_, body) = get().await;
    let scheduled: Vec<&Value> = body["backups"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|backup| backup["kind"] == "scheduled")
        .collect();
    assert_eq!(scheduled.len(), 2);
    assert!(
        scheduled
            .iter()
            .all(|backup| !backup["rotates_at"].is_null())
    );

    // A new interval and more slots for scheduled backups need no ticket.
    let (status, body) = patch(
        &owner,
        None,
        json!({ "keep": 10, "interval_hours": 12, "delete_excess": false }),
    )
    .await;
    assert_eq!((status, &body["applied"]), (StatusCode::OK, &json!(true)));
    let (status, _) = patch(
        &owner,
        None,
        json!({ "keep": 16, "interval_hours": null, "delete_excess": false }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "there are 15 slots");

    // Fewer scheduled slots than backups: named first, then deleted with a ticket.
    let fewer = |delete_excess| json!({ "keep": 1, "interval_hours": null, "delete_excess": delete_excess });
    let (status, body) = patch(&owner, None, fewer(false)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["applied"], false);
    assert_eq!(body["excess"].as_array().unwrap().len(), 1);
    let oldest = body["excess"][0].clone();
    let (status, error) = patch(&owner, None, fewer(true)).await;
    assert_eq!(
        (status, error["error"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("ticket_required"))
    );
    let (_, body) = get().await;
    assert_eq!(body["schedule"]["keep"], 10, "unchanged until then");
    let (status, body) =
        patch(&owner, Some(core.ticket("owner")), fewer(true)).await;
    assert_eq!((status, &body["applied"]), (StatusCode::OK, &json!(true)));
    let (_, body) = get().await;
    assert_eq!(body["schedule"]["keep"], 1);
    let listed = body["backups"].as_array().unwrap();
    assert_eq!(listed.len(), 1);
    assert_ne!(listed[0]["id"], oldest, "the oldest went");

    // Off: nothing is due, every slot is manual, and the last scheduled backup went too.
    let (status, body) = patch(
        &owner,
        Some(core.ticket("owner")),
        json!({ "keep": 0, "interval_hours": null, "delete_excess": true }),
    )
    .await;
    assert_eq!((status, &body["applied"]), (StatusCode::OK, &json!(true)));
    let (_, body) = get().await;
    assert!(body["next_scheduled_at"].is_null());
    assert_eq!(body["schedule"]["keep"], 0);
    assert!(body["backups"].as_array().unwrap().is_empty());

    // A manual backup takes a manual slot, and is renamed.
    let (status, _) = patch(
        &owner,
        None,
        json!({ "keep": 14, "interval_hours": null, "delete_excess": false }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, created) = core
        .call(
            Method::POST,
            &backups,
            Some(&owner),
            None,
            Some(json!({ "name": "One" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = core
        .call(
            Method::POST,
            &backups,
            Some(&owner),
            None,
            Some(json!({ "name": "Two" })),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "the one manual slot is taken");
    let rename = format!("{backups}/{}", created["id"].as_str().unwrap());
    let renamed = Some(json!({ "name": "Renamed" }));
    let (status, _) = core
        .call(Method::PATCH, &rename, Some(&owner), None, renamed)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) = get().await;
    assert_eq!(body["backups"][0]["name"], "Renamed");
}

#[tokio::test]
async fn a_server_not_shared_has_no_share_history() {
    let core = core().await;
    let server = core.server.clone();
    let owner = core.sign_in("owner").await;
    let history = format!("/instances/{server}/shared/history");

    let (status, versions) = core
        .call(Method::GET, &history, Some(&owner), None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(versions, json!([]), "a server not shared has no history");
    let (status, _) = core
        .call(
            Method::DELETE,
            &format!("{history}/1"),
            Some(&owner),
            None,
            None,
        )
        .await;
    assert!(!status.is_success(), "a server not shared has no version 1");
}

#[tokio::test]
async fn deleting_a_shared_server_owes_its_shares_deletion_and_forgets_its_history()
 {
    let core = core().await;
    let owner = core.sign_in("owner").await;
    let engine_db = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(core.state.servers.paths().engine().join("engine.db")),
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO instance_links (instance_id, link_kind, shared_instance_id, shared_instance_role) \
         VALUES (?, 'shared_instance', 'share-1', 'owner') ON CONFLICT (instance_id) DO UPDATE SET \
         link_kind = excluded.link_kind, shared_instance_id = excluded.shared_instance_id, \
         shared_instance_role = excluded.shared_instance_role",
    )
    .bind(&core.server)
    .execute(&engine_db)
    .await
    .unwrap();
    let entry = json!({
        "version": 1,
        "created": 0,
        "content": { "modrinth_ids": [], "modpack_id": null, "removed_files": [], "game_version": "1.21.1", "loader": "vanilla", "loader_version": "" },
        "files": [],
        "diffs": [],
    });
    sqlx::query("INSERT INTO share_history (share_id, version, entry) VALUES ('share-1', 1, ?)")
        .bind(entry.to_string())
        .execute(&engine_db)
        .await
        .unwrap();

    // No backend answers: the deletion is owed, and sync sends it once one does.
    let (status, _) = core
        .call(
            Method::DELETE,
            &format!("/servers/{}", core.server),
            Some(&owner),
            Some(&core.ticket("owner")),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        core.count("SELECT COUNT(*) FROM backend_owed WHERE call LIKE '%\"delete_share\"%' AND call LIKE '%share-1%'").await,
        1
    );
    let history: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM share_history WHERE share_id = 'share-1'",
    )
    .fetch_one(&engine_db)
    .await
    .unwrap();
    assert_eq!(history, 0);
}

#[tokio::test]
async fn a_server_with_nothing_installed_is_not_shared() {
    let core = core().await;
    let owner = core.sign_in("owner").await;
    let push =
        json!({ "file_paths": [], "removed_paths": [], "restart": false });
    let (status, error) = core
        .call(
            Method::POST,
            &format!("/servers/{}/share", core.server),
            Some(&owner),
            None,
            Some(push),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{error}");
    assert!(
        engine::instance::get(&core.state.engine, &core.server)
            .await
            .unwrap()
            .unwrap()
            .shared_instance
            .is_none()
    );
}

#[tokio::test]
async fn the_owner_changes_storage_and_the_store_is_forwarded_to() {
    let core = core().await;
    let owner = core.sign_in("owner").await;
    // The store is not running here, and the settings answer anyway, without waiting for it.
    let (status, storage) = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        core.call(Method::GET, "/storage", Some(&owner), None, None),
    )
    .await
    .expect("GET /storage waited for a store that is not running");
    assert_eq!(status, StatusCode::OK);
    assert_eq!(storage["enabled"], true);
    assert_eq!(storage["egress_cap"], 50u64 << 30);
    assert!(storage["cap"].as_u64().unwrap() >= 10 << 30);

    let change = json!({ "enabled": false, "cap": 5u64 << 30 });
    let (status, storage) = core
        .call(Method::PATCH, "/storage", Some(&owner), None, Some(change))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(storage["enabled"], false);
    assert_eq!(storage["cap"], 5u64 << 30);
    assert_eq!(storage["egress_cap"], 50u64 << 30, "absent settings stay");
    let (status, _) = core
        .call(
            Method::PATCH,
            "/storage",
            Some(&owner),
            None,
            Some(json!({ "cap": -1 })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Anyone may reach the store, which checks its own tickets, but only while it runs.
    let (status, error) = core
        .call(Method::GET, "/store/versions/x", None, None, None)
        .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error["error"], "conflict");
}

#[tokio::test]
async fn the_owner_sets_the_servers_name_and_update_channel() {
    let core = core().await;
    let server = core.server.clone();
    let owner = core.sign_in("owner").await;
    let detail = format!("/servers/{server}");
    let channel = |detail: &Value| detail["update_channel"].clone();

    let (_, before) = core
        .call(Method::GET, &detail, Some(&owner), None, None)
        .await;
    assert_eq!(channel(&before), "release");

    let beta = Some(json!({ "update_channel": "beta" }));
    let (status, _) = core
        .call(Method::PATCH, &detail, Some(&owner), None, beta)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, after) = core
        .call(Method::GET, &detail, Some(&owner), None, None)
        .await;
    assert_eq!(channel(&after), "beta");

    let blank = Some(json!({ "name": "  " }));
    let (_, error) = core
        .call(Method::PATCH, &detail, Some(&owner), None, blank)
        .await;
    assert_eq!(error["error"], "invalid_input");
    let renamed = Some(json!({ "name": " Survival " }));
    let (status, _) = core
        .call(Method::PATCH, &detail, Some(&owner), None, renamed)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, after) = core
        .call(Method::GET, &detail, Some(&owner), None, None)
        .await;
    assert_eq!(after["name"], "Survival");
    assert_eq!(channel(&after), "beta", "a rename keeps the channel");
}

#[tokio::test]
async fn an_icon_is_saved_for_minecraft_and_the_app_then_reset() {
    let core = core().await;
    let owner = core.sign_in("owner").await;
    let uri = format!("/instances/{}/icon", core.server);
    let put = |token: &str, body: Vec<u8>| {
        Request::put(&uri)
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/octet-stream")
            .body(Body::from(body))
            .unwrap()
    };
    let game_icon = core
        .state
        .servers
        .paths()
        .server(&core.server)
        .join("server-icon.png");

    let (status, _) =
        core.call(Method::GET, &uri, Some(&owner), None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let mut wide = Vec::new();
    image::RgbaImage::new(1024, 512)
        .write_to(
            &mut std::io::Cursor::new(&mut wide),
            image::ImageFormat::Png,
        )
        .unwrap();
    assert_eq!(core.send(put(&owner, wide)).await.0, StatusCode::NO_CONTENT);
    let (status, _) =
        core.call(Method::GET, &uri, Some(&owner), None, None).await;
    assert_eq!(status, StatusCode::OK);
    let game = image::open(&game_icon).unwrap();
    assert_eq!((game.width(), game.height()), (64, 64));

    let (status, _) = core
        .call(Method::DELETE, &uri, Some(&owner), None, None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!game_icon.exists());
}

#[tokio::test]
async fn a_server_exports_as_a_pack_once_per_link() {
    use std::io::Read;

    let core = core().await;
    let server = core.server.clone();
    let owner = core.sign_in("owner").await;
    crate::servers::platform::set(
        &core.state.engine,
        &server,
        &crate::servers::platform::Platform {
            loader: engine::state::ModLoader::Fabric,
            game_version: "1.21.1".into(),
            loader_version: Some("0.16.0".into()),
        },
    )
    .await
    .unwrap();
    let config = core.state.servers.paths().server(&server).join("config");
    std::fs::create_dir_all(config.join("sub")).unwrap();
    std::fs::write(config.join("sub/a.toml"), "a = 1").unwrap();
    let path =
        format!("/servers/{server}/v1/worlds/{server}/content/export-mrpack");

    let (status, link) = core.link(&owner, &path).await;
    assert_eq!(status, StatusCode::OK);
    let export = || Request::builder().uri(&link).body(Body::empty()).unwrap();
    let response = core.router.clone().oneshot(export()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["content-disposition"],
        "attachment; filename=\"A.mrpack\""
    );
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let mut pack = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let index = engine::pack::server::read_index(&mut pack).unwrap();
    assert_eq!(index.name, "A");
    assert_eq!(index.dependencies["minecraft"], "1.21.1");
    assert_eq!(index.dependencies["fabric-loader"], "0.16.0");
    let mut text = String::new();
    pack.by_name("overrides/config/sub/a.toml")
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    assert_eq!(text, "a = 1");

    let (status, _) = core.send(export()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "a link exports once");
}

#[tokio::test]
async fn engine_wide_access_is_the_owners() {
    let core = core().await;
    let engine_access: Arc<dyn engine::access::Access> =
        core.state.access.clone();
    let session = |user: &str, owner| access::Caller::Session {
        id: user.into(),
        user_id: user.into(),
        owner,
    };
    for (caller, allowed) in [
        (access::Caller::Local, true),
        (session("owner", true), true),
        (session("stranger", false), false),
    ] {
        let granted = engine_access
            .require(&caller, None, access::Permission::View)
            .await
            .map_err(|error| error.error);
        let expected = if allowed {
            Ok(access::Permission::Owner)
        } else {
            Err(contracts::ErrorCode::Forbidden)
        };
        assert_eq!(granted, expected, "{caller:?}");
    }
}

type EventSocket = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

impl Core {
    /// The `/events` socket of `token`'s session, over loopback.
    async fn events(&self, token: &str) -> EventSocket {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;

        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let router = self.router.clone();
        tokio::spawn(async move { axum::serve(listener, router).await });
        let mut request = format!("ws://{address}/events")
            .into_client_request()
            .unwrap();
        request.headers_mut().insert(
            "authorization",
            format!("Bearer {token}").parse().unwrap(),
        );
        let stream = tokio::net::TcpStream::connect(address).await.unwrap();
        tokio_tungstenite::client_async(request, stream)
            .await
            .unwrap()
            .0
    }
}

async fn subscribe(socket: &mut EventSocket, topic: Value) {
    use futures::SinkExt;

    let message = json!({ "type": "subscribe", "topic": topic });
    socket
        .send(tokio_tungstenite::tungstenite::Message::text(
            message.to_string(),
        ))
        .await
        .unwrap();
}

/// The next event, or `None` once the socket closes.
async fn next(socket: &mut EventSocket) -> Option<Value> {
    use futures::StreamExt;
    use tokio_tungstenite::tungstenite::Message;

    let wait = std::time::Duration::from_secs(10);
    loop {
        match tokio::time::timeout(wait, socket.next()).await.unwrap() {
            Some(Ok(Message::Text(text))) => {
                return Some(serde_json::from_str(&text).unwrap());
            }
            Some(Ok(Message::Close(_)) | Err(_)) | None => return None,
            Some(Ok(_)) => {}
        }
    }
}

#[tokio::test]
async fn ending_the_session_closes_the_socket() {
    let core = core().await;
    let owner = core.sign_in("owner").await;
    let mut socket = core.events(&owner).await;
    subscribe(&mut socket, json!({ "type": "server", "id": core.server }))
        .await;
    next(&mut socket).await.unwrap();

    let (status, _) = core
        .call(
            Method::DELETE,
            "/sessions/current",
            Some(&owner),
            None,
            None,
        )
        .await;
    assert!(status.is_success());
    assert_eq!(next(&mut socket).await, None);
}

#[tokio::test]
async fn an_installs_subscription_starts_with_the_unfinished_jobs() {
    use engine::install::{
        InstallJobStatus, InstallRequest, model::InstallJobState,
    };

    let core = core().await;
    let request = InstallRequest::InstallExistingInstance {
        instance_id: core.server.clone(),
        force: false,
    };
    let job = uuid::Uuid::new_v4();
    engine::install::store::insert(
        job,
        &request,
        &InstallJobState::new(&request),
        InstallJobStatus::Running,
        core.state.engine.pool(),
    )
    .await
    .unwrap();
    let owner = core.sign_in("owner").await;
    let mut socket = core.events(&owner).await;
    subscribe(&mut socket, json!({ "type": "installs" })).await;
    subscribe(&mut socket, json!({ "type": "server", "id": core.server }))
        .await;

    let running = next(&mut socket).await.unwrap();
    assert_eq!(running["type"], "install_job");
    assert_eq!(running["payload"]["job_id"], job.to_string());
    assert_eq!(next(&mut socket).await.unwrap()["type"], "power");
}

#[tokio::test]
async fn a_loader_install_is_refused_while_the_engine_installs_the_server() {
    use engine::install::{
        InstallJobStatus, InstallRequest, model::InstallJobState,
    };

    let core = core().await;
    // A job the engine took up again after Core restarted: no hold covers it.
    let request = InstallRequest::InstallExistingInstance {
        instance_id: core.server.clone(),
        force: false,
    };
    engine::install::store::insert(
        uuid::Uuid::new_v4(),
        &request,
        &InstallJobState::new(&request),
        InstallJobStatus::Running,
        core.state.engine.pool(),
    )
    .await
    .unwrap();
    let owner = core.sign_in("owner").await;
    let (_, error) = core
        .call(
            Method::POST,
            &format!("/servers/{}/reinstall", core.server),
            Some(&owner),
            None,
            Some(json!({ "loader": "fabric", "loader_version": null, "game_version": "1.21.4" })),
        )
        .await;
    assert_eq!(error["error"], "installing", "{error}");
    let instance = engine::instance::get(&core.state.engine, &core.server)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(instance.applied_content_set.game_version, "");
}

#[tokio::test]
async fn the_content_is_announced_once_an_install_lets_the_server_go() {
    let core = core().await;
    let mut events = core.state.engine.events.subscribe();
    let hold = core.state.servers.hold(&core.server).unwrap();
    super::engine_mount::finish_job(Arc::clone(&core.state), hold).await;

    let edited = loop {
        match events.try_recv() {
            Ok(engine::Event::Instance(payload))
                if payload.instance_id == core.server =>
            {
                break payload;
            }
            Ok(_) => {}
            Err(error) => panic!("no edited event for the server: {error}"),
        }
    };
    assert!(matches!(edited.event, engine::InstancePayloadType::Edited));
    assert_eq!(core.state.servers.phase(&core.server), None);
}

#[tokio::test]
async fn a_lagged_subscriber_gets_resync_and_the_servers_state_again() {
    let core = core().await;
    let owner = core.sign_in("owner").await;
    let mut socket = core.events(&owner).await;
    subscribe(&mut socket, json!({ "type": "server", "id": core.server }))
        .await;
    next(&mut socket).await.unwrap();

    for _ in 0..300 {
        let _ = core
            .state
            .backups
            .events
            .send(super::events::CoreEvent::Resync);
    }
    assert_eq!(next(&mut socket).await.unwrap()["type"], "resync");
    assert_eq!(next(&mut socket).await.unwrap()["type"], "power");
}

/// A backend that answers the latest version of a share, `version`, to the read ticket `ticket`.
async fn share_versions(version: Arc<std::sync::atomic::AtomicI32>) -> String {
    use axum::{extract::State, http::HeaderMap, routing::get};
    use contracts::backend::sharing::{
        InstanceVersion, VersionContent, VersionManifest,
    };

    let app = Router::new()
        .route(
            "/v1/instances/{id}/versions",
            get(
                |State(version): State<Arc<std::sync::atomic::AtomicI32>>,
                 headers: HeaderMap| async move {
                    if headers
                        .get(contracts::tickets::READ_TICKET_HEADER)
                        .is_none_or(|ticket| ticket != "ticket")
                    {
                        return Err(StatusCode::FORBIDDEN);
                    }
                    Ok(axum::Json(InstanceVersion {
                        manifest: VersionManifest {
                            content: VersionContent {
                                modrinth_ids: Vec::new(),
                                modpack_id: None,
                                removed_files: Vec::new(),
                                game_version: "1.21.1".into(),
                                loader: "fabric".into(),
                                loader_version: "0.16.0".into(),
                            },
                            version_hash: contracts::Hash([0; 32]),
                            external_files: Vec::new(),
                        },
                        version: version
                            .load(std::sync::atomic::Ordering::SeqCst),
                        ready: true,
                        download: None,
                        upload: None,
                        reports: None,
                    }))
                },
            ),
        )
        .with_state(version);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await });
    url
}

fn follow_instance(ticket: &str) -> Value {
    json!({
        "instance": { "id": "app-instance", "name": "Our pack" },
        "shared_instance_id": "share",
        "ticket": ticket,
        "restart": false,
    })
}

#[tokio::test]
async fn a_stopped_server_follows_an_instance_at_once() {
    let version = Arc::new(std::sync::atomic::AtomicI32::new(2));
    let core = core_with_backend(Some(&share_versions(version).await)).await;
    let owner = core.sign_in("owner").await;
    let follow = format!("/servers/{}/follow", core.server);

    let (status, _) = core
        .call(
            Method::POST,
            &follow,
            Some(&owner),
            None,
            Some(follow_instance("another")),
        )
        .await;
    assert!(!status.is_success(), "the share is read for who asked only");
    let (status, error) = core
        .call(
            Method::POST,
            &follow,
            Some(&owner),
            None,
            Some(follow_instance("ticket")),
        )
        .await;
    assert!(status.is_success(), "{error}");
    let jobs = engine::install::list_jobs(true, &core.state.engine)
        .await
        .unwrap();
    assert!(
        jobs.iter().any(|job| job.instance_id.as_deref()
            == Some(core.server.as_str())
            && job.kind
                == engine::install::InstallJobKind::UpdateSharedInstance),
        "the version installs now"
    );
    let (_, server) = core
        .call(
            Method::GET,
            &format!("/servers/{}", core.server),
            Some(&owner),
            None,
            None,
        )
        .await;
    let linked = &server["linked_instance"];
    assert_eq!(linked["id"], "app-instance");
    assert_eq!(linked["shared_instance_id"], "share");
    assert_ne!(linked["state"], "pending");
}

#[tokio::test]
async fn a_running_server_takes_the_newest_pushed_version_once_it_stops() {
    let version = Arc::new(std::sync::atomic::AtomicI32::new(2));
    let core =
        core_with_backend(Some(&share_versions(Arc::clone(&version)).await))
            .await;
    let owner = core.sign_in("owner").await;
    let follow = format!("/servers/{}/follow", core.server);
    let pending = || {
        core.count(
            "SELECT COALESCE(json_extract(pending, '$.version'), 0) FROM follows",
        )
    };
    tokio::spawn(super::follow::apply_on_stop(Arc::clone(&core.state)));

    let shim = crate::servers::shim::try_lock(
        core.state.servers.paths(),
        &core.server,
    )
    .unwrap()
    .unwrap();
    let (status, error) = core
        .call(
            Method::POST,
            &follow,
            Some(&owner),
            None,
            Some(follow_instance("ticket")),
        )
        .await;
    assert!(status.is_success(), "{error}");
    assert_eq!(pending().await, 2);
    let (_, server) = core
        .call(
            Method::GET,
            &format!("/servers/{}", core.server),
            Some(&owner),
            None,
            None,
        )
        .await;
    assert_eq!(server["linked_instance"]["state"], "pending");
    assert_eq!(server["linked_instance"]["version"], Value::Null);

    // A newer push replaces the version still waiting.
    version.store(3, std::sync::atomic::Ordering::SeqCst);
    core.call(
        Method::POST,
        &follow,
        Some(&owner),
        None,
        Some(follow_instance("ticket")),
    )
    .await;
    assert_eq!(pending().await, 3);

    drop(shim);
    let _ = core
        .state
        .servers
        .events
        .send(crate::servers::Lifecycle::Ended(core.server.clone()));
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while pending().await != 0 {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the version applies once the server stops");
    assert_eq!(
        core.count("SELECT COUNT(*) FROM follows WHERE state = 'pending'")
            .await,
        0
    );
}
