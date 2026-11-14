use contracts::{Auth, Empty, Endpoint, Method, Upgrade};

use crate::server::events::LocalEvent;

/// Sends every engine and App event; reads nothing.
pub struct EventSocket;

impl Endpoint for EventSocket {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/events";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Empty;
    type Response = Upgrade<LocalEvent, ()>;
}
