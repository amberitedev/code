//! The backend's account routes: sign-up, sign-in, sessions, passwords, two-factor, email and the
//! admin's account controls. They keep labrinth's paths and shapes, so upstream's pages call them as
//! they call Modrinth; upstream calls them under `/v2`, `/v3` and `/_internal`, which the backend
//! strips. Every other `/auth/` path answers 501 `unsupported_operation`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::social::{Never, User, UserRef};
use crate::{Auth, Empty, Endpoint, Json, Method};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Globals {
    #[cfg_attr(feature = "ts", ts(type = "Record<string, number>"))]
    pub tax_compliance_thresholds: HashMap<String, u64>,
    pub captcha_enabled: bool,
    pub max_user_icon_size: u64,
    pub max_project_icon_size: u64,
    pub max_organization_icon_size: u64,
    pub max_collection_icon_size: u64,
    pub max_oauth_client_icon_size: u64,
    pub max_gallery_image_size: u64,
}

/// Body of [`CreateAccount`] and [`ValidateNewAccount`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NewAccount {
    pub username: String,
    pub password: String,
    pub email: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub account_consent: Option<bool>,
    /// Also issue the App a session of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub app_session: Option<bool>,
}

/// Body of [`LogIn`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LoginCredentials {
    /// The username or the email.
    pub username: String,
    pub password: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub app_session: Option<bool>,
}

/// Body of [`LogInTwoFactor`]: a code from the authenticator or a backup code.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TwoFactorLogin {
    pub code: String,
    pub flow: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub app_session: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum TwoFactorRequired {
    #[serde(rename = "2fa_required")]
    TwoFactorRequired,
}

/// The account has two-factor on: finish with [`LogInTwoFactor`] and this `flow`. Labrinth sends
/// it at 200, shaped like an error.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TwoFactorChallenge {
    pub error: TwoFactorRequired,
    pub description: String,
    pub flow: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(untagged)]
pub enum LoginAnswer {
    TwoFactorRequired(TwoFactorChallenge),
    SignedIn(AuthSession),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SessionDetails {
    pub id: String,
    pub user_id: String,
    pub created: String,
    pub last_login: String,
    pub expires: String,
    /// Until when [`RefreshSession`] can rotate it.
    pub refresh_expires: String,
    pub user_agent: String,
    pub ip: String,
    pub os: Option<Never>,
    pub platform: Option<Never>,
    pub city: Option<Never>,
    pub country: Option<Never>,
    /// Whether it is the session making the request.
    pub current: bool,
}

/// A session as [`ListSessions`] shows it. Its token is never sent again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UserSession {
    #[serde(flatten)]
    pub details: SessionDetails,
    pub session: Option<Never>,
}

/// A session just issued, with its `mra_` token, and the App's own token when it asked for one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AuthSession {
    #[serde(flatten)]
    pub details: SessionDetails,
    pub session: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub app_session: Option<String>,
}

/// A session's id, or its token.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SessionRef {
    pub id: String,
}

/// Body of [`RequestPasswordReset`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PasswordReset {
    /// The username or the email.
    pub username: String,
}

/// Body of [`ChangePassword`]: the `flow` of a reset email, or the signed-in account's
/// `old_password`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PasswordUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub flow: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub old_password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub new_password: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TwoFactorSecret {
    /// Base32, for the authenticator.
    pub secret: String,
    pub flow: String,
}

/// Body of [`EnableTwoFactor`]: a code for the secret of `flow`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TwoFactorSetup {
    pub code: String,
    pub flow: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BackupCodes {
    pub backup_codes: Vec<String>,
}

/// Body of [`DisableTwoFactor`]: a code from the authenticator or a backup code.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TwoFactorCode {
    pub code: String,
}

/// Body of [`VerifyEmail`]: the `flow` the verification email carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EmailVerification {
    pub flow: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EmailUpdate {
    pub email: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LockReason {
    pub reason: String,
}

/// Body of [`ForcePasswordReset`]: where to send the recovery email, the account's own when absent.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ForcedPasswordReset {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub email: Option<String>,
}

/// Limits and settings upstream's pages read before signing in.
pub struct GetGlobals;

/// Signs up with a password and sends a verification email.
pub struct CreateAccount;

/// Checks a sign-up without creating the account.
pub struct ValidateNewAccount;

/// Signs in with a password, or asks for a second factor.
pub struct LogIn;

pub struct LogInTwoFactor;

/// Rotates the calling session within its refresh window.
pub struct RefreshSession;

/// The account's unexpired sessions, newest first.
pub struct ListSessions;

/// Signs one of the account's sessions out.
pub struct DeleteSession;

/// Emails a reset link, if the account exists and is not locked. Answers the same either way.
pub struct RequestPasswordReset;

pub struct ChangePassword;

/// Starts two-factor setup with a new secret.
pub struct StartTwoFactorSetup;

pub struct EnableTwoFactor;

pub struct DisableTwoFactor;

pub struct VerifyEmail;

pub struct ResendVerifyEmail;

/// Changes the email and sends a verification email to the new one.
pub struct ChangeEmail;

