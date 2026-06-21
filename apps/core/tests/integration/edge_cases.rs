use crate::common;

use serde_json::json;

// ── Input validation ──────────────────────────────────────────────────────────

/// Empty string name is rejected — BEH-01 fix: handler validates name is
/// non-empty and returns 400 Bad Request.
#[tokio::test]
async fn create_instance_empty_name() {
    let app = common::TestApp::spawn().await;
    let mut body = common::default_create_body();
    body["name"] = json!("");

    let res = app
        .client
        .post(app.url("/instances"))
        .json(&body)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 400);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_string());
}

/// Missing required field `game_version` → Axum Json extractor returns 422.
#[tokio::test]
async fn create_instance_missing_required_fields() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .post(app.url("/instances"))
        .json(&json!({
            "name": "no-version",
            "loader": "vanilla",
            "port": 25565,
            "memory": { "min_mb": 512, "max_mb": 1024 }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 422);
}

/// Unknown loader string fails serde deserialization → 422 Unprocessable Entity.
#[tokio::test]
async fn create_instance_invalid_loader() {
    let app = common::TestApp::spawn().await;
    let mut body = common::default_create_body();
    body["loader"] = json!("notaloader");
    let res = app
        .client
        .post(app.url("/instances"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 422);
}

/// Port 0 is rejected — BEH-02 fix: handler validates port >= 1 → 400.
#[tokio::test]
async fn create_instance_port_zero() {
    let app = common::TestApp::spawn().await;
    let mut body = common::default_create_body();
    body["port"] = json!(0u16);
    let res = app
        .client
        .post(app.url("/instances"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_string());
}

/// Port 99999 overflows u16 (max 65535) → serde deserialization error → 422.
#[tokio::test]
async fn create_instance_port_too_high() {
    let app = common::TestApp::spawn().await;
    let mut body = common::default_create_body();
    body["port"] = json!(99999u32);
    let res = app
        .client
        .post(app.url("/instances"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 422, "port > 65535 overflows u16 → 422");
}

// ── Idempotency ───────────────────────────────────────────────────────────────

/// First DELETE returns 200; second DELETE of the same (now-gone) ID returns 404.
/// BEH-03 fix: `instance_repo.delete()` checks `rows_affected() == 0` → 404.
#[tokio::test]
async fn delete_instance_twice() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;

    let first = app
        .client
        .delete(app.url(&format!("/instances/{id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), 200, "first delete should succeed");

    let second = app
        .client
        .delete(app.url(&format!("/instances/{id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(
        second.status(),
        404,
        "second delete of a gone instance must return 404"
    );
}

/// After deletion, GET /instances/:id must return 404.
#[tokio::test]
async fn get_deleted_instance() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;

    app.client
        .delete(app.url(&format!("/instances/{id}")))
        .send()
        .await
        .unwrap();

    let res = app
        .client
        .get(app.url(&format!("/instances/{id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

// ── Response shapes ───────────────────────────────────────────────────────────

/// GET /instances/:id response must include all documented fields.
#[tokio::test]
async fn instance_has_all_expected_fields() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;

    let res = app
        .client
        .get(app.url(&format!("/instances/{id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    let body: serde_json::Value = res.json().await.unwrap();

    // Verify all required fields are present and non-null
    assert!(body["id"].is_string(), "missing field: id");
    assert!(body["name"].is_string(), "missing field: name");
    assert!(
        body["game_version"].is_string(),
        "missing field: game_version"
    );
    assert!(body["loader"].is_string(), "missing field: loader");
    assert!(body["port"].is_number(), "missing field: port");
    assert!(
        body["memory"]["min_mb"].is_number(),
        "missing field: memory.min_mb"
    );
    assert!(
        body["memory"]["max_mb"].is_number(),
        "missing field: memory.max_mb"
    );
    assert!(body["status"].is_string(), "missing field: status");
    assert!(body["created_at"].is_string(), "missing field: created_at");

    // Verify values match what was sent in default_create_body
    assert_eq!(body["id"].as_str().unwrap(), id);
    assert_eq!(body["name"], "test-server");
    assert_eq!(body["game_version"], "1.21.1");
    assert_eq!(body["loader"], "vanilla");
    assert_eq!(body["port"], 25565);
    assert_eq!(body["status"], "offline");

    // data_dir uses the instance name slug while id remains the durable UUID.
    let data_dir = body["data_dir"]
        .as_str()
        .expect("data_dir must be a string");
    assert!(
        data_dir
            .replace('\\', "/")
            .ends_with("/instances/test-server"),
        "data_dir must use the instance name slug; got: {data_dir}"
    );
}

/// GET /instances must return `{ "instances": [...] }` shape (not a bare array).
#[tokio::test]
async fn list_instances_returns_array() {
    let app = common::TestApp::spawn().await;
    common::create_test_instance(&app).await;

    let res = app.client.get(app.url("/instances")).send().await.unwrap();
    assert_eq!(res.status(), 200);

    let body: serde_json::Value = res.json().await.unwrap();
    assert!(
        body.is_object(),
        "response must be a JSON object, not a bare array"
    );
    assert!(
        body["instances"].is_array(),
        "response must have an 'instances' array key"
    );
    let arr = body["instances"].as_array().unwrap();
    assert_eq!(arr.len(), 1, "expected 1 instance in list");

    // Each item in the list must have summary fields
    let item = &arr[0];
    assert!(item["id"].is_string(), "list item missing: id");
    assert!(item["name"].is_string(), "list item missing: name");
    assert!(item["status"].is_string(), "list item missing: status");
    assert!(
        item["game_version"].is_string(),
        "list item missing: game_version"
    );
    assert!(item["loader"].is_string(), "list item missing: loader");
    assert!(item["port"].is_number(), "list item missing: port");
}

// ── Concurrency ───────────────────────────────────────────────────────────────

/// Two concurrent POST /instances requests must both succeed and both appear
/// in a subsequent GET /instances list.
#[tokio::test]
async fn create_two_instances_concurrently() {
    let app = common::TestApp::spawn().await;

    let mut body_a = common::default_create_body();
    body_a["name"] = json!("server-alpha");

    let mut body_b = common::default_create_body();
    body_b["name"] = json!("server-beta");

    let (res_a, res_b) = tokio::join!(
        app.client.post(app.url("/instances")).json(&body_a).send(),
        app.client.post(app.url("/instances")).json(&body_b).send(),
    );

    let res_a = res_a.unwrap();
    let res_b = res_b.unwrap();
    assert!(
        res_a.status().is_success(),
        "concurrent create A failed: {}",
        res_a.status()
    );
    assert!(
        res_b.status().is_success(),
        "concurrent create B failed: {}",
        res_b.status()
    );

    let id_a = res_a.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let id_b = res_b.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    assert_ne!(id_a, id_b, "concurrent creates must produce distinct IDs");

    let list_res = app.client.get(app.url("/instances")).send().await.unwrap();
    let list: serde_json::Value = list_res.json().await.unwrap();
    let instances = list["instances"].as_array().unwrap();
    assert_eq!(
        instances.len(),
        2,
        "both concurrent instances must appear in list"
    );

    let ids: Vec<&str> = instances
        .iter()
        .map(|v| v["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&id_a.as_str()), "server-alpha not in list");
    assert!(ids.contains(&id_b.as_str()), "server-beta not in list");

    // Verify the names match — not just that 2 arbitrary items exist.
    let names: Vec<&str> = instances
        .iter()
        .map(|v| v["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"server-alpha"),
        "server-alpha name missing from list"
    );
    assert!(
        names.contains(&"server-beta"),
        "server-beta name missing from list"
    );
}

// ── Auth (prod mode) ──────────────────────────────────────────────────────────

/// All instance CRUD routes require a valid Authorization header in prod mode.
/// Unpaired Core returns 401 on any protected route with no token.
#[tokio::test]
async fn all_protected_routes_require_auth() {
    let app = common::TestApp::spawn_prod_unpaired().await;

    let routes: &[(&str, &str)] = &[
        ("GET", "/instances"),
        ("POST", "/instances"),
        ("GET", "/instances/00000000-0000-0000-0000-000000000000"),
        ("DELETE", "/instances/00000000-0000-0000-0000-000000000000"),
    ];

    for (method, path) in routes {
        let req = match *method {
            "GET" => app.client.get(app.url(path)),
            "POST" => app.client.post(app.url(path)),
            "DELETE" => app.client.delete(app.url(path)),
            _ => unreachable!(),
        };
        let res = req.send().await.unwrap();
        assert_eq!(
            res.status(),
            401,
            "{method} {path} must return 401 without Authorization header"
        );
        let body: serde_json::Value = res.json().await.unwrap();
        assert!(
            body["error"].is_string(),
            "{method} {path}: 401 response must have an 'error' field"
        );
    }
}
