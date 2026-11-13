//! Serves `crate::server::endpoints::accounts` over the App's sign-in code.

use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{Router, extract::State};
use contracts::{
    ApiError, Empty, ErrorCode, Json,
    server::{Req, route},
};
use engine::Engine;
use tauri::{Manager, UserAttentionType, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;
use tokio::time::Instant;
use url::Url;
use uuid::Uuid;

use crate::{
    accounts::{
        api_error,
        minecraft_auth::{self, MinecraftAccountSummary},
        mr_auth,
        oauth_utils::{
            auth_code_reply::{AuthReplySession, cancel_pending_auth},
            auth_seal::AuthSeal,
        },
        state::ModrinthCredentials,
    },
    desktop::{AppSlot, internal},
    presence::Presence,
    server::{
        AppState,
        endpoints::accounts::{
            CancelModrinthSignIn, CheckMinecraftReachable,
            GetActiveModrinthAccount, GetDefaultMinecraftAccount,
            ListMinecraftAccounts, ListModrinthAccounts,
            RemoveMinecraftAccount, RemoveModrinthAccount,
            SetActiveModrinthAccount, SetDefaultMinecraftAccount,
            SignInToMinecraft, SignInToModrinth, SignOutOfModrinth,
        },
    },
};

type Answer<T> = Result<T, ApiError>;

const LAUNCHER_AUTH_PROTOCOL: &str = "2";
/// Where Microsoft sends the sign-in window once the person has signed in.
const MICROSOFT_REDIRECT: &str = "https://login.live.com/oauth20_desktop.srf";
const MICROSOFT_SIGN_IN_TIMEOUT: Duration = Duration::from_secs(10 * 60);

pub fn routes() -> Router<AppState> {
    [
        route::<CheckMinecraftReachable, _, _, _>(check_minecraft_reachable),
        route::<SignInToMinecraft, _, _, _>(sign_in_to_minecraft),
        route::<ListMinecraftAccounts, _, _, _>(list_minecraft_accounts),
        route::<GetDefaultMinecraftAccount, _, _, _>(
            get_default_minecraft_account,
        ),
        route::<SetDefaultMinecraftAccount, _, _, _>(
            set_default_minecraft_account,
        ),
        route::<RemoveMinecraftAccount, _, _, _>(remove_minecraft_account),
        route::<SignInToModrinth, _, _, _>(sign_in_to_modrinth),
        route::<CancelModrinthSignIn, _, _, _>(cancel_modrinth_sign_in),
        route::<SignOutOfModrinth, _, _, _>(sign_out_of_modrinth),
        route::<GetActiveModrinthAccount, _, _, _>(get_active_modrinth_account),
        route::<ListModrinthAccounts, _, _, _>(list_modrinth_accounts),
        route::<SetActiveModrinthAccount, _, _, _>(set_active_modrinth_account),
        route::<RemoveModrinthAccount, _, _, _>(remove_modrinth_account),
    ]
    .into_iter()
    .fold(Router::new(), |router, (path, method)| {
        router.route(path, method)
    })
}

async fn check_minecraft_reachable(
    _: Req<CheckMinecraftReachable>,
) -> Answer<Empty> {
    minecraft_auth::check_reachable()
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn sign_in_to_minecraft(
    State(engine): State<Arc<Engine>>,
    State(app): State<AppSlot>,
    _: Req<SignInToMinecraft>,
) -> Answer<Json<MinecraftAccountSummary>> {
    let flow = minecraft_auth::begin_login(&engine)
        .await
        .map_err(api_error)?;
    let app = app.wait().await;
    if let Some(window) = app.get_webview_window("signin") {
        window.close().map_err(internal)?;
    }
    let window = WebviewWindowBuilder::new(
        app,
        "signin",
        WebviewUrl::External(flow.auth_request_uri.parse().map_err(internal)?),
    )
    .title("Sign into Modrinth")
    .always_on_top(true)
    .min_inner_size(500.0, 500.0)
    .inner_size(1000.0, 700.0)
    .focused(true)
    .center()
    .build()
    .map_err(internal)?;
    window
        .request_user_attention(Some(UserAttentionType::Critical))
        .map_err(internal)?;

    let code =
        microsoft_code(|| window.title().ok().and_then(|_| window.url().ok()))
            .await;
    window.close().ok();
    let credentials = minecraft_auth::finish_login(&engine, &code?, flow)
        .await
        .map_err(api_error)?;
    Ok(Json(credentials.account_summary().await))
}

/// Polls the sign-in window, which `address` reads as `None` once it is closed, until Microsoft
/// sends it back with a code. The window staying open too long cancels the sign-in too.
async fn microsoft_code(address: impl Fn() -> Option<Url>) -> Answer<String> {
    let deadline = Instant::now() + MICROSOFT_SIGN_IN_TIMEOUT;
    while Instant::now() < deadline {
        let Some(url) = address() else { break };
        if url.as_str().starts_with(MICROSOFT_REDIRECT)
            && let Some((_, code)) =
                url.query_pairs().find(|(key, _)| key == "code")
        {
            return Ok(code.into_owned());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err(sign_in_cancelled())
}

async fn list_minecraft_accounts(
    State(engine): State<Arc<Engine>>,
    _: Req<ListMinecraftAccounts>,
) -> Answer<Json<Vec<MinecraftAccountSummary>>> {
    minecraft_auth::users(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_default_minecraft_account(
    State(engine): State<Arc<Engine>>,
    _: Req<GetDefaultMinecraftAccount>,
) -> Answer<Json<Option<Uuid>>> {
    minecraft_auth::get_default_user(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn set_default_minecraft_account(
    State(engine): State<Arc<Engine>>,
    Req { body, .. }: Req<SetDefaultMinecraftAccount>,
) -> Answer<Empty> {
    minecraft_auth::set_default_user(&engine, body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn remove_minecraft_account(
    State(engine): State<Arc<Engine>>,
    Req { params, .. }: Req<RemoveMinecraftAccount>,
) -> Answer<Empty> {
    minecraft_auth::remove_user(&engine, params.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

/// The browser hands the session back to a loopback listener or through a `modrinth://auth`
/// deep link; `crate::api::utils` delivers the deep link.
async fn sign_in_to_modrinth(
    State(engine): State<Arc<Engine>>,
    State(presence): State<Arc<Presence>>,
    State(app): State<AppSlot>,
    Req { body, .. }: Req<SignInToModrinth>,
) -> Answer<Json<ModrinthCredentials>> {
    let seal = AuthSeal::generate();
    let mut reply = AuthReplySession::start(seal.nonce().to_string());
    let socket = reply.socket_addr().await.map_err(api_error)?;
    let auth_request_uri = auth_request_url(
        &mr_auth::authenticate_begin_flow(&engine, body.flow)
            .await
            .map_err(api_error)?,
        &socket,
        &seal.public_key().map_err(api_error)?,
        seal.nonce(),
        body.add_account,
    )?;

    let app = app.wait().await;
    app.opener()
        .open_url(auth_request_uri, None::<&str>)
        .map_err(internal)?;

    let session_token = modrinth_session(reply, &seal).await?;
    let credentials =
        mr_auth::authenticate_finish_flow(&engine, &presence, &session_token)
            .await
            .map_err(api_error)?;

    if let Some(main_window) = app.get_window("main") {
        main_window.set_focus().ok();
    }

    Ok(Json(credentials))
}

/// Waits for the sealed session; the cancel route ends the wait with `sign_in_cancelled`.
async fn modrinth_session(
    mut reply: AuthReplySession,
    seal: &AuthSeal,
) -> Answer<String> {
    let Some(sealed) = reply.wait().await.map_err(api_error)? else {
        return Err(sign_in_cancelled());
    };
    seal.open(&sealed).map_err(api_error)
}

async fn cancel_modrinth_sign_in(_: Req<CancelModrinthSignIn>) -> Empty {
    cancel_pending_auth();
    Empty
}

async fn sign_out_of_modrinth(
    State(engine): State<Arc<Engine>>,
    State(presence): State<Arc<Presence>>,
    _: Req<SignOutOfModrinth>,
) -> Answer<Empty> {
    mr_auth::logout(&engine, &presence)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn get_active_modrinth_account(
    State(engine): State<Arc<Engine>>,
    State(presence): State<Arc<Presence>>,
    _: Req<GetActiveModrinthAccount>,
) -> Answer<Json<Option<ModrinthCredentials>>> {
    mr_auth::get_credentials(&engine, &presence)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn list_modrinth_accounts(
    State(engine): State<Arc<Engine>>,
    _: Req<ListModrinthAccounts>,
) -> Answer<Json<Vec<ModrinthCredentials>>> {
    mr_auth::get_all(&engine).await.map(Json).map_err(api_error)
}

async fn set_active_modrinth_account(
    State(engine): State<Arc<Engine>>,
    State(presence): State<Arc<Presence>>,
    Req { body, .. }: Req<SetActiveModrinthAccount>,
) -> Answer<Empty> {
    mr_auth::set_active(&engine, &presence, &body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn remove_modrinth_account(
    State(engine): State<Arc<Engine>>,
    State(presence): State<Arc<Presence>>,
    Req { params, .. }: Req<RemoveModrinthAccount>,
) -> Answer<Empty> {
    mr_auth::remove_user(&engine, &presence, &params.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

fn sign_in_cancelled() -> ApiError {
    ApiError::new(ErrorCode::SignInCancelled, "Sign-in was cancelled")
}

fn auth_request_url(
    begin_url: &str,
    socket: &SocketAddr,
    public_key: &str,
    nonce: &str,
    add_account: bool,
) -> Answer<String> {
    let mut url = Url::parse(begin_url).map_err(internal)?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("launcher", "true");
        query.append_pair("protocol", LAUNCHER_AUTH_PROTOCOL);
        query.append_pair("key", public_key);
        query.append_pair("nonce", nonce);
        query.append_pair("ipver", if socket.is_ipv4() { "4" } else { "6" });
        query.append_pair("port", &socket.port().to_string());
        if add_account {
            query.append_pair("add_account", "true");
        }
    }

    Ok(url.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn closing_the_microsoft_window_cancels_and_its_redirect_gives_the_code()
     {
        let closed = microsoft_code(|| None).await.unwrap_err();
        assert_eq!(closed.error, ErrorCode::SignInCancelled);

        let redirect = Url::parse(
            "https://login.live.com/oauth20_desktop.srf?code=M.code&lc=1033",
        )
        .unwrap();
        let code = microsoft_code(|| Some(redirect.clone())).await.unwrap();
        assert_eq!(code, "M.code");
    }

    #[tokio::test]
    async fn cancelling_a_modrinth_sign_in_answers_sign_in_cancelled() {
        let seal = AuthSeal::generate();
        let mut reply = AuthReplySession::start(seal.nonce().to_string());
        reply.socket_addr().await.unwrap();
        let waiting =
            tokio::spawn(async move { modrinth_session(reply, &seal).await });

        cancel_pending_auth();

        let cancelled = waiting.await.unwrap().unwrap_err();
        assert_eq!(cancelled.error, ErrorCode::SignInCancelled);
    }
}