/// The caller's own account, private fields included.
pub struct GetCurrentUser;

/// Always empty: this backend has no passkeys.
pub struct ListPasskeys;

/// Admins only. Locks an account that is not staff and signs it out everywhere.
pub struct LockUser;

/// Admins only.
pub struct UnlockUser;

/// Admins only.
pub struct RevokeUserSessions;

/// Admins only. Clears the password of an account that is not staff, signs it out and emails a
/// reset link.
pub struct ForcePasswordReset;

/// Admins only. Turns two-factor off for an account that is not staff.
pub struct ResetUserTwoFactor;

macro_rules! endpoints {
    ($($name:ident: $method:ident $path:literal, $auth:ident, $params:ty, $body:ty => $response:ty;)+) => {
        $(impl Endpoint for $name {
            const METHOD: Method = Method::$method;
            const PATH: &'static str = $path;
            const AUTH: Auth = Auth::$auth;
            type Params = $params;
            type Query = ();
            type Body = $body;
            type Response = $response;
        })+

        #[cfg(test)]
        fn endpoint_entries() -> String {
            [$(format!("\t{}: {},\n", stringify!($name), crate::ts::endpoint::<$name>())),+].concat()
        }
    };
}

endpoints! {
    GetGlobals: Get "/globals", None, (), Empty => Json<Globals>;
    CreateAccount: Post "/auth/create", None, (), Json<NewAccount> => Json<AuthSession>;
    ValidateNewAccount: Post "/auth/create/validate", None, (), Json<NewAccount> => Empty;
    LogIn: Post "/auth/login", None, (), Json<LoginCredentials> => Json<LoginAnswer>;
    LogInTwoFactor: Post "/auth/login/2fa", None, (), Json<TwoFactorLogin> => Json<AuthSession>;
    RefreshSession: Post "/session/refresh", Account, (), Empty => Json<AuthSession>;
    ListSessions: Get "/session/list", Account, (), Empty => Json<Vec<UserSession>>;
    DeleteSession: Delete "/session/{id}", Account, SessionRef, Empty => Empty;
    RequestPasswordReset: Post "/auth/password/reset", None, (), Json<PasswordReset> => Empty;
    ChangePassword: Patch "/auth/password", Account, (), Json<PasswordUpdate> => Empty;
    StartTwoFactorSetup: Post "/auth/2fa/get_secret", Account, (), Empty => Json<TwoFactorSecret>;
    EnableTwoFactor: Post "/auth/2fa", Account, (), Json<TwoFactorSetup> => Json<BackupCodes>;
    DisableTwoFactor: Delete "/auth/2fa", Account, (), Json<TwoFactorCode> => Empty;
    VerifyEmail: Post "/auth/email/verify", None, (), Json<EmailVerification> => Empty;
    ResendVerifyEmail: Post "/auth/email/resend_verify", Account, (), Empty => Empty;
    ChangeEmail: Patch "/auth/email", Account, (), Json<EmailUpdate> => Empty;
    GetCurrentUser: Get "/user", Account, (), Empty => Json<User>;
    ListPasskeys: Get "/auth/passkey", Account, (), Empty => Json<Vec<Never>>;
    LockUser: Put "/admin/user/{id}/lock", Account, UserRef, Json<LockReason> => Empty;
    UnlockUser: Delete "/admin/user/{id}/lock", Account, UserRef, Empty => Empty;
    RevokeUserSessions: Delete "/admin/user/{id}/sessions", Account, UserRef, Empty => Empty;
    ForcePasswordReset: Post "/admin/user/{id}/password-reset", Account, UserRef, Json<ForcedPasswordReset> => Empty;
    ResetUserTwoFactor: Delete "/admin/user/{id}/2fa", Account, UserRef, Empty => Empty;
}

#[cfg(test)]
pub(crate) fn ts() -> String {
    use ts_rs::TS;

    let cfg = crate::ts::config();
    let declarations = [
        Globals::decl(&cfg),
        NewAccount::decl(&cfg),
        LoginCredentials::decl(&cfg),
        TwoFactorLogin::decl(&cfg),
        TwoFactorRequired::decl(&cfg),
        TwoFactorChallenge::decl(&cfg),
        LoginAnswer::decl(&cfg),
        UserSession::decl(&cfg),
        AuthSession::decl(&cfg),
        SessionRef::decl(&cfg),
        PasswordReset::decl(&cfg),
        PasswordUpdate::decl(&cfg),
        TwoFactorSecret::decl(&cfg),
        TwoFactorSetup::decl(&cfg),
        BackupCodes::decl(&cfg),
        TwoFactorCode::decl(&cfg),
        EmailVerification::decl(&cfg),
        EmailUpdate::decl(&cfg),
        LockReason::decl(&cfg),
        ForcedPasswordReset::decl(&cfg),
    ]
    .map(|declaration| format!("\nexport {declaration}\n"))
    .concat();
    format!(
        "{declarations}\n/** The backend's account routes, in labrinth's shapes. */\nexport const accounts = {{\n{}}}\n",
        endpoint_entries()
    )
}
