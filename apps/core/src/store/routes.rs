//! The store's routes. `/store/...` take a pool ticket and are what Core forwards; `/admin/...`
//! take the admin token and only Core calls them. Shapes: `docs/internals/pool.md`, Protocol.

use std::{
    io::{self, SeekFrom},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use axum::{
    Json, Router,
    body::Body,
    extract::{ConnectInfo, Path, State},
    http::{
        Extensions, HeaderMap, StatusCode,
        header::{
            ACCEPT_RANGES, AUTHORIZATION, CONTENT_LENGTH, CONTENT_RANGE,
            CONTENT_TYPE, RANGE,
        },
    },
    response::{IntoResponse, Response},
    routing::{get, post},
};
use bytes::Bytes;
use futures::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::{ReaderStream, StreamReader};

use super::{
    AdminStatus, Caps, Code, Drop, Store, StoreError, now,
    objects::{self, Hash, VersionList, WriteError},
    pull, tar,
    tickets::{self, Claims},
};

/// The most `version.json` may weigh: 4,096 objects take well under a megabyte.
const MAX_LIST: u64 = 4 * 1024 * 1024;

pub fn router(store: Arc<Store>) -> Router {
    Router::new()
        .route("/store/versions/{root}", get(list).post(upload))
        .route("/store/versions/{root}/have", post(have))
        .route("/store/versions/{root}/objects/{object}", get(object))
        .route("/store/versions/{root}/pull", post(pull_version))
        .route("/admin/status", get(status))
        .route("/admin/drop", post(drop_versions))
        .route("/admin/config", post(config))
        .with_state(store)
}

fn invalid(description: impl Into<String>) -> StoreError {
    StoreError::new(Code::InvalidRequest, description)
}

fn not_found(description: &str) -> StoreError {
    StoreError::new(Code::NotFound, description)
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

fn root_of(text: &str) -> Result<Hash, StoreError> {
    Hash::parse(text).ok_or_else(|| not_found("There is no such version."))
}

impl Store {
    async fn ticket(
        &self,
        headers: &HeaderMap,
        kind: &'static str,
        root: &Hash,
    ) -> Result<Claims, StoreError> {
        let ticket = bearer(headers).ok_or_else(|| {
            StoreError::new(Code::TicketInvalid, "This needs a ticket.")
        })?;
        tickets::verify(&self.keys, ticket, &self.core_id, kind, root, now())
            .await
    }

    /// Count a request against its address's rate. Core forwards with `X-Forwarded-For` set
    /// to its caller; anything else reaching this port is local.
    fn rate(
        &self,
        headers: &HeaderMap,
        extensions: &Extensions,
    ) -> Result<(), StoreError> {
        let address = headers
            .get("x-forwarded-for")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(',').next())
            .and_then(|value| value.trim().parse::<IpAddr>().ok())
            .or_else(|| {
                extensions
                    .get::<ConnectInfo<SocketAddr>>()
                    .map(|info| info.0.ip())
            })
            .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
        if self.limits.allow(address) {
            Ok(())
        } else {
            Err(StoreError::new(
                Code::RateLimited,
                "Too many requests from this address. Wait a minute.",
            ))
        }
    }

    fn admin(&self, headers: &HeaderMap) -> Result<(), StoreError> {
        // Compared as hashes, so the time taken says nothing about the token.
        let given = Sha256::digest(bearer(headers).unwrap_or_default());
        if given == Sha256::digest(&self.admin_token) {
            Ok(())
        } else {
            Err(StoreError::new(
                Code::TicketInvalid,
                "That is not the admin token.",
            ))
        }
    }
}

#[derive(Debug, Deserialize)]
struct Have {
    objects: Vec<Hash>,
}

#[derive(Debug, Serialize)]
struct HaveAnswer {
    missing: Vec<Hash>,
    complete: bool,
}

/// `POST /store/versions/{root}/have`: which of these objects the store lacks.
async fn have(
    State(store): State<Arc<Store>>,
    Path(root): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<HaveAnswer>, StoreError> {
    let root = root_of(&root)?;
    store.ticket(&headers, tickets::STORE, &root).await?;
    let Have { objects } = serde_json::from_slice(&body)
        .map_err(|_| invalid("Send { objects: [hash...] }."))?;
    let mut missing = Vec::new();
    for hash in objects {
        if !tokio::fs::try_exists(store.layout.object(&hash)).await? {
            missing.push(hash);
        }
    }
    Ok(Json(HaveAnswer {
        missing,
        complete: store.is_stored(&root),
    }))
}

#[derive(Debug, Serialize)]
struct Uploaded {
    complete: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    missing: Vec<Hash>,
}

fn bad_tar(error: io::Error) -> StoreError {
    if error.kind() == io::ErrorKind::InvalidData {
        invalid(error.to_string())
    } else {
        invalid(format!("The upload is not a whole tar ({error})."))
    }
}

/// `POST /store/versions/{root}`: a tar of `version.json` and then objects. What arrives is
/// kept even when the upload breaks off, so a retry sends only the rest.
async fn upload(
    State(store): State<Arc<Store>>,
    Path(root): Path<String>,
    headers: HeaderMap,
    body: Body,
) -> Result<Json<Uploaded>, StoreError> {
    let root = root_of(&root)?;
    let claims = store.ticket(&headers, tickets::STORE, &root).await?;
    if store.is_stored(&root) {
        return Ok(Json(Uploaded {
            complete: true,
            missing: Vec::new(),
        }));
    }
    let mut tar = tar::Reader::new(StreamReader::new(
        body.into_data_stream().map_err(io::Error::other),
    ));
    let first = tar.next().await.map_err(bad_tar)?;
    let first = first
        .filter(|entry| entry.name == "version.json")
        .ok_or_else(|| invalid("The upload must start with version.json."))?;
    if first.size > MAX_LIST {
        return Err(invalid("version.json is too large."));
    }
    let mut json = Vec::new();
    tar.body().read_to_end(&mut json).await.map_err(bad_tar)?;
    let list: VersionList = serde_json::from_slice(&json)
        .map_err(|_| invalid("version.json is unreadable."))?;
    store.begin(&root, list.clone(), claims.bytes).await?;
    while let Some(entry) = tar.next().await.map_err(bad_tar)? {
        let hash = Hash::parse(&entry.name).ok_or_else(|| {
            invalid("Every entry after version.json is named by its hash.")
        })?;
        let size = list.size_of(&hash).ok_or_else(|| {
            invalid("The upload holds an object its version does not list.")
        })?;
        if entry.size != size {
            return Err(invalid(
                "An object is not the size its version lists.",
            ));
        }
        if tokio::fs::try_exists(store.layout.object(&hash)).await? {
            continue;
        }
        objects::write_object(&store.layout, &hash, size, tar.body())
            .await
            .map_err(|error| match error {
                WriteError::Mismatch(description) => invalid(description),
                WriteError::Io(error) => error.into(),
            })?;
    }
    let missing = store.finish(&root).await?;
    Ok(Json(Uploaded {
        complete: missing.is_empty(),
        missing,
    }))
}

/// `GET /store/versions/{root}`: the version's list.
async fn list(
    State(store): State<Arc<Store>>,
    Path(root): Path<String>,
    headers: HeaderMap,
    extensions: Extensions,
) -> Result<Json<VersionList>, StoreError> {
    store.rate(&headers, &extensions)?;
    let root = root_of(&root)?;
    store.ticket(&headers, tickets::FETCH, &root).await?;
    let list = store
        .index()
        .servable(&root)
        .ok_or_else(|| not_found("This Core does not hold that version."))?;
    Ok(Json(list.as_ref().clone()))
}

/// `bytes=<from>-`. Any other range is answered whole, as HTTP allows.
fn range_start(headers: &HeaderMap) -> Option<u64> {
    let value = headers.get(RANGE)?.to_str().ok()?;
    value
        .strip_prefix("bytes=")?
        .strip_suffix('-')?
        .parse()
        .ok()
}

/// `GET /store/versions/{root}/objects/{object}`: one object's bytes, counted against the day's
/// egress as they go.
async fn object(
    State(store): State<Arc<Store>>,
    Path((root, object)): Path<(String, String)>,
    headers: HeaderMap,
    extensions: Extensions,
) -> Result<Response, StoreError> {
    store.rate(&headers, &extensions)?;
    let slot = store.limits.fetch_slot().ok_or_else(|| {
        StoreError::new(
            Code::RateLimited,
            "This Core is serving as many downloads as it can. Try again shortly.",
        )
    })?;
    let root = root_of(&root)?;
    store.ticket(&headers, tickets::FETCH, &root).await?;
    let not_held = || not_found("That object is not in this version.");
    let object = Hash::parse(&object).ok_or_else(not_held)?;
    let list = store
        .index()
        .servable(&root)
        .ok_or_else(|| not_found("This Core does not hold that version."))?;
    list.size_of(&object).ok_or_else(not_held)?;
    if store.limits.egress_today() >= store.caps().egress_cap {
        return Err(StoreError::new(
            Code::EgressSpent,
            "This Core has sent all it may today.",
        ));
    }
    let mut file =
        match tokio::fs::File::open(store.layout.object(&object)).await {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(not_held());
            }
            file => file?,
        };
    let len = file.metadata().await?.len();
    let from = range_start(&headers);
    if from.is_some_and(|from| from >= len) {
        let mut refused =
            invalid("The range starts past the object's end.").into_response();
        *refused.status_mut() = StatusCode::RANGE_NOT_SATISFIABLE;
        refused.headers_mut().insert(
            CONTENT_RANGE,
            format!("bytes */{len}").parse().expect("a valid header"),
        );
        return Ok(refused);
    }
    let start = from.unwrap_or(0);
    file.seek(SeekFrom::Start(start)).await?;
    let counter = Arc::clone(&store);
    let stream =
        ReaderStream::new(file.take(len - start)).inspect(move |chunk| {
            // The slot is held for as long as the body streams.
            let _ = &slot;
            if let Ok(bytes) = chunk {
                counter.limits.spend(bytes.len() as u64);
            }
        });
    let mut response = Response::builder()
        .header(CONTENT_TYPE, "application/octet-stream")
        .header(CONTENT_LENGTH, len - start)
        .header(ACCEPT_RANGES, "bytes");
    if from.is_some() {
        response = response
            .status(StatusCode::PARTIAL_CONTENT)
            .header(CONTENT_RANGE, format!("bytes {start}-{}/{len}", len - 1));
    }
    Ok(response
        .body(Body::from_stream(stream))
        .expect("a valid response"))
}

#[derive(Debug, Serialize)]
struct Pulled {
    complete: bool,
}

/// `POST /store/versions/{root}/pull`: copy the version from the Core the ticket names.
async fn pull_version(
    State(store): State<Arc<Store>>,
    Path(root): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Pulled>, StoreError> {
    let root = root_of(&root)?;
    let claims = store.ticket(&headers, tickets::PULL, &root).await?;
    if store.is_stored(&root) {
        return Ok(Json(Pulled { complete: true }));
    }
    let source = claims.source.expect("verify requires it on pull tickets");
    let complete = pull::pull(&store, &root, &source).await?;
    Ok(Json(Pulled { complete }))
}

/// `GET /admin/status`.
async fn status(
    State(store): State<Arc<Store>>,
    headers: HeaderMap,
) -> Result<Json<AdminStatus>, StoreError> {
    store.admin(&headers)?;
    Ok(Json(store.status()?))
}

/// `POST /admin/drop`.
async fn drop_versions(
    State(store): State<Arc<Store>>,
    headers: HeaderMap,
    Json(Drop { roots }): Json<Drop>,
) -> Result<StatusCode, StoreError> {
    store.admin(&headers)?;
    let roots: Vec<_> =
        roots.iter().filter_map(|root| Hash::parse(root)).collect();
    store.drop_versions(&roots).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /admin/config`.
async fn config(
    State(store): State<Arc<Store>>,
    headers: HeaderMap,
    Json(caps): Json<Caps>,
) -> Result<StatusCode, StoreError> {
    store.admin(&headers)?;
    *store.caps.lock().expect("not poisoned") = caps;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
pub mod tests {
    use axum::http::{Method, Request};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::*;
    use crate::{
        access::{keys::Keys, tickets::test_support::Signer},
        store::{
            objects::{GRACE_S, Layout, list_of},
            tar::test_support::tar,
            tickets::test_support::ticket,
        },
    };

    pub const ADMIN: &str = "an admin token of at least thirty-two characters";

    pub struct Fixture {
        pub core: String,
        pub dir: tempfile::TempDir,
        pub store: Arc<Store>,
        pub router: Router,
        pub signer: Signer,
    }

    /// A store for Core `core` with these caps, trusting a fresh signer.
    pub async fn fixture(core: &str, cap: u64, egress_cap: u64) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let signer = Signer::new("k1");
        let keys = Keys::load_file(
            dir.path().join("keys.json"),
            reqwest::Client::new(),
            None,
        );
        keys.set(&signer.public_keys()).await;
        let store = Arc::new(
            Store::open(
                Layout::new(dir.path()),
                core.into(),
                keys,
                ADMIN.into(),
                true,
                Caps { cap, egress_cap },
            )
            .unwrap(),
        );
        Fixture {
            core: core.into(),
            router: router(Arc::clone(&store)),
            dir,
            store,
            signer,
        }
    }

    impl Fixture {
        pub fn ticket(&self, kind: &str, root: &Hash, extra: Value) -> String {
            ticket(
                &self.signer,
                kind,
                json!([self.core]),
                root.as_str(),
                now(),
                extra,
            )
        }

        pub async fn call(
            &self,
            method: Method,
            uri: &str,
            token: &str,
            headers: &[(&str, &str)],
            body: Body,
        ) -> (StatusCode, HeaderMap, Vec<u8>) {
            let mut request = Request::builder()
                .method(method)
                .uri(uri)
                .header("authorization", format!("Bearer {token}"));
            for (name, value) in headers {
                request = request.header(*name, *value);
            }
            let response = self
                .router
                .clone()
                .oneshot(request.body(body).unwrap())
                .await
                .unwrap();
            let status = response.status();
            let headers = response.headers().clone();
            let bytes =
                response.into_body().collect().await.unwrap().to_bytes();
            (status, headers, bytes.to_vec())
        }

        pub async fn json(
            &self,
            method: Method,
            uri: &str,
            token: &str,
            body: Body,
        ) -> (StatusCode, Value) {
            let (status, _, bytes) = self
                .call(
                    method,
                    uri,
                    token,
                    &[("content-type", "application/json")],
                    body,
                )
                .await;
            (
                status,
                serde_json::from_slice(&bytes).unwrap_or(Value::Null),
            )
        }

        /// Upload `objects` of `list` as `root` with a ticket for `bytes`.
        pub async fn upload(
            &self,
            list: &VersionList,
            objects: &[&[u8]],
            bytes: u64,
        ) -> (StatusCode, Value) {
            let root = list.root();
            let json = serde_json::to_vec(list).unwrap();
            let names: Vec<String> = objects
                .iter()
                .map(|data| Hash::of_digest(&Sha256::digest(data)).to_string())
                .collect();
            let mut entries: Vec<(&str, &[u8])> = vec![("version.json", &json)];
            entries.extend(
                names
                    .iter()
                    .map(String::as_str)
                    .zip(objects.iter().copied()),
            );
            let ticket = self.ticket("store", &root, json!({ "bytes": bytes }));
            self.json(
                Method::POST,
                &format!("/store/versions/{root}"),
                &ticket,
                Body::from(tar(&entries, false)),
            )
            .await
        }
    }

    #[tokio::test]
    async fn an_upload_resumes_and_completes() {
        let f = fixture("core", 1 << 30, 1 << 30).await;
        let list = list_of(&[b"one", b"two", b"three"]);
        let root = list.root();
        let store_ticket = f.ticket("store", &root, json!({ "bytes": 100 }));
        let all: Vec<_> =
            list.objects.iter().map(|e| e.hash.to_string()).collect();
        let (status, have) = f
            .json(
                Method::POST,
                &format!("/store/versions/{root}/have"),
                &store_ticket,
                Body::from(json!({ "objects": all }).to_string()),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{have}");
        assert_eq!(have["complete"], false);
        assert_eq!(have["missing"].as_array().unwrap().len(), 3);

        let (status, first) = f.upload(&list, &[b"one"], 100).await;
        assert_eq!(status, StatusCode::OK, "{first}");
        assert_eq!(first["complete"], false);
        assert_eq!(first["missing"].as_array().unwrap().len(), 2);
        // The retry sends the rest, and something already there.
        let (status, second) =
            f.upload(&list, &[b"one", b"two", b"three"], 100).await;
        assert_eq!(status, StatusCode::OK, "{second}");
        assert_eq!(second, json!({ "complete": true }));
        assert!(f.dir.path().join(format!("versions/{root}.json")).exists());

        let fetch = f.ticket("fetch", &root, json!({}));
        let (status, stored) = f
            .json(
                Method::GET,
                &format!("/store/versions/{root}"),
                &fetch,
                Body::empty(),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(stored, serde_json::to_value(&list).unwrap());
        let (_, have) = f
            .json(
                Method::POST,
                &format!("/store/versions/{root}/have"),
                &store_ticket,
                Body::from(json!({ "objects": all }).to_string()),
            )
            .await;
        assert_eq!(have, json!({ "missing": [], "complete": true }));
        let (_, status) =
            f.admin(Method::GET, "/admin/status", Value::Null).await;
        assert_eq!(status["used"], 11);
        assert_eq!(status["versions"], json!([{ "root": root, "bytes": 11 }]));
    }

    #[tokio::test]
    async fn wrong_bytes_and_wrong_lists_are_refused() {
        let f = fixture("core", 1 << 30, 1 << 30).await;
        let list = list_of(&[b"one"]);
        let root = list.root();
        // An entry whose bytes do not match its name.
        let json = serde_json::to_vec(&list).unwrap();
        let name = list.objects[0].hash.to_string();
        let body = tar(&[("version.json", &json), (&name, b"two")], true);
        let ticket = f.ticket("store", &root, json!({ "bytes": 100 }));
        let uri = format!("/store/versions/{root}");
        let (status, error) =
            f.json(Method::POST, &uri, &ticket, Body::from(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(error["error"], "invalid_request");
        let folder = f.store.layout.object(&list.objects[0].hash);
        assert!(!folder.exists());
        assert_eq!(
            std::fs::read_dir(folder.parent().unwrap()).unwrap().count(),
            0,
            "no part file is left"
        );
        // A list that does not hash to the ticket's root.
        let other = list_of(&[b"two"]);
        let json = serde_json::to_vec(&other).unwrap();
        let body = tar(&[("version.json", &json)], false);
        let (status, _) =
            f.json(Method::POST, &uri, &ticket, Body::from(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // version.json must come first.
        let body = tar(&[(&name, b"one"), ("version.json", &json)], false);
        let (status, _) =
            f.json(Method::POST, &uri, &ticket, Body::from(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // Larger than the ticket's bytes.
        let (status, _) = f.upload(&list, &[b"one"], 2).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn an_upload_over_the_cap_is_disk_full() {
        let f = fixture("core", 6, 1 << 30).await;
        let (status, _) = f.upload(&list_of(&[b"one", b"two"]), &[], 100).await;
        assert_eq!(status, StatusCode::OK);
        let (status, error) = f.upload(&list_of(&[b"three"]), &[], 100).await;
        assert_eq!(status, StatusCode::INSUFFICIENT_STORAGE);
        assert_eq!(error["error"], "disk_full");
    }

    impl Fixture {
        async fn admin(
            &self,
            method: Method,
            uri: &str,
            body: Value,
        ) -> (StatusCode, Value) {
            let body = if body.is_null() {
                Body::empty()
            } else {
                Body::from(body.to_string())
            };
            let (status, _, bytes) = self
                .call(
                    method,
                    uri,
                    ADMIN,
                    &[("content-type", "application/json")],
                    body,
                )
                .await;
            (
                status,
                serde_json::from_slice(&bytes).unwrap_or(Value::Null),
            )
        }
    }

    async fn stored(f: &Fixture, objects: &[&[u8]]) -> VersionList {
        let list = list_of(objects);
        let (status, answer) = f.upload(&list, objects, 1 << 20).await;
        assert_eq!(answer, json!({ "complete": true }), "{status}");
        list
    }

    #[tokio::test]
    async fn fetches_honour_ranges_and_check_their_tickets() {
        let f = fixture("core", 1 << 30, 1 << 30).await;
        let list = stored(&f, &[b"hello world"]).await;
        let root = list.root();
        let object = &list.objects[0].hash;
        let uri = format!("/store/versions/{root}/objects/{object}");
        let fetch = f.ticket("fetch", &root, json!({}));
        let (status, headers, body) =
            f.call(Method::GET, &uri, &fetch, &[], Body::empty()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, b"hello world");
        assert_eq!(headers[CONTENT_LENGTH], "11");
        let (status, headers, body) = f
            .call(
                Method::GET,
                &uri,
                &fetch,
                &[("range", "bytes=6-")],
                Body::empty(),
            )
            .await;
        assert_eq!(status, StatusCode::PARTIAL_CONTENT);
        assert_eq!(body, b"world");
        assert_eq!(headers[CONTENT_RANGE], "bytes 6-10/11");
        let (status, ..) = f
            .call(
                Method::GET,
                &uri,
                &fetch,
                &[("range", "bytes=11-")],
                Body::empty(),
            )
            .await;
        assert_eq!(status, StatusCode::RANGE_NOT_SATISFIABLE);

        let code = |ticket: String| {
            let uri = uri.clone();
            let f = &f;
            async move {
                f.json(Method::GET, &uri, &ticket, Body::empty()).await.1["error"].clone()
            }
        };
        assert_eq!(
            code(f.ticket("store", &root, json!({ "bytes": 1 }))).await,
            "ticket_wrong_kind"
        );
        let old = ticket(
            &f.signer,
            "fetch",
            json!(["core"]),
            root.as_str(),
            now() - 7200,
            json!({}),
        );
        assert_eq!(code(old).await, "ticket_expired");
        let elsewhere = ticket(
            &f.signer,
            "fetch",
            json!(["other"]),
            root.as_str(),
            now(),
            json!({}),
        );
        assert_eq!(code(elsewhere).await, "ticket_wrong_core");
        assert_eq!(code("nonsense".into()).await, "ticket_invalid");
        // An object of another version is not served under this one.
        let other = stored(&f, &[b"other"]).await;
        let uri =
            format!("/store/versions/{root}/objects/{}", other.objects[0].hash);
        let (status, _) =
            f.json(Method::GET, &uri, &fetch, Body::empty()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn the_egress_cap_stops_fetches_for_the_day() {
        let f = fixture("core", 1 << 30, 10).await;
        let list = stored(&f, &[b"hello world"]).await;
        let root = list.root();
        let uri =
            format!("/store/versions/{root}/objects/{}", list.objects[0].hash);
        let fetch = f.ticket("fetch", &root, json!({}));
        let (status, ..) =
            f.call(Method::GET, &uri, &fetch, &[], Body::empty()).await;
        assert_eq!(status, StatusCode::OK);
        let (status, error) =
            f.json(Method::GET, &uri, &fetch, Body::empty()).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(error["error"], "egress_spent");
        let (_, status) =
            f.admin(Method::GET, "/admin/status", Value::Null).await;
        assert_eq!(status["egress_today"], 11);
        f.admin(
            Method::POST,
            "/admin/config",
            json!({ "cap": 1 << 30, "egress_cap": 100 }),
        )
        .await;
        let (status, ..) =
            f.call(Method::GET, &uri, &fetch, &[], Body::empty()).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn the_admin_routes_need_the_token() {
        let f = fixture("core", 1 << 30, 1 << 30).await;
        let (status, error) = f
            .json(Method::GET, "/admin/status", "wrong", Body::empty())
            .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{error}");
        let (status, _) =
            f.admin(Method::GET, "/admin/status", Value::Null).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn a_dropped_version_serves_through_the_grace_then_goes() {
        let f = fixture("core", 1 << 30, 1 << 30).await;
        let kept = stored(&f, &[b"shared", b"kept"]).await;
        let dropped = stored(&f, &[b"shared", b"gone"]).await;
        let root = dropped.root();
        let (status, _) = f
            .admin(Method::POST, "/admin/drop", json!({ "roots": [root] }))
            .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (_, status) =
            f.admin(Method::GET, "/admin/status", Value::Null).await;
        assert_eq!(status["versions"].as_array().unwrap().len(), 1);
        let gone = list_of(&[b"gone"]).objects[0].hash.clone();
        let uri = format!("/store/versions/{root}/objects/{gone}");
        let fetch = f.ticket("fetch", &root, json!({}));
        let (code, ..) =
            f.call(Method::GET, &uri, &fetch, &[], Body::empty()).await;
        assert_eq!(code, StatusCode::OK, "served within the grace");

        let layout = f.store.layout.clone();
        // Within the grace, and as if the objects were written long ago.
        objects::collect_garbage(&layout, &f.store.index, now()).unwrap();
        assert!(layout.object(&gone).exists());
        let old =
            std::time::SystemTime::now() - std::time::Duration::from_secs(7200);
        for entry in kept.objects.iter().chain(&dropped.objects) {
            let file = std::fs::File::options()
                .write(true)
                .open(layout.object(&entry.hash))
                .unwrap();
            file.set_modified(old).unwrap();
        }
        objects::collect_garbage(&layout, &f.store.index, now() + GRACE_S)
            .unwrap();
        assert!(!layout.object(&gone).exists());
        for entry in &kept.objects {
            assert!(
                layout.object(&entry.hash).exists(),
                "a kept list's objects stay"
            );
        }
        let (code, ..) =
            f.call(Method::GET, &uri, &fetch, &[], Body::empty()).await;
        assert_eq!(code, StatusCode::NOT_FOUND);
        let (_, status) =
            f.admin(Method::GET, "/admin/status", Value::Null).await;
        assert_eq!(status["used"], 10);
    }

    #[tokio::test]
    async fn one_address_is_rate_limited() {
        let f = fixture("core", 1 << 30, 1 << 30).await;
        let list = stored(&f, &[b"x"]).await;
        let root = list.root();
        let uri = format!("/store/versions/{root}");
        let fetch = f.ticket("fetch", &root, json!({}));
        for _ in 0..crate::store::limits::REQUESTS_PER_MINUTE {
            let (status, ..) = f
                .call(
                    Method::GET,
                    &uri,
                    &fetch,
                    &[("x-forwarded-for", "203.0.113.9")],
                    Body::empty(),
                )
                .await;
            assert_eq!(status, StatusCode::OK);
        }
        let (status, ..) = f
            .call(
                Method::GET,
                &uri,
                &fetch,
                &[("x-forwarded-for", "203.0.113.9")],
                Body::empty(),
            )
            .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        let (status, ..) = f
            .call(
                Method::GET,
                &uri,
                &fetch,
                &[("x-forwarded-for", "203.0.113.10")],
                Body::empty(),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
    }
}
