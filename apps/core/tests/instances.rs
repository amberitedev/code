mod common;

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
    for _ in 0..3 {
        common::create_test_instance(&app).await;
    }
    let res = app.client.get(app.url("/instances")).send().await.unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["instances"].as_array().unwrap().len(), 3);
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
    // BEH-04 fixed: returns 201 Created.
    assert_eq!(res.status(), 201);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["id"].is_string());
}

#[tokio::test]
async fn create_instance_missing_fields_returns_422() {
    let app = common::TestApp::spawn().await;
    // Only "name" provided; game_version, loader, port are required and non-optional.
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
    assert_eq!(body["name"], "test-server");
    assert_eq!(body["status"], "offline");
    assert_eq!(body["game_version"], "1.21.1");
    assert_eq!(body["loader"], "vanilla");
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

    // Subsequent GET must be 404.
    let get = app
        .client
        .get(app.url(&format!("/instances/{id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(get.status(), 404);
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
    assert_eq!(res.status(), 400, "SEC-04 fixed: non-UUID path must return 400");
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_string());
}
