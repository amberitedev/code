mod common;

use serde_json::json;

/// GET /setup/status returns {"paired": false} when not yet paired (prod mode).
#[tokio::test]
async fn setup_status_unpaired() {
    // Dev mode always reports paired=true; use prod mode for this test.
    let app = common::TestApp::spawn_prod_unpaired().await;
    let res = app.client.get(app.url("/setup/status")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["paired"], false);
}

/// GET /setup/status returns {"paired": true} when already paired.
#[tokio::test]
async fn setup_status_paired() {
    let app = common::TestApp::spawn_paired().await;
    let res = app.client.get(app.url("/setup/status")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["paired"], true);
}

/// POST /setup with the wrong code returns 401 (prod mode — has a real pairing code).
#[tokio::test]
async fn setup_wrong_code_returns_401() {
    // Dev mode has no pairing code (returns 400 immediately); use prod mode.
    let app = common::TestApp::spawn_prod_unpaired().await;
    let res = app
        .client
        .post(app.url("/setup"))
        .json(&json!({
            "code": "000000",
            "supabase_url": "https://test.supabase.co",
            "owner_user_id": "user-123"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

/// POST /setup when already paired returns 400 (pairing_code is None → BadRequest).
#[tokio::test]
async fn setup_already_paired_returns_400() {
    let app = common::TestApp::spawn_paired().await;
    let res = app
        .client
        .post(app.url("/setup"))
        .json(&json!({
            "code": "000000",
            "supabase_url": "https://test.supabase.co",
            "owner_user_id": "user-123"
        }))
        .send()
        .await
        .unwrap();
    // Handler returns ApiError::BadRequest("Core is already paired") → 400
    assert_eq!(res.status(), 400);
}

/// POST /setup with the correct pairing code succeeds and marks Core as paired.
#[tokio::test]
async fn setup_correct_code_pairs_core() {
    let app = common::TestApp::spawn_prod_unpaired().await;
    let code = app
        .pairing_code
        .as_deref()
        .expect("spawn_prod_unpaired must populate pairing_code");

    let res = app
        .client
        .post(app.url("/setup"))
        .json(&json!({
            "code": code,
            "supabase_url": "https://test.supabase.co",
            "owner_user_id": "user-123"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["ok"], true);

    // After a successful pairing, /setup/status must report paired = true.
    let status_res = app.client.get(app.url("/setup/status")).send().await.unwrap();
    assert_eq!(status_res.status(), 200);
    let status_body: serde_json::Value = status_res.json().await.unwrap();
    assert_eq!(status_body["paired"], true);
}
