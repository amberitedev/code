use crate::common::{create_test_instance, TestApp};

#[tokio::test]
async fn resumable_upload_appends_and_finalizes() {
    let app = TestApp::spawn().await;
    let instance_id = create_test_instance(&app).await;

    let create = app
        .client
        .post(app.url(&format!(
            "/instances/{instance_id}/fs/uploads?path=uploads/test.txt"
        )))
        .header("Upload-Length", "5")
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), 201);
    assert_eq!(create.headers()["upload-offset"], "0");
    let location = create.headers()["location"].to_str().unwrap().to_string();
    let upload_id = location.rsplit('/').next().unwrap();

    let status =
        app.client
            .head(app.url(&format!(
                "/instances/{instance_id}/fs/uploads/{upload_id}"
            )))
            .send()
            .await
            .unwrap();
    assert_eq!(status.status(), 204);
    assert_eq!(status.headers()["upload-offset"], "0");

    let append =
        app.client
            .patch(app.url(&format!(
                "/instances/{instance_id}/fs/uploads/{upload_id}"
            )))
            .header("Upload-Offset", "0")
            .body("hello")
            .send()
            .await
            .unwrap();
    assert_eq!(append.status(), 204);
    assert_eq!(append.headers()["upload-offset"], "5");

    let record = app
        .state
        .instance_store
        .get(&instance_id.parse().unwrap())
        .await
        .unwrap();
    let uploaded = tokio::fs::read_to_string(
        std::path::Path::new(&record.data_dir).join("uploads/test.txt"),
    )
    .await
    .unwrap();
    assert_eq!(uploaded, "hello");
}

#[tokio::test]
async fn resumable_upload_rejects_offset_mismatch() {
    let app = TestApp::spawn().await;
    let instance_id = create_test_instance(&app).await;
    let upload_id = create_upload(&app, &instance_id, "test.txt", 5).await;

    let res =
        app.client
            .patch(app.url(&format!(
                "/instances/{instance_id}/fs/uploads/{upload_id}"
            )))
            .header("Upload-Offset", "1")
            .body("hello")
            .send()
            .await
            .unwrap();

    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn resumable_upload_rejects_checksum_mismatch() {
    let app = TestApp::spawn().await;
    let instance_id = create_test_instance(&app).await;
    let upload_id = create_upload(&app, &instance_id, "test.txt", 5).await;

    let res =
        app.client
            .patch(app.url(&format!(
                "/instances/{instance_id}/fs/uploads/{upload_id}"
            )))
            .header("Upload-Offset", "0")
            .header(
                "Upload-Checksum",
                "sha256 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
            )
            .body("hello")
            .send()
            .await
            .unwrap();

    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn resumable_upload_rejects_invalid_destination_path() {
    let app = TestApp::spawn().await;
    let instance_id = create_test_instance(&app).await;

    let res = app
        .client
        .post(app.url(&format!(
            "/instances/{instance_id}/fs/uploads?path=../evil.txt"
        )))
        .header("Upload-Length", "5")
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 400);
}

async fn create_upload(
    app: &TestApp,
    instance_id: &str,
    path: &str,
    length: u64,
) -> String {
    let create =
        app.client
            .post(app.url(&format!(
                "/instances/{instance_id}/fs/uploads?path={path}"
            )))
            .header("Upload-Length", length.to_string())
            .send()
            .await
            .unwrap();
    assert_eq!(create.status(), 201);
    create.headers()["location"]
        .to_str()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .to_string()
}
