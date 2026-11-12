//! How the backend's statuses reach callers: a revoked token is not a new share, and deleting a
//! share that is already gone succeeds. Each test answers requests from a local socket.
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use contracts::ErrorCode;
use contracts::backend::sharing::VersionContent;
use contracts::client::{CallError, Remote};
use sharing_client::pool::StoreClient;
use sharing_client::push::{self, KeptConfigs, LocateConfig, PushError};
use sharing_client::rules::ShareScope;
use sharing_client::shares::{Unavailable, delete_share};

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

/// The backend at `base`, called with a token.
fn remote<'a>(http: &'a reqwest::Client, base: &'a str) -> Remote<'a> {
    Remote {
        http,
        base,
        bearer: Some("token"),
        read_ticket: None,
        timeout: None,
    }
}

fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .use_preconfigured_tls(
            sharing_client::net::tls::pinned_config([0; 32]).unwrap(),
        )
        .build()
        .unwrap()
}

async fn push_against(
    answers: Vec<(u16, String)>,
) -> (Result<i32, PushError>, Vec<String>) {
    let (url, seen) = serve(answers);
    let http = http();
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
        &remote(&http, &url),
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
    (result.map(|pushed| pushed.version), seen)
}

#[tokio::test]
async fn revoked_token_fails_the_push() {
    let (result, seen) = push_against(vec![(
        401,
        r#"{"error":"not_member","description":"x"}"#.into(),
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
        r#"{{"modrinth_ids":[],"modpack_id":null,"removed_files":[],"game_version":"1.21.1","loader":"fabric","loader_version":"","version_hash":"{root}","external_files":[],"version":1,"ready":true}}"#
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
async fn the_pool_refuses_a_version_by_its_code() {
    let (result, _) = push_against(vec![
        (404, r#"{"error":"not_found","description":"x"}"#.into()),
        (503, r#"{"error":"no_storage","description":"x"}"#.into()),
    ])
    .await;
    assert!(matches!(result, Err(PushError::NoStorage)), "{result:?}");
}

#[tokio::test]
async fn deleting_a_gone_share_succeeds() {
    let http = http();
    for status in [404, 410] {
        let (url, _) = serve(vec![(status, "{}".into())]);
        let result = delete_share(&remote(&http, &url), "share").await;
        assert!(result.is_ok(), "{status}");
    }
    let (url, _) = serve(vec![(
        403,
        r#"{"error":"not_owner","description":"n"}"#.into(),
    )]);
    let error = delete_share(&remote(&http, &url), "share")
        .await
        .unwrap_err();
    assert!(
        matches!(
            &error,
            CallError::Api { status: 403, error } if error.error == ErrorCode::NotOwner
        ),
        "{error:?}"
    );
}
