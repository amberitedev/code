mod common;

use serde_json::json;

// ── Helpers ───────────────────────────────────────────────────────────────────

const ZERO_UUID: &str = "00000000-0000-0000-0000-000000000000";
const BAD_UUID: &str = "not-a-uuid";

// ── start ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn start_nonexistent_instance() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .post(app.url(&format!("/instances/{ZERO_UUID}/start")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn start_invalid_uuid() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .post(app.url(&format!("/instances/{BAD_UUID}/start")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_string());
}

// ── stop ──────────────────────────────────────────────────────────────────────

/// BEH-06 fix: stop/kill/restart/command check the DB before returning
/// NotRunning. A UUID that doesn't exist in the DB → 404 Not Found.
/// An instance that exists but is not running → 409 Conflict.
#[tokio::test]
async fn stop_nonexistent_instance() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .post(app.url(&format!("/instances/{ZERO_UUID}/stop")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn stop_invalid_uuid() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .post(app.url(&format!("/instances/{BAD_UUID}/stop")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_string());
}

// ── kill ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn kill_nonexistent_instance() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .post(app.url(&format!("/instances/{ZERO_UUID}/kill")))
        .send()
        .await
        .unwrap();
    // BEH-06 fix: DB check before NotRunning → 404 for nonexistent UUID.
    assert_eq!(res.status(), 404);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn kill_invalid_uuid() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .post(app.url(&format!("/instances/{BAD_UUID}/kill")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_string());
}

// ── restart ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn restart_nonexistent_instance() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .post(app.url(&format!("/instances/{ZERO_UUID}/restart")))
        .send()
        .await
        .unwrap();
    // BEH-06 fix: restart calls stop_instance first; DB check → 404 for
    // nonexistent UUID (stop propagates the 404 before restart can proceed).
    assert_eq!(res.status(), 404);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_string());
}

// ── command ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn command_nonexistent_instance() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .post(app.url(&format!("/instances/{ZERO_UUID}/command")))
        .json(&json!({ "command": "say hello" }))
        .send()
        .await
        .unwrap();
    // BEH-06 fix: DB check before NotRunning → 404 for nonexistent UUID.
    assert_eq!(res.status(), 404);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn command_invalid_uuid() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .post(app.url(&format!("/instances/{BAD_UUID}/command")))
        .json(&json!({ "command": "say hello" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_string());
}

// ── offline-instance paths ────────────────────────────────────────────────────

/// An instance that exists in the DB but has never been started lives only in
/// the DB, not in state.instances. After the BEH-06 fix, stop first checks the
/// DB (found → instance exists), then checks the actor map (absent → NotRunning)
/// → 409 Conflict. This is correct behaviour.
#[tokio::test]
async fn stop_offline_instance() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;

    let res = app
        .client
        .post(app.url(&format!("/instances/{id}/stop")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 409);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_string());
    assert!(
        body["error"].as_str().unwrap().contains("not running"),
        "expected 'not running' in error, got: {}",
        body["error"]
    );
}

/// Sending a command to an offline instance → DB check passes (exists) →
/// actor map miss → NotRunning → 409 Conflict.
#[tokio::test]
async fn command_offline_instance() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;

    let res = app
        .client
        .post(app.url(&format!("/instances/{id}/command")))
        .json(&json!({ "command": "list" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 409);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_string());
    assert!(
        body["error"].as_str().unwrap().contains("not running"),
        "expected 'not running' in error, got: {}",
        body["error"]
    );
}

// ── command — missing body field ──────────────────────────────────────────────

/// POST /instances/:id/command with no JSON body → 422 Unprocessable Entity.
#[tokio::test]
async fn command_missing_body_returns_422() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;

    let res = app
        .client
        .post(app.url(&format!("/instances/{id}/command")))
        .header("content-type", "application/json")
        .body("{}")
        .send()
        .await
        .unwrap();
    // {} is valid JSON but missing required "command" field → serde error → 422.
    assert_eq!(res.status(), 422);
}
