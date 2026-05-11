use crate::common;

use serde_json::Value;
use tokio::fs;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Fetch data_dir for an instance from GET /instances/:id.
async fn get_data_dir(app: &common::TestApp, id: &str) -> std::path::PathBuf {
    let body: Value = app
        .client
        .get(app.url(&format!("/instances/{id}")))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    std::path::PathBuf::from(body["data_dir"].as_str().unwrap())
}

// ── List logs ─────────────────────────────────────────────────────────────────

/// scan_dir silently returns [] when the logs directory does not exist.
#[tokio::test]
async fn list_logs_empty_when_no_dir() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;
    let res = app
        .client
        .get(app.url(&format!("/instances/{id}/logs")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["logs"], serde_json::json!([]));
}

/// Files with .log and .log.gz extensions are included; others are excluded.
#[tokio::test]
async fn list_logs_shows_log_files() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;
    let data_dir = get_data_dir(&app, &id).await;

    let logs_dir = data_dir.join("logs");
    fs::create_dir_all(&logs_dir).await.unwrap();
    let log_content = b"server started\n"; // 15 bytes
    fs::write(logs_dir.join("latest.log"), log_content).await.unwrap();
    fs::write(logs_dir.join("2024-01-01-1.log.gz"), b"\x1f\x8b").await.unwrap();
    fs::write(logs_dir.join("debug.txt"), b"ignored").await.unwrap();

    let res = app
        .client
        .get(app.url(&format!("/instances/{id}/logs")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    let logs = body["logs"].as_array().unwrap();
    assert_eq!(logs.len(), 2, "expected 2 log entries (.log + .log.gz), got {}", logs.len());

    let names: Vec<&str> = logs.iter().map(|l| l["filename"].as_str().unwrap()).collect();
    assert!(names.contains(&"latest.log"), "latest.log must be in list");
    assert!(names.contains(&"2024-01-01-1.log.gz"), ".log.gz must be in list");
    assert!(!names.contains(&"debug.txt"), ".txt must be excluded from log list");

    // size_bytes must match the actual bytes written to disk.
    let latest = logs.iter().find(|l| l["filename"] == "latest.log").unwrap();
    assert_eq!(
        latest["size_bytes"],
        log_content.len() as u64,
        "size_bytes must equal actual file size"
    );
    assert!(latest["modified_at"].is_string(), "modified_at must be a string");
}

/// GET /instances/:nonexistent/logs returns 404.
#[tokio::test]
async fn list_logs_instance_not_found() {
    let app = common::TestApp::spawn().await;
    let res = app
        .client
        .get(app.url("/instances/00000000-0000-0000-0000-000000000000/logs"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

// ── Read log ──────────────────────────────────────────────────────────────────

/// A valid .log file is returned as text/plain with its content.
#[tokio::test]
async fn read_log_returns_content() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;
    let data_dir = get_data_dir(&app, &id).await;

    let logs_dir = data_dir.join("logs");
    fs::create_dir_all(&logs_dir).await.unwrap();
    fs::write(logs_dir.join("latest.log"), b"[INFO] Server started\n").await.unwrap();

    let res = app
        .client
        .get(app.url(&format!("/instances/{id}/logs/latest.log")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let ct = res.headers()["content-type"].to_str().unwrap();
    assert!(ct.contains("text/plain"), "expected text/plain, got {ct}");
    let body = res.text().await.unwrap();
    assert_eq!(body, "[INFO] Server started\n");
}

/// Requesting a file that does not exist returns 404.
#[tokio::test]
async fn read_log_not_found() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;
    let res = app
        .client
        .get(app.url(&format!("/instances/{id}/logs/nonexistent.log")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

/// SEC-03: a filename containing `..%2F` (URL-encoded slash) must be rejected.
/// The guard checks for both `..` and `/` after percent-decoding.
#[tokio::test]
async fn read_log_path_traversal_encoded_slash_rejected() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;
    // %2F decodes to '/' in axum Path extractor → contains('/') → InvalidPath → 400
    let url = format!("{}/instances/{id}/logs/..%2Fetc%2Fpasswd", app.base_url);
    let res = app.client.get(&url).send().await.unwrap();
    assert_eq!(res.status(), 400, "SEC-03: encoded path traversal must be rejected");
}

/// A filename with a non-.log extension is rejected with 400.
#[tokio::test]
async fn read_log_wrong_extension_rejected() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;
    let res = app
        .client
        .get(app.url(&format!("/instances/{id}/logs/server.properties")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400, "non-.log extension must be rejected");
}

/// A filename that literally contains `..` is rejected.
#[tokio::test]
async fn read_log_dotdot_in_filename_rejected() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;
    // "evil..log" contains ".." → rejected before filesystem access
    let res = app
        .client
        .get(app.url(&format!("/instances/{id}/logs/evil..log")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400, "filename containing .. must be rejected");
}

// ── Crash reports ─────────────────────────────────────────────────────────────

/// crash-reports dir absent → returns 200 with empty list.
#[tokio::test]
async fn list_crash_reports_empty() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;
    let res = app
        .client
        .get(app.url(&format!("/instances/{id}/crash-reports")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["crash_reports"], serde_json::json!([]));
}

/// A valid crash report .txt file is returned.
#[tokio::test]
async fn read_crash_report_returns_content() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;
    let data_dir = get_data_dir(&app, &id).await;

    let cr_dir = data_dir.join("crash-reports");
    fs::create_dir_all(&cr_dir).await.unwrap();
    fs::write(cr_dir.join("crash-2024-01-01_12.00.00-server.txt"), b"Crash details here\n")
        .await
        .unwrap();

    let res = app
        .client
        .get(app.url(&format!(
            "/instances/{id}/crash-reports/crash-2024-01-01_12.00.00-server.txt"
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(res.text().await.unwrap(), "Crash details here\n");
}

/// Crash reports only accept .txt extension; .log is rejected.
#[tokio::test]
async fn read_crash_report_wrong_ext_rejected() {
    let app = common::TestApp::spawn().await;
    let id = common::create_test_instance(&app).await;
    let res = app
        .client
        .get(app.url(&format!("/instances/{id}/crash-reports/dump.log")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400, "non-.txt extension must be rejected");
}
