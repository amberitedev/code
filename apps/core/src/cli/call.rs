//! The CLI's caller of any `contracts` Endpoint, over the local socket.

use axum::http::Method as HttpMethod;
use bytes::Bytes;
use color_eyre::eyre::{Result, WrapErr};
use contracts::{Empty, Endpoint, Json, Method, client::url};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use super::Client;

/// A request body the CLI sends.
pub trait Body {
    type Input;
    fn bytes(input: Self::Input) -> Result<Bytes>;
}

impl<T: Serialize> Body for Json<T> {
    type Input = T;

    fn bytes(input: T) -> Result<Bytes> {
        Ok(serde_json::to_vec(&input)?.into())
    }
}

impl Body for Empty {
    type Input = ();

    fn bytes((): ()) -> Result<Bytes> {
        Ok(Bytes::new())
    }
}

/// A response body the CLI reads.
pub trait Reply {
    type Output;
    fn read(body: &[u8]) -> Result<Self::Output>;
}

impl<T: DeserializeOwned> Reply for Json<T> {
    type Output = T;

    fn read(body: &[u8]) -> Result<T> {
        serde_json::from_slice(body)
            .wrap_err("Core answered something this CLI cannot read")
    }
}

impl Reply for Empty {
    type Output = ();

    fn read(_: &[u8]) -> Result<()> {
        Ok(())
    }
}

pub async fn call<E: Endpoint>(
    client: &Client,
    params: &E::Params,
    query: &E::Query,
    body: <E::Body as Body>::Input,
) -> Result<<E::Response as Reply>::Output>
where
    E::Body: Body,
    E::Response: Reply,
{
    let method = match E::METHOD {
        Method::Get => HttpMethod::GET,
        Method::Post => HttpMethod::POST,
        Method::Put => HttpMethod::PUT,
        Method::Patch => HttpMethod::PATCH,
        Method::Delete => HttpMethod::DELETE,
    };
    let path = url::<E>("", params)? + &search(query)?;
    let answer = client.send(method, &path, E::Body::bytes(body)?).await?;
    E::Response::read(&answer)
}

/// `query` as `?name=value&...`, leaving out absent fields, as the TypeScript client does.
fn search(query: &impl Serialize) -> Result<String> {
    let Value::Object(fields) = serde_json::to_value(query)? else {
        return Ok(String::new());
    };
    let mut pairs = ::url::form_urlencoded::Serializer::new(String::new());
    for (name, value) in fields {
        match value {
            Value::Null => {}
            Value::String(text) => {
                pairs.append_pair(&name, &text);
            }
            other => {
                pairs.append_pair(&name, &other.to_string());
            }
        }
    }
    let pairs = pairs.finish();
    Ok(if pairs.is_empty() {
        pairs
    } else {
        format!("?{pairs}")
    })
}
