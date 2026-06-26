use crate::common;

/// GET /health returns 200 with {"status": "ok"}
#[tokio::test]
async fn health_returns_ok() {
    let app = common::TestApp::spawn().await;
    let res = app.client.get(app.url("/health")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

/// GET /version returns 200 with a semver version string and a name.
#[tokio::test]
async fn version_returns_semver() {
    let app = common::TestApp::spawn().await;
    let res = app.client.get(app.url("/version")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let version = body["version"].as_str().expect("version must be a string");
    let parts: Vec<&str> = version.split('.').collect();
    assert!(
        parts.len() >= 2
            && parts[0].parse::<u32>().is_ok()
            && parts[1].parse::<u32>().is_ok(),
        "version must be semver (e.g. 0.1.0), got: {version}"
    );
    assert!(
        body["name"].as_str().is_some_and(|n| !n.is_empty()),
        "name must be a non-empty string"
    );
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

#[tokio::test]
async fn java_requires_authorization() {
    let app = common::TestApp::spawn().await;
    let client = reqwest::Client::new();
    let res = client.get(app.url("/java")).send().await.unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn network_status_returns_structured_fields() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .get(app.url("/network/status"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["direct_api_url"].is_string());
    assert!(body["upnp"]["state"].is_string());
    assert_eq!(body["minecraft_exposure"]["cloudflare_tunnel"], false);
}
