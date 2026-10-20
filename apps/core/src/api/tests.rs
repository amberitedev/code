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
    let servers = Servers::open(paths, db.clone()).await.unwrap();
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
    let content = crate::content::Content::open(
        servers.paths().clone(),
        db.clone(),
        Arc::clone(&servers),
    )
    .unwrap();
    let state = Arc::new(AppState {
        servers,
        access: Arc::new(Access::new(keys)),
        db,
        content,
        uploads: super::files::Uploads::default(),
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
