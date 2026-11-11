//! Serving an [`Endpoint`] with axum.

use std::future::Future;

use axum::{
    body::{Body, Bytes},
    extract::{FromRequest, FromRequestParts, Path, Query, Request},
    handler::Handler,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{self, MethodRouter},
};
use serde::{
    Serialize,
    de::{DeserializeOwned, value::UnitDeserializer},
};

use crate::{ApiError, Empty, Endpoint, ErrorCode, Json, Method, Raw, Stream};

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.error.status())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, axum::Json(self)).into_response()
    }
}

/// The Endpoint's path, and `handler` as its method's route.
pub fn route<E, H, T, S>(handler: H) -> (&'static str, MethodRouter<S>)
where
    E: Endpoint,
    H: Handler<T, S>,
    T: 'static,
    S: Clone + Send + Sync + 'static,
{
    let router = match E::METHOD {
        Method::Get => routing::get(handler),
        Method::Post => routing::post(handler),
        Method::Put => routing::put(handler),
        Method::Patch => routing::patch(handler),
        Method::Delete => routing::delete(handler),
    };
    (E::PATH, router)
}

/// A request to `E`, read into its types. Anything that does not fit is `invalid_input`.
pub struct Req<E: Endpoint>
where
    E::Body: FromBody,
{
    pub params: E::Params,
    pub query: E::Query,
    pub body: <E::Body as FromBody>::Inner,
}

impl<E, S> FromRequest<S> for Req<E>
where
    E: Endpoint,
    E::Body: FromBody,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(
        request: Request,
        state: &S,
    ) -> Result<Self, ApiError> {
        let (mut parts, body) = request.into_parts();
        let params = if E::PATH.contains('{') {
            Path::<E::Params>::from_request_parts(&mut parts, state)
                .await
                .map_err(|rejection| invalid(rejection.body_text()))?
                .0
        } else {
            unit::<E::Params>().map_err(|error| invalid(error.to_string()))?
        };
        // A route without a query ignores one, such as a cache-busting `?v=`.
        let query = match Query::<E::Query>::try_from_uri(&parts.uri) {
            Ok(query) => query.0,
            Err(rejection) => {
                unit().map_err(|_| invalid(rejection.body_text()))?
            }
        };
        let body =
            E::Body::from_request(Request::from_parts(parts, body), state)
                .await?;
        Ok(Self {
            params,
            query,
            body,
        })
    }
}

fn unit<T: DeserializeOwned>() -> Result<T, serde::de::value::Error> {
    T::deserialize(UnitDeserializer::new())
}

/// A request body the server reads.
pub trait FromBody {
    type Inner;
    fn from_request<S: Send + Sync>(
        request: Request,
        state: &S,
    ) -> impl Future<Output = Result<Self::Inner, ApiError>> + Send;
}

impl<T: DeserializeOwned> FromBody for Json<T> {
    type Inner = T;

    async fn from_request<S: Send + Sync>(
        request: Request,
        state: &S,
    ) -> Result<T, ApiError> {
        axum::Json::<T>::from_request(request, state)
            .await
            .map(|json| json.0)
            .map_err(|rejection| invalid(rejection.body_text()))
    }
}

impl FromBody for Empty {
    type Inner = ();

    async fn from_request<S: Send + Sync>(
        _: Request,
        _: &S,
    ) -> Result<(), ApiError> {
        Ok(())
    }
}

impl FromBody for Raw {
    type Inner = Raw;

    async fn from_request<S: Send + Sync>(
        request: Request,
        state: &S,
    ) -> Result<Raw, ApiError> {
        let content_type = request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_owned();
        let bytes = Bytes::from_request(request, state)
            .await
            .map_err(|rejection| invalid(rejection.body_text()))?;
        Ok(Raw {
            content_type,
            bytes,
        })
    }
}

impl FromBody for Stream {
    type Inner = Body;

    async fn from_request<S: Send + Sync>(
        request: Request,
        _: &S,
    ) -> Result<Body, ApiError> {
        Ok(request.into_body())
    }
}

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

impl IntoResponse for Empty {
    fn into_response(self) -> Response {
        StatusCode::NO_CONTENT.into_response()
    }
}

impl IntoResponse for Raw {
    fn into_response(self) -> Response {
        ([(header::CONTENT_TYPE, self.content_type)], self.bytes)
            .into_response()
    }
}

fn invalid(description: impl Into<String>) -> ApiError {
    ApiError::new(ErrorCode::InvalidInput, description)
}

#[cfg(test)]
mod tests {
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde::{Deserialize, Serialize};
    use tower::ServiceExt;

    use super::{Req, route};
    use crate::{ApiError, Auth, Empty, Endpoint, ErrorCode, Json, Method};

    #[derive(Serialize, Deserialize)]
    struct File {
        id: String,
        path: String,
    }

    #[derive(Serialize, Deserialize)]
    struct Limit {
        limit: u32,
    }

    struct RenameFile;

    impl Endpoint for RenameFile {
        const METHOD: Method = Method::Post;
        const PATH: &'static str = "/v1/instances/{id}/files/{*path}";
        const AUTH: Auth = Auth::Session;
        type Params = File;
        type Query = Limit;
        type Body = Json<String>;
        type Response = Json<String>;
    }

    async fn rename(request: Req<RenameFile>) -> Json<String> {
        let Req {
            params,
            query,
            body,
        } = request;
        Json(format!(
            "{} {} {} {body}",
            params.id, params.path, query.limit
        ))
    }

    async fn send(body: &'static str) -> (StatusCode, Vec<u8>) {
        let (path, handler) = route::<RenameFile, _, _, ()>(rename);
        let file = File {
            id: "a b/c".into(),
            path: "config/x y.json".into(),
        };
        let url = crate::client::url::<RenameFile>("", &file).unwrap();
        let request = Request::post(format!("{url}?limit=3"))
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();
        let response = Router::new()
            .route(path, handler)
            .oneshot(request)
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, bytes.to_vec())
    }

    #[tokio::test]
    async fn the_server_reads_what_the_client_sends() {
        assert_eq!(
            send(r#""new""#).await,
            (StatusCode::OK, br#""a b/c config/x y.json 3 new""#.to_vec())
        );
        let (status, body) = send("{").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let error: ApiError = serde_json::from_slice(&body).unwrap();
        assert_eq!(error.error, ErrorCode::InvalidInput);
    }

    struct Ping;

    impl Endpoint for Ping {
        const METHOD: Method = Method::Get;
        const PATH: &'static str = "/v1/ping";
        const AUTH: Auth = Auth::None;
        type Params = ();
        type Query = ();
        type Body = Empty;
        type Response = Empty;
    }

    async fn ping(_: Req<Ping>) -> Empty {
        Empty
    }

    #[tokio::test]
    async fn a_route_without_a_query_ignores_one() {
        let (path, handler) = route::<Ping, _, _, ()>(ping);
        let response = Router::new()
            .route(path, handler)
            .oneshot(Request::get("/v1/ping?v=2").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }
}
