use crate::common;

use reqwest::multipart;

// ── list_mods ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn list_mods_no_instance() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .get(app.url("/instances/00000000-0000-0000-0000-000000000000/mods"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_string());
}

/// Invalid public instance path syntax is rejected with 400 before any DB access.
#[tokio::test]
async fn list_mods_invalid_path() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .get(app.url("/instances/bad:name/mods"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn list_mods_empty() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;
    let res = app
        .client
        .get(app.url(&format!("/instances/{id}/mods")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["mods"], serde_json::json!([]));
}

// ── upload_mod ────────────────────────────────────────────────────────────────

/// Invalid public instance path syntax is rejected before multipart parse.
#[tokio::test]
async fn upload_mod_invalid_path() {
    let app = common::TestApp::spawn().await;
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(b"fake-jar-bytes".to_vec())
            .file_name("test.jar")
            .mime_str("application/java-archive")
            .unwrap(),
    );
    let res = app
        .client
        .post(app.url("/instances/bad:name/mods/upload"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

/// Multipart request with no "file" field returns 400 Bad Request.
#[tokio::test]
async fn upload_mod_missing_field() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;
    let form = multipart::Form::new();
    let res = app
        .client
        .post(app.url(&format!("/instances/{id}/mods/upload")))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_string());
}

/// Upload response body must include ok=true and echo the filename.
/// Then GET /mods must list the uploaded file.
#[tokio::test]
async fn upload_mod_then_list_shows_file() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;

    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(b"PK\x03\x04fake-jar-content".to_vec())
            .file_name("my-mod.jar")
            .mime_str("application/java-archive")
            .unwrap(),
    );
    let upload = app
        .client
        .post(app.url(&format!("/instances/{id}/mods/upload")))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(upload.status(), 200, "upload must succeed");
    let upload_body: serde_json::Value = upload.json().await.unwrap();
    assert_eq!(upload_body["ok"], true, "upload response must have ok=true");
    assert_eq!(
        upload_body["filename"].as_str().unwrap(),
        "my-mod.jar",
        "upload response must echo the original filename"
    );

    // The uploaded file must now appear in the mod list.
    let list: serde_json::Value = app
        .client
        .get(app.url(&format!("/instances/{id}/mods")))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let mods = list["mods"].as_array().unwrap();
    assert_eq!(mods.len(), 1, "uploaded mod must appear in list");
    let filenames: Vec<&str> = mods
        .iter()
        .map(|m| m["filename"].as_str().unwrap())
        .collect();
    assert!(
        filenames.contains(&"my-mod.jar"),
        "my-mod.jar not in mod list: {filenames:?}"
    );
    assert_eq!(
        mods[0]["enabled"], true,
        "freshly uploaded mod must be enabled"
    );
}

// ── delete_mod ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_mod_nonexistent_instance() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .delete(app.url(
            "/instances/00000000-0000-0000-0000-000000000000/mods/some.jar",
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn delete_mod_nonexistent_file() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;
    let res = app
        .client
        .delete(app.url(&format!("/instances/{id}/mods/nonexistent.jar")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_string());
}

/// Invalid public instance path syntax returns 400.
#[tokio::test]
async fn delete_mod_invalid_path() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .delete(app.url("/instances/bad:name/mods/test.jar"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

// ── toggle_mod ────────────────────────────────────────────────────────────────

/// Invalid public instance path syntax returns 400.
#[tokio::test]
async fn toggle_mod_invalid_path() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .patch(app.url("/instances/bad:name/mods/test.jar"))
        .json(&serde_json::json!({ "enabled": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

// ── add_mod ───────────────────────────────────────────────────────────────────

/// Invalid public instance path syntax returns 400.
#[tokio::test]
async fn add_mod_invalid_path() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .post(app.url("/instances/bad:name/mods"))
        .json(&serde_json::json!({ "version_id": "AABBCCDD" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}
