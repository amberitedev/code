use std::{io::Write, path::PathBuf};

use reqwest::{multipart, Response};
use serde_json::{json, Value};
use zip::write::SimpleFileOptions;

use crate::common::{create_test_instance, TestApp};

#[tokio::test]
async fn snapshot_apply_materializes_overrides_and_preserves_local_mods() {
    let app = TestApp::spawn().await;
    let instance_id = create_test_instance(&app).await;
    let profile_id = register_sync_profile(&app, &instance_id).await;
    let data_dir = data_dir_for(&app, &instance_id).await;
    let mods_dir = data_dir.join("mods");
    tokio::fs::create_dir_all(&mods_dir).await.unwrap();
    tokio::fs::write(mods_dir.join("local.jar"), b"local")
        .await
        .unwrap();

    let archive = mrpack(
        "v1",
        vec![client_only_file("mods/client-only.jar")],
        vec![
            json!({ "path": "mods/disabled.jar", "server": "disabled" }),
            json!({ "path": "mods/removed.jar", "server": "removed" }),
            json!({ "projectId": "missing-project", "server": "disabled" }),
        ],
        vec![
            ("enabled.jar", b"enabled-v1".as_slice()),
            ("disabled.jar", b"disabled-v1".as_slice()),
            ("removed.jar", b"removed-v1".as_slice()),
        ],
    );

    let result = publish_snapshot(&app, &profile_id, archive.clone()).await;

    assert_eq!(
        tokio::fs::read(mods_dir.join("enabled.jar")).await.unwrap(),
        b"enabled-v1"
    );
    assert!(!mods_dir.join("disabled.jar").exists());
    assert_eq!(
        tokio::fs::read(mods_dir.join("disabled.jar.disabled"))
            .await
            .unwrap(),
        b"disabled-v1"
    );
    assert!(!mods_dir.join("removed.jar").exists());
    assert!(!mods_dir.join("removed.jar.disabled").exists());
    assert!(!mods_dir.join("client-only.jar").exists());
    assert!(mods_dir.join("local.jar").exists());

    let listed_mods = response_json(
        app.client
            .get(app.url(&format!("/instances/{instance_id}/mods")))
            .send()
            .await
            .unwrap(),
    )
    .await;
    let listed_files = listed_mods["mods"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["filename"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        listed_files
            .iter()
            .filter(|filename| **filename == "disabled.jar")
            .count(),
        1
    );
    assert!(!listed_files
        .iter()
        .any(|filename| *filename == "disabled.jar.disabled"));

    let diff: Value = serde_json::from_str(
        result["event"]["diff_json"].as_str().expect("diff_json"),
    )
    .unwrap();
    assert_eq!(diff["invalidOverrides"][0]["reason"], "no matching file");

    let snapshots = response_json(
        app.client
            .get(app.url(&format!("/sync/profiles/{profile_id}/snapshots")))
            .send()
            .await
            .unwrap(),
    )
    .await;
    let snapshot_id = snapshots["snapshots"][0]["id"].as_str().unwrap();
    let downloaded = app
        .client
        .get(app.url(&format!(
            "/sync/profiles/{profile_id}/snapshots/{snapshot_id}/download"
        )))
        .send()
        .await
        .unwrap();
    assert!(downloaded.status().is_success());
    assert_eq!(
        downloaded.bytes().await.unwrap().as_ref(),
        archive.as_slice()
    );
}

#[tokio::test]
async fn snapshot_apply_updates_disabled_and_removes_previous_managed() {
    let app = TestApp::spawn().await;
    let instance_id = create_test_instance(&app).await;
    let profile_id = register_sync_profile(&app, &instance_id).await;
    let data_dir = data_dir_for(&app, &instance_id).await;
    let mods_dir = data_dir.join("mods");
    tokio::fs::create_dir_all(&mods_dir).await.unwrap();
    tokio::fs::write(mods_dir.join("local.jar"), b"local")
        .await
        .unwrap();

    publish_snapshot(
        &app,
        &profile_id,
        mrpack(
            "v1",
            vec![],
            vec![json!({ "path": "mods/disabled.jar", "server": "disabled" })],
            vec![
                ("old.jar", b"old-v1".as_slice()),
                ("disabled.jar", b"disabled-v1".as_slice()),
            ],
        ),
    )
    .await;
    publish_snapshot(
        &app,
        &profile_id,
        mrpack(
            "v2",
            vec![],
            vec![json!({ "path": "mods/disabled.jar", "server": "disabled" })],
            vec![
                ("new.jar", b"new-v2".as_slice()),
                ("disabled.jar", b"disabled-v2".as_slice()),
            ],
        ),
    )
    .await;

    assert!(!mods_dir.join("old.jar").exists());
    assert!(!mods_dir.join("old.jar.disabled").exists());
    assert_eq!(
        tokio::fs::read(mods_dir.join("new.jar")).await.unwrap(),
        b"new-v2"
    );
    assert!(!mods_dir.join("disabled.jar").exists());
    assert_eq!(
        tokio::fs::read(mods_dir.join("disabled.jar.disabled"))
            .await
            .unwrap(),
        b"disabled-v2"
    );
    assert!(mods_dir.join("local.jar").exists());

    publish_snapshot(
        &app,
        &profile_id,
        mrpack(
            "v3",
            vec![],
            vec![json!({ "path": "mods/disabled.jar", "server": "removed" })],
            vec![
                ("new.jar", b"new-v2".as_slice()),
                ("disabled.jar", b"disabled-v3".as_slice()),
            ],
        ),
    )
    .await;

    assert!(!mods_dir.join("disabled.jar").exists());
    assert!(!mods_dir.join("disabled.jar.disabled").exists());
    assert!(mods_dir.join("new.jar").exists());
    assert!(mods_dir.join("local.jar").exists());
}

async fn register_sync_profile(app: &TestApp, instance_id: &str) -> String {
    let body = json!({
        "name": "Synced Pack",
        "core_instance_id": instance_id,
        "game_version": "1.21.1",
        "loader": "vanilla",
        "sync_enabled": true
    });
    let value = response_json(
        app.client
            .post(app.url("/sync/profiles"))
            .json(&body)
            .send()
            .await
            .unwrap(),
    )
    .await;
    value["id"].as_str().unwrap().to_string()
}

async fn publish_snapshot(
    app: &TestApp,
    profile_id: &str,
    archive: Vec<u8>,
) -> Value {
    let form = multipart::Form::new().part(
        "mrpack",
        multipart::Part::bytes(archive)
            .file_name("snapshot.mrpack")
            .mime_str("application/zip")
            .unwrap(),
    );
    response_json(
        app.client
            .post(app.url(&format!("/sync/profiles/{profile_id}/snapshots")))
            .multipart(form)
            .send()
            .await
            .unwrap(),
    )
    .await
}

async fn data_dir_for(app: &TestApp, instance_id: &str) -> PathBuf {
    let data_dir: String =
        sqlx::query_scalar("SELECT data_dir FROM instances WHERE id = ?")
            .bind(instance_id)
            .fetch_one(&app.state.pool)
            .await
            .unwrap();
    PathBuf::from(data_dir)
}

async fn response_json(response: Response) -> Value {
    let status = response.status();
    let text = response.text().await.unwrap();
    assert!(status.is_success(), "{status}: {text}");
    serde_json::from_str(&text).unwrap()
}

fn mrpack(
    version_id: &str,
    files: Vec<Value>,
    overrides: Vec<Value>,
    archive_mods: Vec<(&str, &[u8])>,
) -> Vec<u8> {
    let index = json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": version_id,
        "name": "Synced Pack",
        "files": files,
        "dependencies": { "minecraft": "1.21.1" },
        "overrides": overrides
    });
    let cursor = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cursor);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("modrinth.index.json", options).unwrap();
    zip.write_all(&serde_json::to_vec(&index).unwrap()).unwrap();
    for (filename, data) in archive_mods {
        zip.start_file(format!("overrides/mods/{filename}"), options)
            .unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn client_only_file(path: &str) -> Value {
    json!({
        "path": path,
        "hashes": {},
        "env": { "client": "required", "server": "unsupported" },
        "downloads": [],
        "fileSize": 0
    })
}
