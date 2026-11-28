//! The CLI's caller of any `contracts` Endpoint, over the local socket.

use color_eyre::eyre::{Result, WrapErr};
use contracts::{
    Endpoint,
    client::{Encode, Reply, request},
};

use super::Client;

pub async fn call<E: Endpoint>(
    client: &Client,
    params: &E::Params,
    query: &E::Query,
    body: <E::Body as Encode>::Input,
) -> Result<<E::Response as Reply>::Output>
where
    E::Body: Encode,
    E::Response: Reply,
{
    let (status, content_type, answer) = client
        .send(request::<E>("", params, query, body, None)?)
        .await?;
    E::Response::decode(status, content_type.as_deref(), answer)
        .wrap_err("Amberite Core answered something this CLI cannot read")
}
