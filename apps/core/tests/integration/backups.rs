use std::{fs::File, io::Write};

use crate::common::{create_test_instance, TestApp};
use serde_json::json;
use zip::{write::SimpleFileOptions, ZipWriter};

#[tokio::test]
async fn created_backup_reports_metadata() {
    let app = TestApp::spawn().await;
    let instance_id = create_test_instance(&app).await;

    let res = app
        .client
        .post(app.url(&format!("/instances/{instance_id}/backups")))
        .json(&json!({ "name": "manual test" }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body = res.json::<serde_json::Value>().await.unwrap();
    assert_eq!(body["hot"], false);
    assert_eq!(body["consistency"], "offline");
    assert_eq!(body["trigger"], "manual");
}

#[tokio::test]
async fn restore_requires_backup_to_belong_to_instance() {
    let app = TestApp::spawn().await;
    let first_id = create_test_instance(&app).await;
    let second_id = create_test_instance(&app).await;
    let backup_id = "11111111-1111-4111-8111-111111111111";
    write_zip(
        &app.state.config.data_dir.join("backups").join(&first_id),
        backup_id,
        &[("server.properties", b"server-port=25565\n".as_slice())],
    );
    insert_backup(&app, &first_id, backup_id).await;

    let res = app
        .client
        .post(app.url(&format!(
            "/instances/{second_id}/backups/{backup_id}/restore"
        )))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn restore_path_traversal_fails_without_replacing_data_dir() {
    let app = TestApp::spawn().await;
    let instance_id = create_test_instance(&app).await;
    let backup_id = "22222222-2222-4222-8222-222222222222";
    let record = app
        .state
        .instance_store
        .get(&instance_id.parse().unwrap())
        .await
        .unwrap();
    let marker = std::path::Path::new(&record.data_dir).join("marker.txt");
    tokio::fs::write(&marker, "still here").await.unwrap();
    write_zip(
        &app.state.config.data_dir.join("backups").join(&instance_id),
        backup_id,
        &[("../evil.txt", b"evil".as_slice())],
    );
    insert_backup(&app, &instance_id, backup_id).await;

    let res = app
        .client
        .post(app.url(&format!(
            "/instances/{instance_id}/backups/{backup_id}/restore"
        )))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 400);
    assert_eq!(
        tokio::fs::read_to_string(&marker).await.unwrap(),
        "still here"
    );
}

#[tokio::test]
async fn running_backup_without_rcon_returns_conflict() {
    let app = TestApp::spawn_with_mock().await;
    let instance_id = create_test_instance(&app).await;
    let start = app
        .client
        .post(app.url(&format!("/instances/{instance_id}/start")))
        .send()
        .await
        .unwrap();
    assert_eq!(start.status(), 200);

    let res = app
        .client
        .post(app.url(&format!("/instances/{instance_id}/backups")))
        .json(&json!({ "name": "hot" }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 409);
    let body = res.json::<serde_json::Value>().await.unwrap();
    assert_eq!(body["error"], "rcon_required_for_hot_backup");
}

#[tokio::test]
async fn backup_schedule_writes_scheduled_task() {
    let app = TestApp::spawn().await;
    let instance_id = create_test_instance(&app).await;

    let res = app
        .client
        .put(app.url(&format!("/instances/{instance_id}/backups/schedule")))
        .json(&json!({
            "enabled": true,
            "cron": "0 4 * * *",
            "retain_count": 3
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    let task_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM scheduled_tasks WHERE instance_id = ? AND task_type = 'backup' AND enabled = 1",
    )
    .bind(&instance_id)
    .fetch_one(&app.state.pool)
    .await
    .unwrap();
    assert_eq!(task_count, 1);
}

#[tokio::test]
async fn backup_schedule_rejects_invalid_retention() {
    let app = TestApp::spawn().await;
    let instance_id = create_test_instance(&app).await;

    let res = app
        .client
        .put(app.url(&format!("/instances/{instance_id}/backups/schedule")))
        .json(&json!({
            "enabled": true,
            "cron": "0 4 * * *",
            "retain_count": 0
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

fn write_zip(dir: &std::path::Path, id: &str, files: &[(&str, &[u8])]) {
    std::fs::create_dir_all(dir).unwrap();
    let file = File::create(dir.join(format!("{id}.zip"))).unwrap();
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    for (name, bytes) in files {
        zip.start_file(name, options).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

async fn insert_backup(app: &TestApp, instance_id: &str, backup_id: &str) {
    sqlx::query(
        "INSERT INTO backups (id, instance_id, name, size_bytes, locked, trigger, hot, consistency, created_at) VALUES (?, ?, 'test', 1, 0, 'manual', 0, 'offline', ?)",
    )
    .bind(backup_id)
    .bind(instance_id)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&app.state.pool)
    .await
    .unwrap();
}
