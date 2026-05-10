mod common;

use serde_json::json;
use tokio::time::{sleep, Duration};

// ── Start ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn start_instance_returns_200() {
    let app = common::TestApp::spawn_with_mock().await;
    let id = common::create_test_instance(&app).await;

    let res = app
        .client
        .post(app.url(&format!("/instances/{id}/start")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["ok"], true);
}

#[tokio::test]
async fn start_instance_transitions_status_to_starting() {
    let app = common::TestApp::spawn_with_mock().await;
    let id = common::create_test_instance(&app).await;

    app.client
        .post(app.url(&format!("/instances/{id}/start")))
        .send()
        .await
        .unwrap();

    // DB should reflect "starting" immediately after start returns.
    let body: serde_json::Value = app
        .client
        .get(app.url(&format!("/instances/{id}")))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let status = body["status"].as_str().unwrap();
    // MockSpawner never emits "Done (" so stays "starting" (not "running").
    assert!(
        status == "starting" || status == "running",
        "unexpected status: {status}"
    );
}

#[tokio::test]
async fn start_nonexistent_instance_returns_404() {
    let app = common::TestApp::spawn_with_mock().await;
    let res = app
        .client
        .post(app.url("/instances/00000000-0000-0000-0000-000000000000/start"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn start_instance_twice_returns_409() {
    let app = common::TestApp::spawn_with_mock().await;
    let id = common::create_test_instance(&app).await;

    let r1 = app
        .client
        .post(app.url(&format!("/instances/{id}/start")))
        .send()
        .await
        .unwrap();
    assert_eq!(r1.status(), 200);

    let r2 = app
        .client
        .post(app.url(&format!("/instances/{id}/start")))
        .send()
        .await
        .unwrap();
    assert_eq!(r2.status(), 409, "second start must return 409 Already Running");
}

// ── Kill ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn kill_nonexistent_instance_returns_404() {
    let app = common::TestApp::spawn_with_mock().await;
    let res = app
        .client
        .post(app.url("/instances/00000000-0000-0000-0000-000000000000/kill"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn kill_offline_instance_returns_409() {
    let app = common::TestApp::spawn_with_mock().await;
    let id = common::create_test_instance(&app).await;

    let res = app
        .client
        .post(app.url(&format!("/instances/{id}/kill")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 409);
}

#[tokio::test]
async fn kill_running_instance_transitions_to_offline() {
    let app = common::TestApp::spawn_with_mock().await;
    let id = common::create_test_instance(&app).await;

    app.client
        .post(app.url(&format!("/instances/{id}/start")))
        .send()
        .await
        .unwrap();

    let kill = app
        .client
        .post(app.url(&format!("/instances/{id}/kill")))
        .send()
        .await
        .unwrap();
    assert_eq!(kill.status(), 200);

    // Give the actor task one scheduler tick to process the Kill cmd and exit.
    sleep(Duration::from_millis(50)).await;

    let body: serde_json::Value = app
        .client
        .get(app.url(&format!("/instances/{id}")))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["status"], "offline", "killed instance must be offline");
}

// ── Command ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn send_command_to_offline_instance_returns_409() {
    let app = common::TestApp::spawn_with_mock().await;
    let id = common::create_test_instance(&app).await;

    let res = app
        .client
        .post(app.url(&format!("/instances/{id}/command")))
        .json(&json!({ "command": "say hello" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 409);
}

#[tokio::test]
async fn send_command_to_running_instance_returns_200() {
    let app = common::TestApp::spawn_with_mock().await;
    let id = common::create_test_instance(&app).await;

    app.client
        .post(app.url(&format!("/instances/{id}/start")))
        .send()
        .await
        .unwrap();

    let res = app
        .client
        .post(app.url(&format!("/instances/{id}/command")))
        .json(&json!({ "command": "say hello" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
}

// ── Stop (offline guard) ──────────────────────────────────────────────────────

#[tokio::test]
async fn stop_nonexistent_instance_returns_404() {
    let app = common::TestApp::spawn_with_mock().await;
    let res = app
        .client
        .post(app.url("/instances/00000000-0000-0000-0000-000000000000/stop"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn stop_offline_instance_returns_409() {
    let app = common::TestApp::spawn_with_mock().await;
    let id = common::create_test_instance(&app).await;

    let res = app
        .client
        .post(app.url(&format!("/instances/{id}/stop")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 409);
}
