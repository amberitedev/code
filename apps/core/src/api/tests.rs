//! Routes as the App calls them: per-server roles, dangerous actions, and sessions, against a
//! real router and an in-memory database.

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

/// A Core owned by `owner`, with one server where `editor` and `viewer` have those roles.
async fn core() -> Core {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    folders::create(&paths, None).unwrap();
    let engine = crate::daemon::start_engine(&paths, None).await.unwrap();
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
    for (user, role) in [("editor", "editor"), ("viewer", "viewer")] {
        sqlx::query("INSERT INTO access (server_id, user_id, role, added_at) VALUES (?, ?, ?, 0)")
            .bind(&server)
            .bind(user)
            .bind(role)
            .execute(&db)
            .await
            .unwrap();
    }
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
        uploads: super::files::Uploads::default(),
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
async fn each_role_reaches_only_what_the_table_gives_it() {
    let core = core().await;
    let server = core.server.clone();
    let owner = core.sign_in("owner").await;
    let editor = core.sign_in("editor").await;
    let viewer = core.sign_in("viewer").await;
    sqlx::query("INSERT INTO access (server_id, user_id, role, added_at) SELECT id, 'stranger', 'viewer', 0 FROM servers")
        .execute(&core.state.db)
        .await
        .unwrap();
    let stranger = core.sign_in("stranger").await;
    sqlx::query("DELETE FROM access WHERE user_id = 'stranger'")
        .execute(&core.state.db)
        .await
        .unwrap();

    let get = format!("/servers/{server}");
    let (status, detail) = core
        .call(Method::GET, &get, Some(&viewer), None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["eula_accepted"], false);
    let (status, _) = core
        .call(Method::GET, &get, Some(&stranger), None, None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "no role reads as no server");

    let power = format!("/servers/{server}/power");
    let start = Some(json!({ "action": "start" }));
    let (status, _) = core
        .call(Method::POST, &power, Some(&viewer), None, start.clone())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (_, error) = core
        .call(Method::POST, &power, Some(&editor), None, start)
        .await;
    assert_eq!(
        error["error"], "eula_not_accepted",
        "allowed, but the EULA is not accepted"
    );
    core.call(Method::POST, "/eula", Some(&owner), None, None)
        .await;
    let (_, detail) = core
        .call(Method::GET, &get, Some(&viewer), None, None)
        .await;
    assert_eq!(detail["eula_accepted"], true);

    let create = Some(json!({ "name": "B" }));
    let (status, _) = core
        .call(
            Method::POST,
            "/servers",
            Some(&editor),
            None,
            create.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = core
        .call(Method::POST, "/servers", Some(&owner), None, create)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        core.call(Method::GET, "/activity", Some(&editor), None, None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, page) = core
        .call(Method::GET, "/activity", Some(&owner), None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["entries"][0]["action"]["type"], "server_created");

    let (_, list) = core
        .call(Method::GET, "/servers", Some(&editor), None, None)
        .await;
    assert_eq!(
        list.as_array().unwrap().len(),
        1,
        "friends see only their servers"
    );
    let (_, list) = core
        .call(Method::GET, "/servers", Some(&owner), None, None)
        .await;
    assert_eq!(list.as_array().unwrap().len(), 2);
    let (_, me) = core
        .call(Method::GET, "/access/me", Some(&editor), None, None)
        .await;
    assert_eq!(me["servers"][0]["role"], "editor");
    assert_eq!(me["owner"], false);

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
async fn crash_console_lines_are_kept_from_viewers() {
    let core = core().await;
    sqlx::query(
        "INSERT INTO runs (id, server_id, started_at, ended_at, outcome, exit_code, last_lines) \
         VALUES ('run', ?, 1, 2, 'crashed', 1, '<Steve> my address is 10.0.0.5')",
    )
    .bind(&core.server)
    .execute(&core.state.db)
    .await
    .unwrap();
    let insight = format!("/servers/{}/insight", core.server);
    let viewer = core.sign_in("viewer").await;
    let (status, seen) = core
        .call(Method::GET, &insight, Some(&viewer), None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(seen["last_crash"]["exit_code"], 1);
    assert_eq!(seen["last_crash"]["last_lines"], Value::Null);
    let editor = core.sign_in("editor").await;
    let (_, seen) = core
        .call(Method::GET, &insight, Some(&editor), None, None)
        .await;
    assert_eq!(
        seen["last_crash"]["last_lines"],
        "<Steve> my address is 10.0.0.5"
    );
}

#[tokio::test]
async fn dangerous_actions_need_a_fresh_ticket_of_the_owner() {
    let core = core().await;
    let owner = core.sign_in("owner").await;
    let editor = core.sign_in("editor").await;
    let invite = format!("/servers/{}/access", core.server);
    let body = || {
        Some(
            json!({ "user": { "id": "friend", "name": "Friend", "avatar_url": null }, "role": "editor" }),
        )
    };

    let (status, error) = core
        .call(Method::POST, &invite, Some(&owner), None, body())
        .await;
    assert_eq!(
        (status, error["error"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("ticket_required"))
    );
    let theirs = core.ticket("editor");
    let (_, error) = core
        .call(Method::POST, &invite, Some(&owner), Some(&theirs), body())
        .await;
    assert_eq!(
        error["error"], "ticket_invalid",
        "a ticket for someone else"
    );
    let (status, _) = core
        .call(Method::POST, &invite, Some(&editor), Some(&theirs), body())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "only the owner invites");

    let ticket = core.ticket("owner");
    let (status, member) = core
        .call(Method::POST, &invite, Some(&owner), Some(&ticket), body())
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(member["user"]["name"], "Friend");
    assert_eq!(
        core.count(
            "SELECT COUNT(*) FROM backend_owed WHERE call LIKE '%\"grant\"%'"
        )
        .await,
        1
    );
    let (_, error) = core
        .call(Method::POST, &invite, Some(&owner), Some(&ticket), body())
        .await;
    assert_eq!(error["error"], "ticket_reused");

    // Removing the editor ends their sessions and owes the backend the change.
    let remove = format!("/servers/{}/access/editor", core.server);
    assert_eq!(
        core.call(Method::DELETE, &remove, Some(&owner), None, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let mut changes = core.state.access.changed.subscribe();
    let (status, _) = core
        .call(
            Method::DELETE,
            &remove,
            Some(&owner),
            Some(&core.ticket("owner")),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        changes.recv().await.unwrap(),
        access::Changed::User("editor".into())
    );
    assert_eq!(
        core.call(Method::GET, "/access/me", Some(&editor), None, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        core.count("SELECT COUNT(*) FROM backend_owed WHERE call LIKE '%\"remove\"%' AND call LIKE '%\"keep_core\":false%'").await,
        1
    );

    // The CLI on the Core's machine is the owner and needs no ticket.
    let mut request = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/servers/{}/access/viewer", core.server))
        .header("content-type", "application/json")
        .body(Body::from(json!({ "role": "editor" }).to_string()))
        .unwrap();
    request.extensions_mut().insert(Local);
    let (status, member) = core.send(request).await;
    assert_eq!(
        (status, member["role"].as_str()),
        (StatusCode::OK, Some("editor"))
    );

    // Deleting a server takes a ticket too, and drops everyone's role on it.
    let delete = format!("/servers/{}", core.server);
    assert_eq!(
        core.call(Method::DELETE, &delete, Some(&owner), None, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
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
    assert_eq!(core.count("SELECT COUNT(*) FROM access").await, 0);
    assert!(!core.state.servers.paths().server(&core.server).exists());
}

#[tokio::test]
async fn invites_stop_at_the_directory_cap() {
    let core = core().await;
    let owner = core.sign_in("owner").await;
    // Two people already have a role; fill the rest of the directory.
    for n in 2..super::access::MAX_DIRECTORY {
        sqlx::query("INSERT INTO access (server_id, user_id, role, added_at) VALUES (?, ?, 'viewer', 0)")
            .bind(&core.server)
            .bind(format!("user{n}"))
            .execute(&core.state.db)
            .await
            .unwrap();
    }
    let invite = format!("/servers/{}/access", core.server);
    let body = |id: &str| {
        Some(
            json!({ "user": { "id": id, "name": id, "avatar_url": null }, "role": "editor" }),
        )
    };
    let (status, error) = core
        .call(
            Method::POST,
            &invite,
            Some(&owner),
            Some(&core.ticket("owner")),
            body("newcomer"),
        )
        .await;
    assert_eq!(
        (status, error["error"].as_str()),
        (StatusCode::CONFLICT, Some("conflict"))
    );
    // Someone already in the directory still gets a new role.
    let (status, _) = core
        .call(
            Method::POST,
            &invite,
            Some(&owner),
            Some(&core.ticket("owner")),
            body("viewer"),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn sessions_are_revoked_and_signed_out() {
    let core = core().await;
    let owner = core.sign_in("owner").await;
    let laptop = core.sign_in("editor").await;
    let phone = core.sign_in("editor").await;
    let viewer = core.sign_in("viewer").await;

    let (_, mine) = core
        .call(Method::GET, "/sessions", Some(&laptop), None, None)
        .await;
    let mine = mine.as_array().unwrap().clone();
    assert_eq!(mine.len(), 2, "a friend sees only their own sessions");
    let phone_id = mine
        .iter()
        .find(|session| session["current"] == false)
        .unwrap()["id"]
        .clone();
    let (_, all) = core
        .call(Method::GET, "/sessions", Some(&owner), None, None)
        .await;
    assert_eq!(all.as_array().unwrap().len(), 4);

    // Someone else's session: hidden from a friend, a dangerous action for the owner.
    let revoke_phone = format!("/sessions/{}", phone_id.as_str().unwrap());
    assert_eq!(
        core.call(Method::DELETE, &revoke_phone, Some(&viewer), None, None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        core.call(Method::DELETE, &revoke_phone, Some(&owner), None, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, _) = core
        .call(
            Method::DELETE,
            &revoke_phone,
            Some(&owner),
            Some(&core.ticket("owner")),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        core.call(Method::GET, "/sessions", Some(&phone), None, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );

    // One's own session needs no ticket.
    assert_eq!(
        core.call(
            Method::DELETE,
            "/sessions/current",
            Some(&laptop),
            None,
            None
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        core.call(Method::GET, "/sessions", Some(&laptop), None, None)
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
        json!({ "ticket": core.ticket("viewer"), "device_name": "Phone" });
    let (status, signed) = core
        .call(Method::POST, "/sessions", None, None, Some(body))
        .await;
    assert_eq!(status, StatusCode::OK);
    let token = signed["token"].as_str().unwrap();
    let (_, me) = core
        .call(Method::GET, "/access/me", Some(token), None, None)
        .await;
    assert_eq!(me["user"]["name"], "viewer name");
    let stranger =
        json!({ "ticket": core.ticket("stranger"), "device_name": "Phone" });
    let (status, error) = core
        .call(Method::POST, "/sessions", None, None, Some(stranger))
        .await;
    assert_eq!(
        (status, error["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("not_invited"))
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
async fn content_and_files_follow_the_role_table() {
    let core = core().await;
    let server = core.server.clone();
    let editor = core.sign_in("editor").await;
    let viewer = core.sign_in("viewer").await;
    sqlx::query("INSERT INTO access (server_id, user_id, role, added_at) VALUES (?, 'stranger', 'viewer', 0)")
        .bind(&server)
        .execute(&core.state.db)
        .await
        .unwrap();
    let stranger = core.sign_in("stranger").await;
    sqlx::query("DELETE FROM access WHERE user_id = 'stranger'")
        .execute(&core.state.db)
        .await
        .unwrap();
    let fs = format!("/servers/{server}/modrinth/v0/fs");
    let instance = format!("/instances/{server}");

    // Viewers see the content list and what the owner edited, nothing else.
    for uri in [
        format!("{instance}/content"),
        format!("{instance}/content/edited"),
    ] {
        let (status, list) = core
            .call(Method::GET, &uri, Some(&viewer), None, None)
            .await;
        assert_eq!((status, list), (StatusCode::OK, json!([])), "{uri}");
        let (status, _) = core
            .call(Method::GET, &uri, Some(&stranger), None, None)
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "no role reads as no server");
    }
    for (method, uri) in [
        (Method::GET, format!("{fs}/list?path=/&page=1&page_size=10")),
        (Method::GET, format!("/servers/{server}/properties")),
        (Method::POST, format!("{instance}/content/repair")),
        (Method::POST, format!("{instance}/modpack/unlink")),
        (Method::GET, format!("{instance}/shared/publish-preview")),
    ] {
        let (status, _) =
            core.call(method, &uri, Some(&viewer), None, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}");
    }
    // Engine routes a Core does not serve: their bodies or answers carry host paths, or they
    // reach past the server.
    for (method, uri) in [
        (Method::PATCH, instance.clone()),
        (Method::GET, "/instances".to_owned()),
        (Method::POST, format!("{instance}/install")),
    ] {
        let (status, _) =
            core.call(method, &uri, Some(&editor), None, None).await;
        assert!(
            matches!(
                status,
                StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
            ),
            "{uri}: {status}"
        );
    }

    // Editors use the files, inside the server folder only.
    let create = format!("{fs}/create?path=/config/a.toml&type=file");
    assert_eq!(
        raw(&core, Method::POST, &create, &editor, "x = 1").await.0,
        StatusCode::NO_CONTENT
    );
    let (status, list) = core
        .call(
            Method::GET,
            &format!("{fs}/list?path=/config&page=1&page_size=10"),
            Some(&editor),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["items"][0]["path"], "/config/a.toml");
    let (status, body) = raw(
        &core,
        Method::GET,
        &format!("{fs}/download?path=/config/a.toml"),
        &editor,
        "",
    )
    .await;
    assert_eq!((status, &body[..]), (StatusCode::OK, &b"x = 1"[..]));
    let (status, _) = raw(
        &core,
        Method::GET,
        &format!("{fs}/download?path=/../../db/core.db"),
        &editor,
        "",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // An upload never replaces a server-owned file; an edit may.
    let properties = format!("{fs}/create?path=/server.properties&type=file");
    assert_eq!(
        raw(&core, Method::POST, &properties, &editor, "motd=Mine\n")
            .await
            .0,
        StatusCode::NO_CONTENT,
        "nothing to replace yet"
    );
    assert_eq!(
        raw(&core, Method::POST, &properties, &editor, "motd=Theirs\n")
            .await
            .0,
        StatusCode::CONFLICT
    );
    let update = format!("{fs}/update?path=/server.properties");
    assert_eq!(
        raw(&core, Method::PUT, &update, &editor, "motd=Edited\n")
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let (status, read) = core
        .call(
            Method::GET,
            &format!("/servers/{server}/properties"),
            Some(&editor),
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
    let editor = core.sign_in("editor").await;
    let viewer = core.sign_in("viewer").await;
    let instance = format!("/instances/{server}");
    let unlink = format!("{instance}/modpack/unlink");

    // A running server's content stays as it is.
    let shim =
        crate::servers::shim::try_lock(core.state.servers.paths(), &server)
            .unwrap()
            .unwrap();
    let (status, _) = core
        .call(Method::POST, &unlink, Some(&viewer), None, None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "the role is checked first");
    let (_, error) = core
        .call(Method::POST, &unlink, Some(&editor), None, None)
        .await;
    assert_eq!(error["error"], "already_running");
    // Share changes touch no files of the server.
    let owner = core.sign_in("owner").await;
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
        .call(Method::POST, &unlink, Some(&editor), None, None)
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
    let (status, _) = core
        .call(
            Method::POST,
            &format!("{instance}/modpack/update"),
            Some(&editor),
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
}

#[tokio::test]
async fn restores_need_the_owners_fresh_ticket_and_downloads_a_link_or_a_session()
 {
    let core = core().await;
    let server = core.server.clone();
    let owner = core.sign_in("owner").await;
    let editor = core.sign_in("editor").await;
    let viewer = core.sign_in("viewer").await;
    let queue = format!("/servers/{server}/backups");

    let name = Some(json!({ "name": "First" }));
    let (status, _) = core
        .call(Method::POST, &queue, Some(&viewer), None, name.clone())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "viewers only see backups");
    let (status, created) = core
        .call(Method::POST, &queue, Some(&editor), None, name.clone())
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
        .call(Method::GET, &queue, Some(&viewer), None, None)
        .await;
    assert_eq!(list["backups"][0]["state"], "done");
    assert_eq!(list["backups"][0]["operations"][0]["state"], "done");

    let restore = format!("{queue}/{backup}/restore");
    let (status, _) = core
        .call(
            Method::POST,
            &restore,
            Some(&editor),
            Some(&core.ticket("editor")),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "only the owner restores");
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

    // A link works once, for the exact path it was made for, while its maker may still make it.
    let download = format!(
        "/servers/{server}/v1/worlds/{server}/backups/{backup}/download"
    );
    let (status, _) = core.link(&viewer, &download).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "a viewer cannot download");
    let (status, link) = core.link(&editor, &download).await;
    assert_eq!(status, StatusCode::OK);
    let get =
        |uri: &str| Request::builder().uri(uri).body(Body::empty()).unwrap();
    let response = core.router.clone().oneshot(get(&link)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert!(bytes.starts_with(b"PK"), "a zip");
    let (status, _) = core.send(get(&link)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "a second use");
    let (_, link) = core.link(&editor, &download).await;
    let token = link.rsplit_once("link=").unwrap().1;
    let world =
        format!("/servers/{server}/v1/worlds/{server}/files/download-full-zip");
    let (status, _) = core.send(get(&format!("{world}?link={token}"))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "another path");
    let (status, _) = core.send(get(&link)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "spent on the other path");
    let (_, link) = core.link(&editor, &download).await;
    let demote = |role: &'static str| {
        sqlx::query("UPDATE access SET role = ? WHERE user_id = 'editor'")
            .bind(role)
            .execute(&core.state.db)
    };
    demote("viewer").await.unwrap();
    let (status, _) = core.send(get(&link)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "no longer an editor");
    demote("editor").await.unwrap();
    let (status, _) = core
        .call(Method::GET, &download, Some(&viewer), None, None)
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a viewer's session does not download"
    );

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
            &editor,
            &format!(
                "/servers/{server}/v1/worlds/{server}/files/contents-raw?path=server.properties"
            ),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let response = core.router.clone().oneshot(get(&link)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&bytes[..], b"motd=Hi\n");

    // Deleting a backup is the owner's too.
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
        .call(Method::DELETE, &delete, Some(&editor), None, None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
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
    let editor = core.sign_in("editor").await;
    let (status, _) = core
        .call(Method::GET, "/update", Some(&editor), None, None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

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
    let editor = core.sign_in("editor").await;
    let remove = |token, ticket| {
        core.call(Method::DELETE, "/pairing", Some(token), ticket, None)
    };
    let theirs = core.ticket("editor");
    assert_eq!(
        remove(&editor, Some(&theirs)).await.0,
        StatusCode::FORBIDDEN
    );
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
        ("roles", "SELECT COUNT(*) FROM access"),
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
        .call(Method::GET, "/access/me", Some(&owner), None, None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_member_leaves_a_server_without_a_ticket() {
    let core = core().await;
    let editor = core.sign_in("editor").await;
    let viewer = core.sign_in("viewer").await;
    let path = |user: &str| format!("/servers/{}/access/{user}", core.server);

    let (status, _) = core
        .call(Method::DELETE, &path("viewer"), Some(&editor), None, None)
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "only the owner removes others"
    );

    let mut changes = core.state.access.changed.subscribe();
    let (status, _) = core
        .call(Method::DELETE, &path("viewer"), Some(&viewer), None, None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        changes.recv().await.unwrap(),
        access::Changed::User("viewer".into())
    );
    assert_eq!(
        core.count("SELECT COUNT(*) FROM access WHERE user_id = 'viewer'")
            .await,
        0
    );
    assert_eq!(
        core.count("SELECT COUNT(*) FROM sessions WHERE user_id = 'viewer'")
            .await,
        0
    );
    assert_eq!(
        core.count(
            "SELECT COUNT(*) FROM backend_owed WHERE call LIKE '%\"remove\"%'"
        )
        .await,
        1
    );
    let (status, _) = core
        .call(Method::DELETE, &path("viewer"), Some(&viewer), None, None)
        .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the session ended with it"
    );
}

#[tokio::test]
async fn each_role_is_shown_its_own_role() {
    let core = core().await;
    let uri = format!("/servers/{}", core.server);
    for (user, expected) in [
        ("owner", Value::Null),
        ("editor", json!("editor")),
        ("viewer", json!("viewer")),
    ] {
        let token = core.sign_in(user).await;
        let (_, server) =
            core.call(Method::GET, &uri, Some(&token), None, None).await;
        assert_eq!(server["role"], expected, "{user}");
    }
}

#[tokio::test]
async fn slot_options_rotate_scheduled_backups_as_the_owners_dangerous_action()
{
    use crate::backups::records::Kind;

    let core = core().await;
    let server = core.server.clone();
    let owner = core.sign_in("owner").await;
    let editor = core.sign_in("editor").await;
    let viewer = core.sign_in("viewer").await;
    let backups = format!("/servers/{server}/backups");
    let schedule = format!("{backups}/schedule");
    let get = || core.call(Method::GET, &backups, Some(&viewer), None, None);
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

    // Editors change the interval and give scheduled backups more slots.
    let (status, body) = patch(
        &editor,
        None,
        json!({ "keep": 10, "interval_hours": 12, "delete_excess": false }),
    )
    .await;
    assert_eq!((status, &body["applied"]), (StatusCode::OK, &json!(true)));
    let (status, _) = patch(
        &editor,
        None,
        json!({ "keep": 16, "interval_hours": null, "delete_excess": false }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "there are 15 slots");

    // Fewer scheduled slots than backups: named first, then deleted, which is the owner's
    // with a ticket.
    let fewer = |delete_excess| json!({ "keep": 1, "interval_hours": null, "delete_excess": delete_excess });
    let (status, body) = patch(&editor, None, fewer(false)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["applied"], false);
    assert_eq!(body["excess"].as_array().unwrap().len(), 1);
    let oldest = body["excess"][0].clone();
    let (status, _) =
        patch(&editor, Some(core.ticket("editor")), fewer(true)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
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

    // A manual backup takes a manual slot; renaming is the editor's.
    let (status, _) = patch(
        &editor,
        None,
        json!({ "keep": 14, "interval_hours": null, "delete_excess": false }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, created) = core
        .call(
            Method::POST,
            &backups,
            Some(&editor),
            None,
            Some(json!({ "name": "One" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = core
        .call(
            Method::POST,
            &backups,
            Some(&editor),
            None,
            Some(json!({ "name": "Two" })),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "the one manual slot is taken");
    let rename = format!("{backups}/{}", created["id"].as_str().unwrap());
    let renamed = Some(json!({ "name": "Renamed" }));
    let (status, _) = core
        .call(Method::PATCH, &rename, Some(&viewer), None, renamed.clone())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = core
        .call(Method::PATCH, &rename, Some(&editor), None, renamed)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) = get().await;
    assert_eq!(body["backups"][0]["name"], "Renamed");
}

#[tokio::test]
async fn only_the_owner_reads_and_changes_a_servers_share_history() {
    let core = core().await;
    let server = core.server.clone();
    let owner = core.sign_in("owner").await;
    let editor = core.sign_in("editor").await;
    let history = format!("/instances/{server}/shared/history");

    for (method, uri) in [
        (Method::GET, history.clone()),
        (Method::POST, format!("{history}/1/restore")),
        (Method::DELETE, format!("{history}/1")),
    ] {
        let (status, _) =
            core.call(method, &uri, Some(&editor), None, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}");
    }
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
    let editor = core.sign_in("editor").await;
    let (status, _) = core
        .call(Method::GET, "/storage", Some(&editor), None, None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, storage) = core
        .call(Method::GET, "/storage", Some(&owner), None, None)
        .await;
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
async fn editors_set_the_servers_name_and_update_channel() {
    let core = core().await;
    let server = core.server.clone();
    let editor = core.sign_in("editor").await;
    let viewer = core.sign_in("viewer").await;
    let detail = format!("/servers/{server}");
    let channel = |detail: &Value| detail["update_channel"].clone();

    let (_, before) = core
        .call(Method::GET, &detail, Some(&viewer), None, None)
        .await;
    assert_eq!(channel(&before), "release");

    let beta = Some(json!({ "update_channel": "beta" }));
    let (status, _) = core
        .call(Method::PATCH, &detail, Some(&viewer), None, beta.clone())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = core
        .call(Method::PATCH, &detail, Some(&editor), None, beta)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, after) = core
        .call(Method::GET, &detail, Some(&viewer), None, None)
        .await;
    assert_eq!(channel(&after), "beta");

    let blank = Some(json!({ "name": "  " }));
    let (_, error) = core
        .call(Method::PATCH, &detail, Some(&editor), None, blank)
        .await;
    assert_eq!(error["error"], "invalid_input");
    let renamed = Some(json!({ "name": " Survival " }));
    let (status, _) = core
        .call(Method::PATCH, &detail, Some(&editor), None, renamed)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, after) = core
        .call(Method::GET, &detail, Some(&viewer), None, None)
        .await;
    assert_eq!(after["name"], "Survival");
    assert_eq!(channel(&after), "beta", "a rename keeps the channel");
}

#[tokio::test]
async fn an_icon_is_saved_for_minecraft_and_the_app_then_reset() {
    let core = core().await;
    let editor = core.sign_in("editor").await;
    let viewer = core.sign_in("viewer").await;
    let uri = format!("/v1/servers/{}/icon", core.server);
    let get = |uri: String, token: String| {
        let request = Request::get(uri)
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let router = core.router.clone();
        async move {
            let response = router.oneshot(request).await.unwrap();
            let status = response.status();
            let bytes =
                response.into_body().collect().await.unwrap().to_bytes();
            (status, bytes)
        }
    };
    let put = |token: &str, body: Vec<u8>| {
        Request::put(&uri)
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/octet-stream")
            .body(Body::from(body))
            .unwrap()
    };

    assert_eq!(
        get(uri.clone(), viewer.clone()).await.0,
        StatusCode::NOT_FOUND
    );

    let mut wide = Vec::new();
    image::RgbaImage::new(1024, 512)
        .write_to(
            &mut std::io::Cursor::new(&mut wide),
            image::ImageFormat::Png,
        )
        .unwrap();
    assert_eq!(
        core.send(put(&viewer, wide.clone())).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        core.send(put(&editor, b"not an image".to_vec())).await.0,
        StatusCode::BAD_REQUEST
    );
    let (status, uploaded) = core.send(put(&editor, wide.clone())).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        uploaded["config"],
        Value::Null,
        "an uploaded image has no editor settings"
    );

    let (status, game) = get(uri.clone(), viewer.clone()).await;
    assert_eq!(status, StatusCode::OK);
    let game = image::load_from_memory(&game).unwrap();
    assert_eq!((game.width(), game.height()), (64, 64));
    let (_, original) =
        get(format!("{uri}?original=true"), viewer.clone()).await;
    let original = image::load_from_memory(&original).unwrap();
    assert_eq!(
        (original.width(), original.height()),
        (512, 256),
        "shrunk to fit, shape kept"
    );
    let server_dir = core.state.servers.paths().server(&core.server);
    assert!(server_dir.join("server-icon.png").exists());

    // An icon made in the icon editor keeps its settings until the icon is replaced.
    let config = json!({ "background": { "type": "color", "value": "#c78aff" }, "symbol": "dusk_block" });
    let boundary = "icon-boundary";
    let mut form = Vec::new();
    form.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"icon.png\"\r\nContent-Type: image/png\r\n\r\n"
        )
        .as_bytes(),
    );
    form.extend_from_slice(&wide);
    form.extend_from_slice(
        format!(
            "\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"config\"\r\n\r\n{config}\r\n--{boundary}--\r\n"
        )
        .as_bytes(),
    );
    let generated = Request::put(format!("{uri}/generated"))
        .header("authorization", format!("Bearer {editor}"))
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(form))
        .unwrap();
    let (status, made) = core.send(generated).await;
    assert_eq!(status, StatusCode::OK, "{made}");
    assert_eq!(made["config"], config);
    let (status, metadata) = core
        .call(
            Method::GET,
            &format!("{uri}/metadata"),
            Some(&viewer),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(metadata, made);
    let (status, _) = core.send(put(&editor, wide)).await;
    assert_eq!(status, StatusCode::OK);
    let (_, metadata) = core
        .call(
            Method::GET,
            &format!("{uri}/metadata"),
            Some(&viewer),
            None,
            None,
        )
        .await;
    assert_eq!(metadata["config"], Value::Null);

    let (status, _) = core
        .call(Method::DELETE, &uri, Some(&editor), None, None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        get(format!("{uri}?original=true"), viewer).await.0,
        StatusCode::NOT_FOUND
    );
    assert!(!server_dir.join("server-icon.png").exists());
}

#[tokio::test]
async fn a_server_exports_as_a_pack_once_per_link() {
    use std::io::Read;

    let core = core().await;
    let server = core.server.clone();
    let editor = core.sign_in("editor").await;
    let viewer = core.sign_in("viewer").await;
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

    let (status, _) = core.link(&viewer, &path).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "a viewer may not export");
    let (status, link) = core.link(&editor, &path).await;
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
        (session("editor", false), false),
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
async fn revoking_a_role_ends_the_console_and_keeps_the_socket() {
    let core = core().await;
    let server = core.server.clone();
    let editor = core.sign_in("editor").await;
    let mut socket = core.events(&editor).await;
    subscribe(&mut socket, json!({ "type": "server", "id": server })).await;
    assert_eq!(next(&mut socket).await.unwrap()["type"], "power");
    let console = json!({ "type": "console", "id": server });
    subscribe(&mut socket, console.clone()).await;

    sqlx::query("UPDATE access SET role = 'viewer' WHERE user_id = 'editor'")
        .execute(&core.state.db)
        .await
        .unwrap();
    core.state.access.changed("editor");
    let ended = next(&mut socket).await.unwrap();
    assert_eq!(ended["type"], "ended");
    assert_eq!(ended["payload"]["topic"], console);
    assert_eq!(ended["payload"]["error"]["error"], "forbidden");

    subscribe(&mut socket, console).await;
    let refused = next(&mut socket).await.expect("the socket stays open");
    assert_eq!(refused["payload"]["error"]["error"], "forbidden");
}

#[tokio::test]
async fn ending_the_session_closes_the_socket() {
    let core = core().await;
    let editor = core.sign_in("editor").await;
    let mut socket = core.events(&editor).await;
    subscribe(&mut socket, json!({ "type": "server", "id": core.server }))
        .await;
    next(&mut socket).await.unwrap();

    let (status, _) = core
        .call(
            Method::DELETE,
            "/sessions/current",
            Some(&editor),
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
    let editor = core.sign_in("editor").await;
    let mut socket = core.events(&editor).await;
    subscribe(&mut socket, json!({ "type": "installs" })).await;
    subscribe(&mut socket, json!({ "type": "server", "id": core.server }))
        .await;

    let running = next(&mut socket).await.unwrap();
    assert_eq!(running["type"], "install_job");
    assert_eq!(running["payload"]["job_id"], job.to_string());
    assert_eq!(next(&mut socket).await.unwrap()["type"], "power");
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
    let editor = core.sign_in("editor").await;
    let mut socket = core.events(&editor).await;
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
