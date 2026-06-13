use crate::common;

use serde_json::json;

// ── List ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn list_instances_empty() {
    let app = common::TestApp::spawn().await;
    let res = app.client.get(app.url("/instances")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["instances"], json!([]));
}

#[tokio::test]
async fn list_instances_shows_created() {
    let app = common::TestApp::spawn().await;
    let mut created_ids = Vec::new();
    for _ in 0..3 {
        created_ids.push(common::create_test_instance(&app).await);
    }
    let res = app.client.get(app.url("/instances")).send().await.unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    let instances = body["instances"].as_array().unwrap();
    assert_eq!(instances.len(), 3);
    assert_eq!(instances[0]["install_status"], "ready");
    assert!(instances[0]["created_at"].is_string());
    assert!(instances[0]["updated_at"].is_string());
    assert!(instances[0]["data_dir"].is_null());
    // Verify the actual IDs are present — not just any 3 items.
    let returned_ids: Vec<&str> = instances
        .iter()
        .map(|v| v["id"].as_str().unwrap())
        .collect();
    for id in &created_ids {
        assert!(
            returned_ids.contains(&id.as_str()),
            "created ID {id} not found in list; got: {returned_ids:?}"
        );
    }
}

// ── Create ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn create_instance_returns_id() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .post(app.url("/instances"))
        .json(&common::default_create_body())
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201);
    let body: serde_json::Value = res.json().await.unwrap();
    let id = body["id"].as_str().expect("id must be a string");

    // Validate UUID format: 8-4-4-4-12 hex groups.
    let parts: Vec<&str> = id.split('-').collect();
    assert_eq!(
        parts.len(),
        5,
        "id must be UUID (5 hyphen-separated groups): {id}"
    );
    assert_eq!(parts[0].len(), 8, "UUID group 0 must be 8 chars: {id}");
    assert_eq!(parts[1].len(), 4, "UUID group 1 must be 4 chars: {id}");
    assert_eq!(parts[2].len(), 4, "UUID group 2 must be 4 chars: {id}");
    assert_eq!(parts[3].len(), 4, "UUID group 3 must be 4 chars: {id}");
    assert_eq!(parts[4].len(), 12, "UUID group 4 must be 12 chars: {id}");

    // The returned ID must be fetchable via GET.
    let fetched: serde_json::Value = app
        .client
        .get(app.url(&format!("/instances/{id}")))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        fetched["id"].as_str().unwrap(),
        id,
        "GET must return same ID as create"
    );
}

#[tokio::test]
async fn create_instance_missing_fields_returns_422() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .post(app.url("/instances"))
        .json(&json!({ "name": "incomplete" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 422);
}

#[tokio::test]
async fn create_instance_bad_loader_returns_422() {
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

// ── Get ───────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_instance_returns_record() {
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

    // Core fields match what was sent in default_create_body().
    assert_eq!(body["id"].as_str().unwrap(), id);
    assert_eq!(body["name"], "test-server");
    assert_eq!(body["status"], "offline");
    assert_eq!(body["install_status"], "ready");
    assert_eq!(body["game_version"], "1.21.1");
    assert_eq!(body["loader"], "vanilla");
    assert_eq!(body["port"], 25565);
    assert_eq!(body["memory"]["min_mb"], 512);
    assert_eq!(body["memory"]["max_mb"], 1024);

    // data_dir uses a stable filesystem-safe slug while the ID remains a UUID.
    let data_dir = body["data_dir"]
        .as_str()
        .expect("data_dir must be a string");
    assert!(
        data_dir.replace('\\', "/").ends_with("/instances/test-server"),
        "data_dir must use the instance name slug; got: {data_dir}"
    );
}

#[tokio::test]
async fn get_instance_not_found_returns_404() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .get(app.url("/instances/00000000-0000-0000-0000-000000000000"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

// ── Delete ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_instance_removes_record() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;

    let del = app
        .client
        .delete(app.url(&format!("/instances/{id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 200);

    let get = app
        .client
        .get(app.url(&format!("/instances/{id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(get.status(), 404);
}

/// Deleting a running instance must return 409 — the actor is still alive.
#[tokio::test]
async fn delete_running_instance_returns_409() {
    let app = common::TestApp::spawn_with_mock().await;
    let id = common::create_test_instance(&app).await;

    app.client
        .post(app.url(&format!("/instances/{id}/start")))
        .send()
        .await
        .unwrap();

    let del = app
        .client
        .delete(app.url(&format!("/instances/{id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(
        del.status(),
        409,
        "deleting a running instance must return 409 Conflict"
    );
    let body: serde_json::Value = del.json().await.unwrap();
    assert!(
        body["error"].as_str().unwrap().contains("stop"),
        "error must mention 'stop'; got: {}",
        body["error"]
    );
}

/// SEC-04 fixed: deleting a non-UUID path now returns 400 Bad Request.
#[tokio::test]
async fn delete_instance_invalid_uuid_returns_400() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .delete(app.url("/instances/not-a-uuid"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        400,
        "SEC-04 fixed: non-UUID path must return 400"
    );
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_string());
}
