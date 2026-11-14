use contracts::{Auth, Empty, Endpoint, Method};

pub struct CheckHealth;

impl Endpoint for CheckHealth {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/health";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Empty;
    type Response = Empty;
}
