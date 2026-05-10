mod common;

use serde_json::json;

// ── Auth enforcement (production mode) ───────────────────────────────────────

/// In prod mode (dev_mode=false), a protected route without Authorization → 401.
#[tokio::test]
async fn auth_no_header_returns_401_in_prod_mode() {
    let app = common::TestApp::spawn_prod_unpaired().await;
    let res = app.client.get(app.url("/instances")).send().await.unwrap();
    assert_eq!(res.status(), 401);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].as_str().unwrap().contains("missing"), "expected 'missing' in error");
}

/// In prod mode with a fake Bearer token, unpaired Core returns 401 "not paired".
#[tokio::test]
async fn auth_fake_token_unpaired_returns_401() {
    let app = common::TestApp::spawn_prod_unpaired().await;
    let res = app
        .client
        .get(app.url("/instances"))
        .header("Authorization", "Bearer fake.jwt.token")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(
        body["error"].as_str().unwrap().contains("paired"),
        "expected 'paired' in error, got: {}",
        body["error"]
    );
}

/// Unauthenticated endpoints (health, version, setup/status) work in prod mode.
#[tokio::test]
async fn public_routes_accessible_without_auth_in_prod_mode() {
    let app = common::TestApp::spawn_prod_unpaired().await;
    let health = app.client.get(app.url("/health")).send().await.unwrap();
    assert_eq!(health.status(), 200);
    let status = app.client.get(app.url("/setup/status")).send().await.unwrap();
    assert_eq!(status.status(), 200);
}

// ── Pairing brute-force (SEC-01) ──────────────────────────────────────────────

/// SEC-01 fixed: pairing endpoint locks out after 5 wrong attempts (returns 429).
#[tokio::test]
async fn pairing_brute_force_locked_out_after_five_failures() {
    // Must use prod mode — dev mode has no pairing code and returns 400 immediately.
    let app = common::TestApp::spawn_prod_unpaired().await;
    // Send 5 wrong codes — each returns 401.
    for i in 0..5u32 {
        let wrong_code = format!("{:06}", i);
        let res = app
            .client
            .post(app.url("/setup"))
            .json(&json!({
                "code": wrong_code,
                "supabase_url": "https://test.supabase.co",
                "owner_user_id": "attacker"
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 401, "attempt {i}: expected 401 before lockout");
    }
    // 6th attempt: locked out — must return 429.
    let res = app
        .client
        .post(app.url("/setup"))
        .json(&json!({
            "code": "999999",
            "supabase_url": "https://test.supabase.co",
            "owner_user_id": "attacker"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 429, "SEC-01: 6th attempt must return 429 Too Many Requests");
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].as_str().unwrap().contains("too many"), "error message should mention 'too many'");
}

/// Pairing code of all zeros is rejected when the real code differs.
#[tokio::test]
async fn pairing_empty_string_code_rejected() {
    // Must use prod mode — dev mode has no pairing code.
    let app = common::TestApp::spawn_prod_unpaired().await;
    let res = app
        .client
        .post(app.url("/setup"))
        .json(&json!({
            "code": "",
            "supabase_url": "https://test.supabase.co",
            "owner_user_id": "attacker"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

// ── DELETE with invalid UUID (SEC-04) ─────────────────────────────────────────

/// Deleting a valid zero UUID with no matching row returns 404 Not Found.
/// BEH-03 fixed: delete of non-existent record returns 404, not 200.
#[tokio::test]
async fn delete_zero_uuid_nonexistent_returns_404() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .delete(app.url("/instances/00000000-0000-0000-0000-000000000000"))
        .send()
        .await
        .unwrap();
    // Valid UUID format but no matching row → 404 Not Found.
    assert_eq!(res.status(), 404);
}

/// SEC-04 fixed: non-UUID paths for DELETE /instances/:id return 400 Bad Request.
#[tokio::test]
async fn delete_non_uuid_path_returns_400() {
    let app = common::TestApp::spawn().await;
    let cases = ["not-a-uuid", "'; DROP TABLE instances; --", "1=1"];
    for case in cases {
        let res = app
            .client
            .delete(app.url(&format!("/instances/{case}")))
            .send()
            .await
            .unwrap();
        assert_eq!(
            res.status(),
            400,
            "SEC-04: non-UUID path {case:?} must return 400"
        );
    }
}

/// URL path traversal via `..` segments is prevented by HTTP stack normalization.
/// `/instances/../admin` normalizes to `/admin` → 404 (no such route).
/// This is correct behavior at the routing layer (not a SEC-04 bypass).
#[tokio::test]
async fn delete_dotdot_segment_normalized_to_404() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .delete(app.url("/instances/../admin"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404, "path traversal via .. must not match any route");
}

// ── SQL injection safety ──────────────────────────────────────────────────────

/// Parameterized queries prevent SQL injection in the `name` field.
/// The malicious string must be stored and retrieved as-is, not executed.
#[tokio::test]
async fn sql_injection_in_name_is_safe() {
    let app = common::TestApp::spawn().await;
    let evil_name = "'; DROP TABLE instances; --";
    let mut body = common::default_create_body();
    body["name"] = json!(evil_name);

    let create = app
        .client
        .post(app.url("/instances"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), 201, "create should succeed despite injection name");

    let id = create.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let get = app
        .client
        .get(app.url(&format!("/instances/{id}")))
        .send()
        .await
        .unwrap();
    assert_eq!(get.status(), 200, "instances table must still exist");
    let got_name = get.json::<serde_json::Value>().await.unwrap()["name"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(got_name, evil_name, "name must be stored verbatim");
}
