mod common;

use serde_json::json;

// ── Properties: GET ───────────────────────────────────────────────────────────

/// GET /instances/:id/properties for an existing instance returns 200 with a
/// properties object that contains at least the keys written by
/// `write_initial_properties` (server-port, eula, online-mode).
#[tokio::test]
async fn get_properties_existing_instance_returns_200() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;

    let res = app
        .client
        .get(app.url(&format!("/instances/{id}/properties")))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["properties"].is_object(), "response must have a 'properties' object");
}

/// The initial properties file must include the `server-port` key.
#[tokio::test]
async fn get_properties_contains_server_port() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;

    let res = app
        .client
        .get(app.url(&format!("/instances/{id}/properties")))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let props = body["properties"].as_object().expect("properties must be an object");
    assert!(
        props.contains_key("server-port"),
        "server-port must be present in initial properties; got keys: {:?}",
        props.keys().collect::<Vec<_>>()
    );
}

/// GET /instances/:id/properties for a nonexistent (but valid UUID) instance
/// returns 404.
#[tokio::test]
async fn get_properties_nonexistent_instance_returns_404() {
    let app = common::TestApp::spawn().await;

    let res = app
        .client
        .get(app.url("/instances/00000000-0000-0000-0000-000000000000/properties"))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 404);
}

/// GET /instances/:id/properties with a non-UUID id returns 400.
/// SEC-04 fix applied to `properties.rs::fetch_data_dir`: UUID is parsed
/// upfront via `InstanceId` → `ParseError` → 400 Bad Request.
#[tokio::test]
async fn get_properties_non_uuid_id_returns_400() {
    let app = common::TestApp::spawn().await;

    let res = app
        .client
        .get(app.url("/instances/not-a-uuid/properties"))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 400);
}

// ── Properties: PATCH ────────────────────────────────────────────────────────

/// PATCH /instances/:id/properties with a valid key returns 200 and lists the
/// updated key in `updated_keys`.
#[tokio::test]
async fn patch_properties_valid_key_returns_200() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;

    let res = app
        .client
        .patch(app.url(&format!("/instances/{id}/properties")))
        .json(&json!({ "max-players": "42" }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let updated = body["updated_keys"].as_array().expect("updated_keys must be an array");
    assert!(
        updated.iter().any(|v| v.as_str() == Some("max-players")),
        "max-players must appear in updated_keys; got: {updated:?}"
    );
}

/// After a PATCH, GET should reflect the new value.
#[tokio::test]
async fn patch_properties_value_is_persisted() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;

    app.client
        .patch(app.url(&format!("/instances/{id}/properties")))
        .json(&json!({ "motd": "Hello Amberite" }))
        .send()
        .await
        .unwrap();

    let res = app
        .client
        .get(app.url(&format!("/instances/{id}/properties")))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let props = body["properties"].as_object().unwrap();
    assert_eq!(
        props.get("motd").and_then(|v| v.as_str()),
        Some("Hello Amberite"),
        "patched motd must be readable back via GET"
    );
}

/// PATCH /instances/:id/properties for a nonexistent instance returns 404.
#[tokio::test]
async fn patch_properties_nonexistent_instance_returns_404() {
    let app = common::TestApp::spawn().await;

    let res = app
        .client
        .patch(app.url("/instances/00000000-0000-0000-0000-000000000000/properties"))
        .json(&json!({ "max-players": "10" }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 404);
}

/// PATCH /instances/:id/properties with a non-UUID id returns 400.
/// SEC-04 fix: `fetch_data_dir` now parses UUID upfront → 400 Bad Request.
#[tokio::test]
async fn patch_properties_non_uuid_id_returns_400() {
    let app = common::TestApp::spawn().await;

    let res = app
        .client
        .patch(app.url("/instances/not-a-uuid/properties"))
        .json(&json!({ "max-players": "10" }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 400);
}

// ── Stats: GET /instances/:id/stats ──────────────────────────────────────────

/// Stats for an offline instance (exists in DB, no active handle) returns 200
/// with all metric fields null — the server is not running so there is nothing
/// to measure. BEH-05 fix: `get_stats` calls `instance_store.get()` when not
/// in the actor map; found in DB → 200 with nulls.
#[tokio::test]
async fn stats_offline_instance_returns_200_with_nulls() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;

    let res = app
        .client
        .get(app.url(&format!("/instances/{id}/stats")))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    // All fields are null for an offline instance.
    assert!(body["cpu_percent"].is_null(), "cpu_percent must be null for offline instance");
    assert!(body["memory_mb"].is_null(), "memory_mb must be null for offline instance");
    assert!(body["player_count"].is_null(), "player_count must be null for offline instance");
    assert!(body["uptime_seconds"].is_null(), "uptime_seconds must be null for offline instance");
}

/// Stats for a nonexistent instance (valid UUID, no DB row) returns 404.
/// BEH-05 fix: `get_stats` calls `instance_store.get()` when not in the actor
/// map → not found in DB → 404 Not Found.
#[tokio::test]
async fn stats_nonexistent_instance_returns_404() {
    let app = common::TestApp::spawn().await;

    let res = app
        .client
        .get(app.url("/instances/00000000-0000-0000-0000-000000000000/stats"))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 404);
}

/// Stats with a non-UUID id returns 404.
///
/// `get_stats` parses the UUID itself; parse failure → `StatsError::NotFound`
/// → 404. Not ideal (400 would be more correct), but this is current behaviour.
#[tokio::test]
async fn stats_non_uuid_id_returns_404() {
    let app = common::TestApp::spawn().await;

    let res = app
        .client
        .get(app.url("/instances/not-a-uuid/stats"))
        .send()
        .await
        .unwrap();

    // stats_service maps UUID parse failure to StatsError::NotFound → 404.
    assert_eq!(res.status(), 404);
}

// ── Diagnostics ───────────────────────────────────────────────────────────────

/// GET /health returns 200 {"status": "ok"}.
#[tokio::test]
async fn diagnostics_health_returns_ok() {
    let app = common::TestApp::spawn().await;
    let res = app.client.get(app.url("/health")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

/// GET /version returns 200 with `version` and `name` fields.
#[tokio::test]
async fn diagnostics_version_has_version_field() {
    let app = common::TestApp::spawn().await;
    let res = app.client.get(app.url("/version")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["version"].is_string(), "version field must be a string");
    assert!(body["name"].is_string(), "name field must be a string");
}

/// GET /java returns 200 with an `installations` array (may be empty in CI).
#[tokio::test]
async fn diagnostics_java_returns_installations_array() {
    let app = common::TestApp::spawn().await;
    let res = app.client.get(app.url("/java")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["installations"].is_array(), "installations must be an array");
}
