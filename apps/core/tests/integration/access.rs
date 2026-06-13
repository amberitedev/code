use crate::common::{create_test_instance, TestApp};
use serde_json::json;

#[tokio::test]
async fn owner_can_grant_core_access_and_activity_is_recorded() {
    let app = TestApp::spawn().await;

    let grant = app
        .client
        .post(app.url("/core/access"))
        .json(&json!({
            "user_id": "dev-two",
            "display_name": "Dev Two",
            "role": "admin",
            "permission_preset": "admin"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(grant.status(), 200);

    let access = app
        .client
        .get(app.url("/core/access"))
        .header("Authorization", "Bearer dev:dev-two")
        .send()
        .await
        .unwrap();
    assert_eq!(access.status(), 200);
    let body = access.json::<serde_json::Value>().await.unwrap();
    assert_eq!(body["viewer"]["user_id"], "dev-two");
    assert_eq!(body["viewer"]["can_manage_users"], true);

    let activity = app.client.get(app.url("/activity")).send().await.unwrap();
    assert_eq!(activity.status(), 200);
    let body = activity.json::<serde_json::Value>().await.unwrap();
    assert_eq!(body["entries"][0]["action"], "user_access_granted");
    assert_eq!(body["entries"][0]["target_user_id"], "dev-two");
}

#[tokio::test]
async fn admin_can_grant_instance_access() {
    let app = TestApp::spawn().await;
    let instance_id = create_test_instance(&app).await;

    app.client
        .post(app.url("/core/access"))
        .json(&json!({
            "user_id": "dev-two",
            "display_name": "Dev Two",
            "role": "admin",
            "permission_preset": "admin"
        }))
        .send()
        .await
        .unwrap();

    let grant = app
        .client
        .post(app.url(&format!("/instances/{instance_id}/access")))
        .header("Authorization", "Bearer dev:dev-two")
        .json(&json!({
            "user_id": "viewer",
            "display_name": "Viewer",
            "role": "member",
            "permission_preset": "viewer"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(grant.status(), 200);

    let access = app
        .client
        .get(app.url(&format!("/instances/{instance_id}/access")))
        .header("Authorization", "Bearer dev:viewer")
        .send()
        .await
        .unwrap();
    assert_eq!(access.status(), 200);
    let body = access.json::<serde_json::Value>().await.unwrap();
    assert_eq!(body["viewer"]["permission_preset"], "viewer");
    assert_eq!(body["viewer"]["can_manage_users"], false);
}

#[tokio::test]
async fn outsider_cannot_read_core_access() {
    let app = TestApp::spawn().await;

    let res = app
        .client
        .get(app.url("/core/access"))
        .header("Authorization", "Bearer dev:outsider")
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn instance_activity_filters_to_instance() {
    let app = TestApp::spawn().await;
    let first_id = create_test_instance(&app).await;
    let second_id = create_test_instance(&app).await;

    let res = app
        .client
        .get(app.url(&format!("/instances/{first_id}/activity")))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body = res.json::<serde_json::Value>().await.unwrap();
    let entries = body["entries"].as_array().unwrap();
    assert!(entries.iter().any(|entry| entry["instance_id"] == first_id));
    assert!(entries
        .iter()
        .all(|entry| entry["instance_id"] != second_id));
}

#[tokio::test]
async fn viewer_cannot_read_instance_logs() {
    let app = TestApp::spawn().await;
    let instance_id = create_test_instance(&app).await;

    let grant = app
        .client
        .post(app.url(&format!("/instances/{instance_id}/access")))
        .json(&json!({
            "user_id": "viewer",
            "display_name": "Viewer",
            "role": "member",
            "permission_preset": "viewer"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(grant.status(), 200);

    let res = app
        .client
        .get(app.url(&format!("/instances/{instance_id}/logs")))
        .header("Authorization", "Bearer dev:viewer")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn activity_hides_instance_events_without_instance_view_permission() {
    let app = TestApp::spawn().await;
    let instance_id = create_test_instance(&app).await;

    let grant = app
        .client
        .post(app.url("/core/access"))
        .json(&json!({
            "user_id": "client-only",
            "display_name": "Client Only",
            "role": "member",
            "permission_preset": "client-only"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(grant.status(), 200);

    let res = app
        .client
        .get(app.url("/activity"))
        .header("Authorization", "Bearer dev:client-only")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body = res.json::<serde_json::Value>().await.unwrap();
    let entries = body["entries"].as_array().unwrap();
    assert!(entries
        .iter()
        .all(|entry| entry["instance_id"] != instance_id));
}
