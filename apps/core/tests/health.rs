mod common;

/// GET /health returns 200 with {"status": "ok"}
#[tokio::test]
async fn health_returns_ok() {
    let app = common::TestApp::spawn().await;
    let res = app.client.get(app.url("/health")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

/// GET /version returns 200 with a version string.
#[tokio::test]
async fn version_returns_semver() {
    let app = common::TestApp::spawn().await;
    let res = app.client.get(app.url("/version")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["version"].is_string());
    assert!(body["name"].is_string());
}

/// GET /java returns 200 with an installations array (may be empty in CI).
#[tokio::test]
async fn java_returns_list() {
    let app = common::TestApp::spawn().await;
    let res = app.client.get(app.url("/java")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["installations"].is_array());
}
