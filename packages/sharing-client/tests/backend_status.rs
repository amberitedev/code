//! How the backend's statuses reach callers: a revoked token is not a new share, and deleting a
//! share that is already gone succeeds. Each test answers requests from a local socket.
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use sharing_client::backend::{Backend, Unavailable};
use sharing_client::pool::StoreClient;
use sharing_client::push::{self, KeptConfigs, LocateConfig, PushError};
use sharing_client::rules::ShareScope;
use sharing_client::versions::VersionContent;

/// Answers each request with the next (status, body) and records each request line.
fn serve(answers: Vec<(u16, String)>) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    std::thread::spawn(move || {
        for (status, body) in answers {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = vec![0; 16384];
            let read = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..read]);
            log.lock()
                .unwrap()
                .push(request.lines().next().unwrap_or_default().to_owned());
            let response = format!(
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    (url, seen)
}

fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .use_preconfigured_tls(
            bedrock::net::tls::pinned_config([0; 32]).unwrap(),
        )
        .build()
        .unwrap()
}

async fn push_against(
    answers: Vec<(u16, String)>,
) -> (Result<i32, PushError>, Vec<String>) {
    let (url, seen) = serve(answers);
    let backend = Backend::new(http(), url, "token");
    let locate: &LocateConfig = &|_| Box::pin(async { Ok(None) });
    let kept = KeptConfigs {
        scope: ShareScope::Server,
        removed: &[],
        locate,
    };
    let scratch = tempfile::tempdir().unwrap();
    let content = VersionContent {
        modrinth_ids: Vec::new(),
        modpack_id: None,
        removed_files: Vec::new(),
        game_version: "1.21.1".into(),
        loader: "fabric".into(),
        loader_version: String::new(),
    };
    let result = push::push(
        &backend,
        &StoreClient::new("token"),
        "share",
        &content,
        None,
        Vec::new(),
        kept,
        scratch.path(),
        &mut |_| {},
    )
    .await;
    let seen = seen.lock().unwrap().clone();
    (result, seen)
}

#[tokio::test]
async fn revoked_token_fails_the_push() {
    let (result, seen) = push_against(vec![(
        401,
        r#"{"error":"unauthorized","description":"x"}"#.into(),
    )])
    .await;
    assert!(
        matches!(
            result,
            Err(PushError::Unavailable(Unavailable::AccessRevoked))
        ),
        "{result:?}"
    );
    assert_eq!(seen.len(), 1, "no version is created after a 401");
}

#[tokio::test]
async fn share_without_versions_gets_its_first() {
    let root =
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let created = format!(
        r#"{{"modrinth_ids":[],"modpack_id":null,"removed_files":[],"game_version":"1.21.1","loader":"fabric","loader_version":"","root":"{root}","external_files":[],"version":1,"ready":true}}"#
    );
    let (result, seen) = push_against(vec![
        (404, r#"{"error":"not_found","description":"x"}"#.into()),
        (200, created),
    ])
    .await;
    assert_eq!(result.unwrap(), 1);
    assert!(seen[1].starts_with("POST /v1/instances/share/versions"));
}

#[tokio::test]
async fn deleting_a_gone_share_succeeds() {
    for status in [404, 410] {
        let (url, _) = serve(vec![(status, "{}".into())]);
        let backend = Backend::new(http(), url, "token");
        assert!(backend.delete_share("share").await.is_ok(), "{status}");
    }
    let (url, _) = serve(vec![(
        403,
        r#"{"error":"forbidden","code":"not_owner","description":"n"}"#.into(),
    )]);
    let error = Backend::new(http(), url, "token")
        .delete_share("share")
        .await
        .unwrap_err();
    assert_eq!(error.status(), Some(reqwest::StatusCode::FORBIDDEN));
    assert_eq!(error.body().unwrap().code(), "not_owner");
}
