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

use super::{AppState, router};
use crate::{
    access::{
        self, Access,
        keys::Keys,
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
    state: Arc<AppState>,
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
    let db = db::memory().await;
    let core_id = db::core_id(&db).await.unwrap();
    sqlx::query("UPDATE core SET owner_user_id = 'owner'")
        .execute(&db)
        .await
        .unwrap();
    let servers = Servers::open(paths.clone(), db.clone()).await.unwrap();
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
    let keys = Keys::load(db.clone(), reqwest::Client::new(), None)
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
    let content = crate::content::Content::open(
        servers.paths().clone(),
        db.clone(),
        Arc::clone(&servers),
        Arc::clone(&backups),
    )
    .unwrap();
    let update = Arc::new(Updater::new(
        paths,
        db.clone(),
        reqwest::Client::new(),
        String::new(),
    ));
    let state = Arc::new(AppState {
        servers,
        access: Arc::new(Access::new(keys)),
        network: Arc::new(crate::network::forward::Network::off(db.clone())),
        db,
        content,
        backups,
        uploads: super::files::Uploads::default(),
        update,
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
            request = request.header(access::TICKET_HEADER, ticket);
        }
        let request = match body {
            Some(body) => request
                .header("content-type", "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        };
        self.send(request.unwrap()).await
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
        sqlx::query_scalar(sql)
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

    let get = format!("/modrinth/v0/servers/{server}");
    let (status, _) = core
        .call(Method::GET, &get, Some(&viewer), None, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = core
        .call(Method::GET, &get, Some(&stranger), None, None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "no role reads as no server");

    let console = format!("/modrinth/v0/servers/{server}/ws");
    let (status, _) = core
        .call(Method::GET, &console, Some(&viewer), None, None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "viewers get no console");
    let (status, _) = core
        .call(Method::GET, &console, Some(&editor), None, None)
        .await;
    assert_eq!(status, StatusCode::OK);

    let power = format!("/modrinth/v0/servers/{server}/power");
    let start = Some(json!({ "action": "Start" }));
    let (status, _) = core
        .call(Method::POST, &power, Some(&viewer), None, start.clone())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = core
        .call(Method::POST, &power, Some(&editor), None, start)
        .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "allowed, but nothing is installed to start"
    );

    let create = Some(json!({ "name": "B" }));
    let (status, _) = core
        .call(
            Method::POST,
            "/local/servers",
            Some(&editor),
            None,
            create.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = core
        .call(Method::POST, "/local/servers", Some(&owner), None, create)
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
        .call(
            Method::GET,
            "/modrinth/v0/servers",
            Some(&editor),
            None,
            None,
        )
        .await;
    assert_eq!(
        list["servers"].as_array().unwrap().len(),
        1,
        "friends see only their servers"
    );
    let (_, list) = core
        .call(
            Method::GET,
            "/modrinth/v0/servers",
            Some(&owner),
            None,
            None,
        )
        .await;
    assert_eq!(list["servers"].as_array().unwrap().len(), 2);
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
    let delete = format!("/local/servers/{}", core.server);
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
    let fs = format!("/servers/{server}/modrinth/v0/fs");
    let world = format!("/v1/servers/{server}/worlds/{server}");

    // Viewers see the content list and what the owner edited, nothing else.
    let (status, addons) = core
        .call(
            Method::GET,
            &format!("{world}/addons"),
            Some(&viewer),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(addons["addons"], json!([]));
    for (method, uri) in [
        (Method::GET, format!("{fs}/list?path=/&page=1&page_size=10")),
        (Method::GET, format!("{world}/properties")),
        (Method::POST, format!("{world}/content/repair")),
    ] {
        let (status, _) =
            core.call(method, &uri, Some(&viewer), None, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}");
    }
    let (status, _) = core
        .call(
            Method::GET,
            &format!("/servers/{server}/content/edited"),
            Some(&viewer),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = core
        .call(
            Method::GET,
            &format!("/v1/servers/{server}/worlds/another/addons"),
            Some(&viewer),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "a server has one world");

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
        &format!("{fs}/download?path=/../../db/copal.db"),
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
            &format!("{world}/properties"),
            Some(&editor),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read["known"]["motd"], "Edited");

    // A file the pack placed and the owner changed is listed as edited.
    sqlx::query(
        "INSERT INTO content_files (server_id, path, origin, sha1) VALUES (?, 'config/a.toml', 'pack', 'as placed')",
    )
    .bind(&server)
    .execute(&core.state.db)
    .await
    .unwrap();
    let (_, edited) = core
        .call(
            Method::GET,
            &format!("/servers/{server}/content/edited"),
            Some(&viewer),
            None,
            None,
        )
        .await;
    assert_eq!(
        edited,
        json!({ "files": [{ "path": "config/a.toml", "deleted": false, "pending": null }] })
    );
}

#[tokio::test]
async fn an_upload_over_a_pack_file_counts_as_the_owners_edit() {
    let core = core().await;
    let server = core.server.clone();
    let editor = core.sign_in("editor").await;
    sqlx::query(
        "INSERT INTO content_files (server_id, path, origin, sha1, project_id, version_id) \
         VALUES (?, 'mods/a.jar', 'pack', 'as placed', 'P', 'V')",
    )
    .bind(&server)
    .execute(&core.state.db)
    .await
    .unwrap();
    let boundary = "copal-test";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.jar\"\r\n\r\npatched\r\n--{boundary}--\r\n"
    );
    let request = Request::builder()
        .method(Method::POST)
        .uri(format!(
            "/servers/{server}/v1/worlds/{server}/content/upload-addon-file"
        ))
        .header("authorization", format!("Bearer {editor}"))
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    let (status, _) = core.send(request).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let row: (String, String) = sqlx::query_as(
        "SELECT origin, sha1 FROM content_files WHERE path = 'mods/a.jar'",
    )
    .fetch_one(&core.state.db)
    .await
    .unwrap();
    assert_eq!(row, ("pack".into(), "as placed".into()));
    let (_, edited) = core
        .call(
            Method::GET,
            &format!("/servers/{server}/content/edited"),
            Some(&editor),
            None,
            None,
        )
        .await;
    assert_eq!(edited["files"][0]["path"], "mods/a.jar");
}

#[tokio::test]
async fn restores_need_the_owners_fresh_ticket_and_downloads_a_link_or_a_session()
 {
    let core = core().await;
    let server = core.server.clone();
    let owner = core.sign_in("owner").await;
    let editor = core.sign_in("editor").await;
    let viewer = core.sign_in("viewer").await;
    let queue = format!("/v1/servers/{server}/worlds/{server}/backups-queue");

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
    assert_eq!(list["backups"][0]["status"], "done");
    assert_eq!(list["backups"][0]["history"][0]["state"], "completed");

    let restore = format!("{queue}/{backup}/restore");
    let (status, _) = core
        .call(
            Method::POST,
            &restore,
            Some(&editor),
            Some(&core.ticket("editor")),
            name.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "only the owner restores");
    let (status, error) = core
        .call(Method::POST, &restore, Some(&owner), None, name.clone())
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
            name,
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);

    // A link token works once, for the backup it names.
    let download = format!(
        "/servers/{server}/v1/worlds/{server}/backups/{backup}/download"
    );
    let (status, _) = core
        .call(
            Method::GET,
            &format!("{download}/authorize"),
            Some(&viewer),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, token) = core
        .call(
            Method::GET,
            &format!("{download}/authorize"),
            Some(&editor),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let link = format!("{download}?token={}", token["token"].as_str().unwrap());
    let request = || Request::builder().uri(&link).body(Body::empty()).unwrap();
    let response = core.router.clone().oneshot(request()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert!(bytes.starts_with(b"PK"), "a zip");
    let (status, _) = core.send(request()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "a second use");
    let (status, _) = core
        .call(Method::GET, &download, Some(&viewer), None, None)
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a viewer's session does not download"
    );

    // One file of the server, the same way; the token names the file.
    std::fs::write(
        core.state
            .servers
            .paths()
            .server(&server)
            .join("server.properties"),
        "motd=Hi\n",
    )
    .unwrap();
    let file =
        format!("/servers/{server}/v1/worlds/{server}/files/contents-raw");
    let (status, token) = core
        .call(
            Method::GET,
            &format!("{file}/authorize?path=server.properties"),
            Some(&editor),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let link = format!("{file}?token={}", token["token"].as_str().unwrap());
    let response = core
        .router
        .clone()
        .oneshot(Request::builder().uri(&link).body(Body::empty()).unwrap())
        .await
        .unwrap();
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
async fn each_role_is_shown_its_own_permissions() {
    let core = core().await;
    let uri = format!("/modrinth/v0/servers/{}", core.server);
    for (user, expected) in [
        ("owner", 0xFFFF_FFFF_FFFF_8000),
        ("editor", 0xFE00_0000_0000_0000),
        ("viewer", 1 << 63),
    ] {
        let token = core.sign_in(user).await;
        let (_, server) =
            core.call(Method::GET, &uri, Some(&token), None, None).await;
        assert_eq!(
            server["current_user_permissions"].as_u64(),
            Some(expected),
            "{user}"
        );
        // The App reads this as a JavaScript number: it must survive a double exactly.
        assert_eq!(expected as f64 as u64, expected);
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
    let queue = format!("/v1/servers/{server}/worlds/{server}/backups-queue");
    let options = format!("{queue}/with-options");
    let get = || core.call(Method::GET, &options, Some(&viewer), None, None);
    let patch = |token, ticket: Option<String>, acknowledged: bool, body| {
        let core = &core;
        let options = &options;
        async move {
            let path = if acknowledged {
                format!("{options}?ar=true")
            } else {
                options.clone()
            };
            core.call(
                Method::PATCH,
                &path,
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
        body["options"],
        json!({ "split_automatic": 7, "split_manual": 8, "limit": 15, "interval": 24 })
    );
    assert!(!body["next_auto_backup_at"].is_null());

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
        .filter(|backup| backup["automated"] == true)
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
        false,
        json!({ "split_automatic": 10, "split_manual": 5, "interval": 12 }),
    )
    .await;
    assert_eq!((status, &body["applied"]), (StatusCode::OK, &json!(true)));
    let (status, _) = patch(
        &editor,
        None,
        false,
        json!({ "split_automatic": 1, "split_manual": 1 }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "the slots add up to 15");

    // Fewer scheduled slots than backups: named first, then deleted, which is the owner's
    // with a ticket.
    let fewer = json!({ "split_automatic": 1, "split_manual": 14 });
    let (status, body) = patch(&editor, None, false, fewer.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["applied"], false);
    assert_eq!(body["backup_ids"].as_array().unwrap().len(), 1);
    let oldest = body["backup_ids"][0].clone();
    let (status, _) =
        patch(&editor, Some(core.ticket("editor")), true, fewer.clone()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, error) = patch(&owner, None, true, fewer.clone()).await;
    assert_eq!(
        (status, error["error"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("ticket_required"))
    );
    let (_, body) = get().await;
    assert_eq!(
        body["options"]["split_automatic"], 10,
        "unchanged until then"
    );
    let (status, body) =
        patch(&owner, Some(core.ticket("owner")), true, fewer).await;
    assert_eq!((status, &body["applied"]), (StatusCode::OK, &json!(true)));
    let (_, body) = get().await;
    assert_eq!(body["options"]["split_automatic"], 1);
    let backups = body["backups"].as_array().unwrap();
    assert_eq!(backups.len(), 1);
    assert_ne!(backups[0]["id"], oldest, "the oldest went");

    // Off: nothing is due, every slot is manual, and the last scheduled backup went too.
    let (status, body) = patch(
        &owner,
        Some(core.ticket("owner")),
        true,
        json!({ "split_automatic": 0, "split_manual": 15 }),
    )
    .await;
    assert_eq!((status, &body["applied"]), (StatusCode::OK, &json!(true)));
    let (_, body) = get().await;
    assert!(body["next_auto_backup_at"].is_null());
    assert_eq!(body["options"]["split_manual"], 15);
    assert!(body["backups"].as_array().unwrap().is_empty());

    // A manual backup takes a manual slot; renaming is the editor's.
    let (status, _) = patch(
        &editor,
        None,
        false,
        json!({ "split_automatic": 14, "split_manual": 1 }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, created) = core
        .call(
            Method::POST,
            &queue,
            Some(&editor),
            None,
            Some(json!({ "name": "One" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = core
        .call(
            Method::POST,
            &queue,
            Some(&editor),
            None,
            Some(json!({ "name": "Two" })),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "the one manual slot is taken");
    let rename = format!(
        "/v1/servers/{server}/worlds/{server}/backups/{}",
        created["id"].as_str().unwrap()
    );
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
async fn a_pack_update_takes_an_empty_body_sent_as_json() {
    let core = core().await;
    let server = core.server.clone();
    let editor = core.sign_in("editor").await;
    let uri =
        format!("/v1/servers/{server}/worlds/{server}/content/modpack/update");
    // The App's client sends `Content-Type: application/json` with no body. The server follows
    // no pack, so getting past the body means 404.
    let send = |body: &'static str| {
        Request::builder()
            .method(Method::POST)
            .uri(&uri)
            .header("authorization", format!("Bearer {editor}"))
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap()
    };
    let (status, _) = core.send(send("")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = core.send(send(r#"{"force":true}"#)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = core.send(send("{")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
