use crate::ErrorKind;
use crate::util::fetch::INSECURE_REQWEST_CLIENT;
use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::Engine;
use base64::prelude::{BASE64_STANDARD, BASE64_URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, TimeZone, Utc};
use dashmap::DashMap;
use heck::ToTitleCase;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use p256::pkcs8::{DecodePrivateKey, EncodePrivateKey, LineEnding};
use rand::rngs::OsRng;
use rand::{Rng, RngCore};
use reqwest::header::HeaderMap;
use reqwest::{Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::json;
use sha2::Digest;
use sqlx::Row;
use std::borrow::Cow;
use std::collections::HashMap;
use std::future::Future;
use std::hash::{BuildHasherDefault, DefaultHasher};
use std::io;
use std::ops::Deref;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Instant;
use tokio::runtime::{Handle, RuntimeFlavor};
use tokio::sync::Mutex;
use tokio::task;
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub enum MinecraftAuthStep {
    GetDeviceToken,
    SisuAuthenticate,
    GetOAuthToken,
    RefreshOAuthToken,
    SisuAuthorize,
    XstsAuthorize,
    MinecraftToken,
    MinecraftEntitlements,
    MinecraftProfile,
}

#[derive(thiserror::Error, Debug)]
pub enum MinecraftAuthenticationError {
    #[error("Error reading public key during generation")]
    ReadingPublicKey,
    #[error("Failed to serialize private key to PEM: {0}")]
    PEMSerialize(#[from] p256::pkcs8::Error),
    #[error("Failed to serialize body to JSON during step {step:?}: {source}")]
    SerializeBody {
        step: MinecraftAuthStep,
        #[source]
        source: serde_json::Error,
    },
    #[error(
        "Failed to deserialize response to JSON during step {step:?}: {source}. Status Code: {status_code} Body: {raw}"
    )]
    DeserializeResponse {
        step: MinecraftAuthStep,
        raw: String,
        #[source]
        source: serde_json::Error,
        status_code: StatusCode,
    },
    #[error("Request failed during step {step:?}: {source}")]
    Request {
        step: MinecraftAuthStep,
        #[source]
        source: reqwest::Error,
    },
    #[error("Error reading XBOX Session ID header")]
    NoSessionId,
    #[error("Error reading user hash")]
    NoUserHash,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MinecraftLoginFlow {
    pub verifier: String,
    pub challenge: String,
    pub session_id: String,
    pub auth_request_uri: String,
    #[serde(skip)]
    staged_device: Option<StoredDeviceTokenPair>,
}

#[tracing::instrument]
pub async fn login_begin(
    exec: &sqlx::Pool<sqlx::Sqlite>,
) -> crate::Result<MinecraftLoginFlow> {
    login_begin_inner(true, true, exec).await
}

#[tracing::instrument]
pub async fn login_begin_with_prompt(
    select_account: bool,
    exec: &sqlx::Pool<sqlx::Sqlite>,
) -> crate::Result<MinecraftLoginFlow> {
    login_begin_inner(select_account, true, exec).await
}

#[tracing::instrument]
pub async fn login_begin_staged_with_prompt(
    select_account: bool,
    exec: &sqlx::Pool<sqlx::Sqlite>,
) -> crate::Result<MinecraftLoginFlow> {
    login_begin_inner(select_account, false, exec).await
}

async fn login_begin_inner(
    select_account: bool,
    persist_device: bool,
    exec: &sqlx::Pool<sqlx::Sqlite>,
) -> crate::Result<MinecraftLoginFlow> {
    let (pair, current_date) = DeviceTokenPair::refresh_and_get_device_token(
        Utc::now(),
        exec,
        persist_device,
    )
    .await?;

    let verifier = generate_oauth_challenge();
    let result = sha2::Sha256::digest(&verifier);
    let challenge = BASE64_URL_SAFE_NO_PAD.encode(result);

    match sisu_authenticate(
        &pair.token.token,
        &challenge,
        &pair.key,
        current_date,
        select_account,
    )
    .await
    {
        Ok((session_id, redirect_uri)) => {
            let staged_device = if persist_device {
                None
            } else {
                Some(StoredDeviceTokenPair::try_from(&pair)?)
            };
            return Ok(MinecraftLoginFlow {
                verifier,
                challenge,
                session_id,
                auth_request_uri: redirect_uri.value.msa_oauth_redirect,
                staged_device,
            });
        }
        Err(err) => return Err(crate::ErrorKind::from(err).into()),
    }
}

#[tracing::instrument(skip_all)]
pub async fn login_finish(
    code: &str,
    flow: MinecraftLoginFlow,
    exec: &sqlx::Pool<sqlx::Sqlite>,
) -> crate::Result<Credentials> {
    Ok(login_finish_inner(code, flow, exec, true)
        .await?
        .credentials)
}

#[tracing::instrument(skip_all)]
pub async fn login_finish_staged(
    code: &str,
    flow: MinecraftLoginFlow,
    exec: &sqlx::Pool<sqlx::Sqlite>,
) -> crate::Result<StagedMinecraftLogin> {
    login_finish_inner(code, flow, exec, false).await
}

async fn login_finish_inner(
    code: &str,
    flow: MinecraftLoginFlow,
    exec: &sqlx::Pool<sqlx::Sqlite>,
    persist: bool,
) -> crate::Result<StagedMinecraftLogin> {
    let pair = match flow.staged_device.clone() {
        Some(stored) => DeviceTokenPair::try_from(stored)?,
        None => {
            DeviceTokenPair::refresh_and_get_device_token(
                Utc::now(),
                exec,
                persist,
            )
            .await?
            .0
        }
    };

    let oauth_token = oauth_token(code, &flow.verifier).await?;
    let sisu_authorize = sisu_authorize(
        Some(&flow.session_id),
        &oauth_token.value.access_token,
        &pair.token.token,
        &pair.key,
        oauth_token.date,
    )
    .await?;

    let xbox_token = xsts_authorize(
        sisu_authorize.value,
        &pair.token.token,
        &pair.key,
        sisu_authorize.date,
    )
    .await?;
    let minecraft_token = minecraft_token(xbox_token.value).await?;

    minecraft_entitlements(&minecraft_token.access_token).await?;

    let mut credentials = Credentials {
        offline_profile: MinecraftProfile::default(),
        access_token: minecraft_token.access_token,
        refresh_token: oauth_token.value.refresh_token,
        expires: oauth_token.date
            + Duration::seconds(oauth_token.value.expires_in as i64),
        active: true,
    };

    // During login, we need to fetch the online profile at least once to get the
    // player UUID and name to use for the offline profile, in order for that offline
    // profile to make sense. It's also important to modify the returned credentials
    // object, as otherwise continued usage of it will skip the profile cache due to
    // the dummy UUID
    let online_profile = credentials
        .online_profile()
        .await
        .ok_or(io::Error::other("Failed to fetch player profile"))?;
    credentials.offline_profile = MinecraftProfile {
        id: online_profile.id,
        name: online_profile.name.clone(),
        ..credentials.offline_profile
    };

    if persist {
        credentials.upsert(exec).await?;
    }

    Ok(StagedMinecraftLogin {
        credentials,
        device_token: StoredDeviceTokenPair::try_from(&pair)?,
    })
}

#[derive(Deserialize, Debug)]
pub struct Credentials {
    /// The offline profile of the user these credentials are for.
    ///
    /// Such a profile can only be relied upon to have a proper player UUID, which is
    /// never changed. A potentially stale username may be available, but no other data
    /// such as skins or capes is available.
    #[serde(rename = "profile")]
    pub offline_profile: MinecraftProfile,
    pub access_token: String,
    pub refresh_token: String,
    pub expires: DateTime<Utc>,
    pub active: bool,
}

#[derive(Serialize, Debug, Clone)]
pub struct MinecraftAccountSummary {
    pub profile: MinecraftProfile,
    pub expires: DateTime<Utc>,
    pub active: bool,
}

const PRODUCT_SESSION_VERSION: u32 = 1;
const PRODUCT_SESSION_KEYRING_SERVICE: &str = "dev.amberite.app";
const DEFAULT_PRODUCT_SESSION_ACCOUNT: &str = "amberite-product-session";
static PRODUCT_SESSION_ACCOUNT: OnceLock<RwLock<String>> = OnceLock::new();

#[derive(Serialize, Deserialize, Debug, Clone)]
struct StoredMinecraftCredential {
    profile: MinecraftProfile,
    access_token: String,
    refresh_token: String,
    expires: DateTime<Utc>,
    active: bool,
}

impl From<&Credentials> for StoredMinecraftCredential {
    fn from(value: &Credentials) -> Self {
        Self {
            profile: value.offline_profile.clone(),
            access_token: value.access_token.clone(),
            refresh_token: value.refresh_token.clone(),
            expires: value.expires,
            active: value.active,
        }
    }
}

impl From<StoredMinecraftCredential> for Credentials {
    fn from(value: StoredMinecraftCredential) -> Self {
        Self {
            offline_profile: value.profile,
            access_token: value.access_token,
            refresh_token: value.refresh_token,
            expires: value.expires,
            active: value.active,
        }
    }
}

fn merge_minecraft_credential(
    bundle: &mut ProductSessionBundle,
    credentials: &Credentials,
) {
    if credentials.active {
        for credential in &mut bundle.minecraft_credentials {
            credential.active = false;
        }
    }
    let stored = StoredMinecraftCredential::from(credentials);
    if let Some(existing) = bundle
        .minecraft_credentials
        .iter_mut()
        .find(|value| value.profile.id == credentials.offline_profile.id)
    {
        *existing = stored;
    } else {
        bundle.minecraft_credentials.push(stored);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct StoredDeviceTokenPair {
    token: DeviceToken,
    key_id: Uuid,
    private_key: String,
    x: String,
    y: String,
}

pub struct StagedMinecraftLogin {
    pub credentials: Credentials,
    device_token: StoredDeviceTokenPair,
}

impl TryFrom<&DeviceTokenPair> for StoredDeviceTokenPair {
    type Error = crate::Error;

    fn try_from(value: &DeviceTokenPair) -> crate::Result<Self> {
        Ok(Self {
            token: value.token.clone(),
            key_id: value.key.id,
            private_key: value
                .key
                .key
                .to_pkcs8_pem(LineEnding::default())
                .map_err(MinecraftAuthenticationError::PEMSerialize)?
                .to_string(),
            x: value.key.x.clone(),
            y: value.key.y.clone(),
        })
    }
}

impl TryFrom<StoredDeviceTokenPair> for DeviceTokenPair {
    type Error = crate::Error;

    fn try_from(value: StoredDeviceTokenPair) -> crate::Result<Self> {
        Ok(Self {
            token: value.token,
            key: DeviceTokenKey {
                id: value.key_id,
                key: SigningKey::from_pkcs8_pem(&value.private_key)
                    .map_err(product_session_error)?,
                x: value.x,
                y: value.y,
            },
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RememberedAmberiteIdentity {
    pub minecraft_uuid: Uuid,
    pub verified_minecraft_handle: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub last_successful_sign_in: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AmberiteNativeSession {
    pub access_token: String,
    pub refresh_token: String,
    pub user: serde_json::Value,
    pub active_identity_uuid: Uuid,
    pub expires_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ProductSessionBundle {
    version: u32,
    minecraft_credentials: Vec<StoredMinecraftCredential>,
    device_token: Option<StoredDeviceTokenPair>,
    amberite: Option<AmberiteNativeSession>,
    remembered_identity: Option<RememberedAmberiteIdentity>,
    signed_out: bool,
}

impl Default for ProductSessionBundle {
    fn default() -> Self {
        Self {
            version: PRODUCT_SESSION_VERSION,
            minecraft_credentials: Vec::new(),
            device_token: None,
            amberite: None,
            remembered_identity: None,
            signed_out: true,
        }
    }
}

pub fn configure_product_session_account(account: impl Into<String>) {
    let lock = PRODUCT_SESSION_ACCOUNT.get_or_init(|| {
        RwLock::new(DEFAULT_PRODUCT_SESSION_ACCOUNT.to_string())
    });
    if let Ok(mut value) = lock.write() {
        *value = account.into();
    }
}

fn product_session_account() -> String {
    PRODUCT_SESSION_ACCOUNT
        .get_or_init(|| {
            RwLock::new(DEFAULT_PRODUCT_SESSION_ACCOUNT.to_string())
        })
        .read()
        .map(|value| value.clone())
        .unwrap_or_else(|_| DEFAULT_PRODUCT_SESSION_ACCOUNT.to_string())
}

fn product_session_error(error: impl std::fmt::Display) -> crate::Error {
    ErrorKind::OtherError(format!("secure product session failed: {error}"))
        .as_error()
}

#[cfg(not(test))]
fn product_session_key_entry() -> crate::Result<keyring::Entry> {
    keyring::Entry::new(
        PRODUCT_SESSION_KEYRING_SERVICE,
        &format!("{}:encryption-key", product_session_account()),
    )
    .map_err(product_session_error)
}

#[cfg(test)]
fn test_product_session_keys()
-> &'static std::sync::Mutex<HashMap<String, [u8; 32]>> {
    static KEYS: OnceLock<std::sync::Mutex<HashMap<String, [u8; 32]>>> =
        OnceLock::new();
    KEYS.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

#[cfg(not(test))]
fn read_product_session_key() -> crate::Result<Option<[u8; 32]>> {
    match product_session_key_entry()?.get_password() {
        Ok(value) => {
            let Ok(decoded) = BASE64_URL_SAFE_NO_PAD.decode(value) else {
                tracing::warn!("Secure product-session key is malformed");
                return Ok(None);
            };
            let Ok(key) = decoded.try_into() else {
                tracing::warn!(
                    "Secure product-session key has the wrong length"
                );
                return Ok(None);
            };
            Ok(Some(key))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(product_session_error(error)),
    }
}

#[cfg(test)]
fn read_product_session_key() -> crate::Result<Option<[u8; 32]>> {
    Ok(test_product_session_keys()
        .lock()
        .map_err(product_session_error)?
        .get(&product_session_account())
        .copied())
}

fn get_or_create_product_session_key() -> crate::Result<[u8; 32]> {
    if let Some(key) = read_product_session_key()? {
        return Ok(key);
    }
    let mut key = [0_u8; 32];
    OsRng.fill_bytes(&mut key);
    store_product_session_key(key)?;
    Ok(key)
}

#[cfg(not(test))]
fn store_product_session_key(key: [u8; 32]) -> crate::Result<()> {
    product_session_key_entry()?
        .set_password(&BASE64_URL_SAFE_NO_PAD.encode(key))
        .map_err(product_session_error)
}

#[cfg(test)]
fn store_product_session_key(key: [u8; 32]) -> crate::Result<()> {
    test_product_session_keys()
        .lock()
        .map_err(product_session_error)?
        .insert(product_session_account(), key);
    Ok(())
}

#[cfg(not(test))]
fn clear_product_session_key() {
    if let Ok(entry) = product_session_key_entry() {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(error) => tracing::warn!(
                "Failed to clear corrupt product-session key: {error}"
            ),
        }
    }
}

#[cfg(test)]
fn clear_product_session_key() {
    if let Ok(mut keys) = test_product_session_keys().lock() {
        keys.remove(&product_session_account());
    }
}

fn product_session_aad() -> String {
    format!(
        "amberite-product-session:{}:v{PRODUCT_SESSION_VERSION}",
        product_session_account()
    )
}

fn encrypt_product_session(
    bundle: &ProductSessionBundle,
    key: &[u8; 32],
) -> crate::Result<(Vec<u8>, [u8; 12])> {
    validate_product_session(bundle)?;
    let plaintext = serde_json::to_vec(bundle)?;
    let mut nonce = [0_u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(product_session_error)?;
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &plaintext,
                aad: product_session_aad().as_bytes(),
            },
        )
        .map_err(|_| product_session_error("bundle encryption failed"))?;
    Ok((ciphertext, nonce))
}

fn decrypt_product_session(
    ciphertext: &[u8],
    nonce: &[u8],
    key: &[u8; 32],
) -> crate::Result<ProductSessionBundle> {
    if nonce.len() != 12 {
        return Err(product_session_error("stored nonce has the wrong length"));
    }
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(product_session_error)?;
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: product_session_aad().as_bytes(),
            },
        )
        .map_err(|_| product_session_error("bundle authentication failed"))?;
    let bundle: ProductSessionBundle = serde_json::from_slice(&plaintext)?;
    validate_product_session(&bundle)?;
    Ok(bundle)
}

fn validate_product_session(
    bundle: &ProductSessionBundle,
) -> crate::Result<()> {
    if bundle.version != PRODUCT_SESSION_VERSION {
        return Err(product_session_error("unsupported bundle version"));
    }
    if bundle.signed_out && bundle.amberite.is_some() {
        return Err(product_session_error(
            "signed-out bundle contains an Amberite session",
        ));
    }
    if !bundle.signed_out && bundle.amberite.is_none() {
        return Err(product_session_error(
            "signed-in bundle has no Amberite session",
        ));
    }
    if bundle
        .minecraft_credentials
        .iter()
        .filter(|credential| credential.active)
        .count()
        > 1
    {
        return Err(product_session_error(
            "multiple launcher accounts are active",
        ));
    }
    if bundle.minecraft_credentials.iter().any(|credential| {
        credential.profile.id.is_nil()
            || credential.access_token.is_empty()
            || credential.refresh_token.is_empty()
    }) {
        return Err(product_session_error(
            "Minecraft credentials are incomplete",
        ));
    }
    if let Some(device) = &bundle.device_token
        && (device.key_id.is_nil()
            || device.private_key.is_empty()
            || device.token.token.is_empty())
    {
        return Err(product_session_error("device credentials are incomplete"));
    }
    if let Some(session) = &bundle.amberite {
        if session.access_token.is_empty()
            || session.refresh_token.is_empty()
            || session.active_identity_uuid.is_nil()
            || session.expires_at <= session.updated_at
        {
            return Err(product_session_error(
                "Amberite session is incomplete",
            ));
        }
        let user_uuid = session
            .user
            .get("minecraftUuid")
            .and_then(serde_json::Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok());
        if user_uuid != Some(session.active_identity_uuid) {
            return Err(product_session_error("Amberite user UUID mismatch"));
        }
        if bundle
            .remembered_identity
            .as_ref()
            .map(|identity| identity.minecraft_uuid)
            != Some(session.active_identity_uuid)
        {
            return Err(product_session_error(
                "remembered identity UUID mismatch",
            ));
        }
        if !bundle.minecraft_credentials.iter().any(|credential| {
            credential.active
                && credential.profile.id == session.active_identity_uuid
        }) {
            return Err(product_session_error(
                "active Minecraft credential UUID mismatch",
            ));
        }
    }
    Ok(())
}

async fn product_session_row_exists(
    pool: &sqlx::Pool<sqlx::Sqlite>,
) -> crate::Result<bool> {
    Ok(
        sqlx::query("SELECT 1 FROM amberite_product_session WHERE id = 0")
            .fetch_optional(pool)
            .await?
            .is_some(),
    )
}

async fn load_product_session(
    pool: &sqlx::Pool<sqlx::Sqlite>,
) -> crate::Result<ProductSessionBundle> {
    let Some(row) = sqlx::query(
        "SELECT version, encrypted_bundle, nonce, remembered_identity, signed_out FROM amberite_product_session WHERE id = 0",
    )
    .fetch_optional(pool)
    .await?
    else {
        return Ok(ProductSessionBundle::default());
    };
    let remembered = row
        .try_get::<Option<String>, _>("remembered_identity")?
        .and_then(|value| serde_json::from_str(&value).ok());
    let signed_out = row.try_get::<i64, _>("signed_out")? == 1;
    let ciphertext = row.try_get::<Option<Vec<u8>>, _>("encrypted_bundle")?;
    let nonce = row.try_get::<Option<Vec<u8>>, _>("nonce")?;
    let version = row.try_get::<i64, _>("version")? as u32;
    let Some(ciphertext) = ciphertext else {
        if !signed_out {
            return reset_corrupt_product_session(pool, remembered).await;
        }
        return Ok(ProductSessionBundle {
            version: PRODUCT_SESSION_VERSION,
            remembered_identity: remembered,
            signed_out: true,
            ..ProductSessionBundle::default()
        });
    };
    let Some(nonce) = nonce else {
        return reset_corrupt_product_session(pool, remembered).await;
    };
    let Some(key) = read_product_session_key()? else {
        return reset_corrupt_product_session(pool, remembered).await;
    };
    if version != PRODUCT_SESSION_VERSION {
        return reset_corrupt_product_session(pool, remembered).await;
    }
    match decrypt_product_session(&ciphertext, &nonce, &key) {
        Ok(bundle) => Ok(bundle),
        Err(error) => {
            tracing::warn!("Clearing corrupt secure product session: {error}");
            reset_corrupt_product_session(pool, remembered).await
        }
    }
}

async fn clear_legacy_secret_columns(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> crate::Result<()> {
    sqlx::query(
        "UPDATE minecraft_users SET access_token = '', refresh_token = ''",
    )
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        "UPDATE minecraft_device_tokens SET private_key = '', token = '', display_claims = json('{}')",
    )
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn reset_corrupt_product_session(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    remembered_identity: Option<RememberedAmberiteIdentity>,
) -> crate::Result<ProductSessionBundle> {
    clear_product_session_key();
    let bundle = ProductSessionBundle {
        version: PRODUCT_SESSION_VERSION,
        remembered_identity,
        signed_out: true,
        ..ProductSessionBundle::default()
    };
    let remembered = bundle
        .remembered_identity
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    let mut transaction = pool.begin().await?;
    sqlx::query(
        "INSERT INTO amberite_product_session (id, version, encrypted_bundle, nonce, remembered_identity, signed_out, updated_at) VALUES (0, ?, NULL, NULL, ?, 1, ?) ON CONFLICT(id) DO UPDATE SET version = excluded.version, encrypted_bundle = NULL, nonce = NULL, remembered_identity = excluded.remembered_identity, signed_out = 1, updated_at = excluded.updated_at",
    )
    .bind(PRODUCT_SESSION_VERSION as i64)
    .bind(remembered)
    .bind(Utc::now().timestamp())
    .execute(&mut *transaction)
    .await?;
    clear_legacy_secret_columns(&mut transaction).await?;
    transaction.commit().await?;
    Ok(bundle)
}

async fn write_product_session_row(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    bundle: &ProductSessionBundle,
) -> crate::Result<()> {
    let key = get_or_create_product_session_key()?;
    let (ciphertext, nonce) = encrypt_product_session(bundle, &key)?;
    let remembered = bundle
        .remembered_identity
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    sqlx::query(
        "INSERT INTO amberite_product_session (id, version, encrypted_bundle, nonce, remembered_identity, signed_out, updated_at) VALUES (0, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET version = excluded.version, encrypted_bundle = excluded.encrypted_bundle, nonce = excluded.nonce, remembered_identity = excluded.remembered_identity, signed_out = excluded.signed_out, updated_at = excluded.updated_at",
    )
    .bind(PRODUCT_SESSION_VERSION as i64)
    .bind(ciphertext)
    .bind(nonce.to_vec())
    .bind(remembered)
    .bind(bundle.signed_out)
    .bind(Utc::now().timestamp())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn save_product_session(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    bundle: &ProductSessionBundle,
) -> crate::Result<()> {
    let mut transaction = pool.begin().await?;
    write_product_session_row(&mut transaction, bundle).await?;
    transaction.commit().await?;
    Ok(())
}

pub async fn amberite_product_session(
    pool: &sqlx::Pool<sqlx::Sqlite>,
) -> crate::Result<Option<AmberiteNativeSession>> {
    Ok(load_product_session(pool).await?.amberite)
}

pub async fn remembered_amberite_identity(
    pool: &sqlx::Pool<sqlx::Sqlite>,
) -> crate::Result<Option<RememberedAmberiteIdentity>> {
    Ok(load_product_session(pool).await?.remembered_identity)
}

pub async fn commit_amberite_product_session(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    staged: StagedMinecraftLogin,
    expected_minecraft_uuid: Option<Uuid>,
    session: AmberiteNativeSession,
    remembered_identity: RememberedAmberiteIdentity,
) -> crate::Result<()> {
    let staged_uuid = staged.credentials.offline_profile.id;
    if expected_minecraft_uuid.is_some_and(|expected| expected != staged_uuid) {
        return Err(product_session_error("expected Minecraft UUID mismatch"));
    }
    if session.active_identity_uuid != staged_uuid
        || remembered_identity.minecraft_uuid != staged_uuid
    {
        return Err(product_session_error("staged identity UUID mismatch"));
    }

    let mut bundle = load_product_session(pool).await?;
    bundle.version = PRODUCT_SESSION_VERSION;
    let credentials = staged.credentials;
    let uuid = credentials.offline_profile.id;
    merge_minecraft_credential(&mut bundle, &credentials);
    bundle.device_token = Some(staged.device_token);
    bundle.amberite = Some(session);
    bundle.remembered_identity = Some(remembered_identity);
    bundle.signed_out = false;

    let mut transaction = pool.begin().await?;
    write_product_session_row(&mut transaction, &bundle).await?;
    sqlx::query("UPDATE minecraft_users SET active = FALSE")
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        "INSERT INTO minecraft_users (uuid, active, username, access_token, refresh_token, expires) VALUES (?, TRUE, ?, '', '', ?) ON CONFLICT(uuid) DO UPDATE SET active = TRUE, username = excluded.username, access_token = '', refresh_token = '', expires = excluded.expires",
    )
    .bind(uuid.as_hyphenated().to_string())
    .bind(&credentials.offline_profile.name)
    .bind(credentials.expires.timestamp())
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(())
}

pub async fn attach_legacy_amberite_product_session(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    credentials: Credentials,
    session: AmberiteNativeSession,
    remembered_identity: RememberedAmberiteIdentity,
) -> crate::Result<()> {
    let mut bundle = load_product_session(pool).await?;
    bundle.version = PRODUCT_SESSION_VERSION;
    let uuid = credentials.offline_profile.id;
    merge_minecraft_credential(&mut bundle, &credentials);
    bundle.amberite = Some(session);
    bundle.remembered_identity = Some(remembered_identity);
    bundle.signed_out = false;

    let mut transaction = pool.begin().await?;
    write_product_session_row(&mut transaction, &bundle).await?;
    sqlx::query("UPDATE minecraft_users SET active = FALSE")
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        "INSERT INTO minecraft_users (uuid, active, username, access_token, refresh_token, expires) VALUES (?, TRUE, ?, '', '', ?) ON CONFLICT(uuid) DO UPDATE SET active = TRUE, username = excluded.username, access_token = '', refresh_token = '', expires = excluded.expires",
    )
    .bind(uuid.as_hyphenated().to_string())
    .bind(&credentials.offline_profile.name)
    .bind(credentials.expires.timestamp())
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(())
}

pub async fn update_amberite_product_session(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    session: AmberiteNativeSession,
) -> crate::Result<()> {
    let mut bundle = load_product_session(pool).await?;
    bundle.version = PRODUCT_SESSION_VERSION;
    if bundle.signed_out {
        return Err(product_session_error("session is explicitly signed out"));
    }
    bundle.amberite = Some(session);
    save_product_session(pool, &bundle).await
}

pub async fn clear_product_session_preserving_identity(
    pool: &sqlx::Pool<sqlx::Sqlite>,
) -> crate::Result<()> {
    let mut bundle = load_product_session(pool).await?;
    bundle.version = PRODUCT_SESSION_VERSION;
    bundle.minecraft_credentials.clear();
    bundle.device_token = None;
    bundle.amberite = None;
    bundle.signed_out = true;
    let mut transaction = pool.begin().await?;
    write_product_session_row(&mut transaction, &bundle).await?;
    clear_legacy_secret_columns(&mut transaction).await?;
    transaction.commit().await?;
    Ok(())
}

pub async fn migrate_legacy_product_session(
    pool: &sqlx::Pool<sqlx::Sqlite>,
) -> crate::Result<()> {
    if product_session_row_exists(pool).await? {
        let _ = load_product_session(pool).await?;
        return Ok(());
    }

    let rows = sqlx::query(
        "SELECT uuid, active, username, access_token, refresh_token, expires FROM minecraft_users",
    )
    .fetch_all(pool)
    .await?;
    let mut bundle = ProductSessionBundle::default();
    let mut has_active_credential = false;
    for row in rows {
        let access_token: String = row.try_get("access_token")?;
        let refresh_token: String = row.try_get("refresh_token")?;
        if access_token.is_empty() || refresh_token.is_empty() {
            continue;
        }
        let uuid = row.try_get::<String, _>("uuid")?;
        let Ok(uuid) = Uuid::parse_str(&uuid) else {
            tracing::warn!(
                "Skipping legacy Minecraft credentials with an invalid UUID"
            );
            continue;
        };
        let requested_active = row.try_get::<i64, _>("active")? == 1;
        let active = requested_active && !has_active_credential;
        has_active_credential |= active;
        bundle
            .minecraft_credentials
            .push(StoredMinecraftCredential {
                profile: MinecraftProfile {
                    id: uuid,
                    name: row.try_get("username")?,
                    ..MinecraftProfile::default()
                },
                access_token,
                refresh_token,
                expires: Utc
                    .timestamp_opt(row.try_get("expires")?, 0)
                    .single()
                    .unwrap_or_else(Utc::now),
                active,
            });
    }

    if let Some(row) = sqlx::query(
        "SELECT uuid, private_key, x, y, issue_instant, not_after, token, display_claims FROM minecraft_device_tokens LIMIT 1",
    )
    .fetch_optional(pool)
    .await?
    {
        let private_key: String = row.try_get("private_key")?;
        let token: String = row.try_get("token")?;
        let x: String = row.try_get("x")?;
        let y: String = row.try_get("y")?;
        let key_id = Uuid::parse_str(row.try_get::<String, _>("uuid")?.as_str());
        if !private_key.is_empty()
            && !token.is_empty()
            && !x.is_empty()
            && !y.is_empty()
            && let Ok(key_id) = key_id
        {
            bundle.device_token = Some(StoredDeviceTokenPair {
                token: DeviceToken {
                    issue_instant: Utc
                        .timestamp_opt(row.try_get("issue_instant")?, 0)
                        .single()
                        .unwrap_or_else(Utc::now),
                    not_after: Utc
                        .timestamp_opt(row.try_get("not_after")?, 0)
                        .single()
                        .unwrap_or_else(Utc::now),
                    token,
                    display_claims: serde_json::from_str(
                        &row.try_get::<String, _>("display_claims")?,
                    )
                    .unwrap_or_default(),
                },
                key_id,
                private_key,
                x,
                y,
            });
        }
    }

    bundle.version = PRODUCT_SESSION_VERSION;
    bundle.signed_out = true;
    validate_product_session(&bundle)?;
    let mut transaction = pool.begin().await?;
    write_product_session_row(&mut transaction, &bundle).await?;
    clear_legacy_secret_columns(&mut transaction).await?;
    transaction.commit().await?;
    Ok(())
}

/// An entry in the player profile cache, keyed by player UUID.
pub(super) enum ProfileCacheEntry {
    /// A cached profile that is valid, even though it may be stale.
    Hit(Arc<MinecraftProfile>),
    /// A negative profile fetch result due to an authentication error,
    /// from which we're recovering by holding off from repeatedly
    /// attempting to fetch the profile until the token is refreshed
    /// or some time has passed.
    AuthErrorBackoff {
        likely_expired_token: String,
        last_attempt: Instant,
    },
}

/// A thread-safe cache of online profiles, used to avoid fetching the
/// same profile multiple times as long as they don't get too stale.
///
/// The cache has to be static because credential objects are short lived
/// and disposable, and in the future several threads may be interested in
/// profile data.
pub(super) static PROFILE_CACHE: Mutex<
    HashMap<Uuid, ProfileCacheEntry, BuildHasherDefault<DefaultHasher>>,
> = Mutex::const_new(HashMap::with_hasher(BuildHasherDefault::new()));

const ONLINE_PROFILE_CACHE_MAX_AGE: std::time::Duration =
    std::time::Duration::from_secs(60);
const ONLINE_PROFILE_LIVE_STATE_MAX_AGE: std::time::Duration =
    std::time::Duration::from_secs(5);
const ONLINE_PROFILE_AUTH_ERROR_BACKOFF: std::time::Duration =
    std::time::Duration::from_secs(60);

#[derive(Debug, Clone, Copy)]
enum OnlineProfileCacheIntent {
    NormalRead,
    LiveStateRead,
    RefreshFromMojang,
}

impl OnlineProfileCacheIntent {
    fn max_age(self) -> std::time::Duration {
        match self {
            Self::NormalRead => ONLINE_PROFILE_CACHE_MAX_AGE,
            Self::LiveStateRead => ONLINE_PROFILE_LIVE_STATE_MAX_AGE,
            Self::RefreshFromMojang => std::time::Duration::ZERO,
        }
    }

    fn can_use_stale_on_fetch_error(self) -> bool {
        matches!(self, Self::LiveStateRead)
    }
}

impl Credentials {
    pub async fn account_summary(&self) -> MinecraftAccountSummary {
        MinecraftAccountSummary {
            profile: self.maybe_online_profile().await.deref().clone(),
            expires: self.expires,
            active: self.active,
        }
    }

    /// Refreshes the authentication tokens for this user if they are expired, or
    /// very close to expiration.
    async fn refresh(
        &mut self,
        exec: &sqlx::Pool<sqlx::Sqlite>,
    ) -> crate::Result<()> {
        // Use a margin of 5 minutes to give e.g. Minecraft and potentially
        // other operations that depend on a fresh token 5 minutes to complete
        // from now, and deal with some classes of clock skew
        if self.expires > Utc::now() + Duration::minutes(5) {
            return Ok(());
        }

        let oauth_token = oauth_refresh(&self.refresh_token).await?;
        let (pair, current_date) =
            DeviceTokenPair::refresh_and_get_device_token(
                oauth_token.date,
                exec,
                true,
            )
            .await?;

        let sisu_authorize = sisu_authorize(
            None,
            &oauth_token.value.access_token,
            &pair.token.token,
            &pair.key,
            current_date,
        )
        .await?;

        let xbox_token = xsts_authorize(
            sisu_authorize.value,
            &pair.token.token,
            &pair.key,
            sisu_authorize.date,
        )
        .await?;

        let minecraft_token = minecraft_token(xbox_token.value).await?;

        self.access_token = minecraft_token.access_token;
        self.refresh_token = oauth_token.value.refresh_token;
        self.expires = oauth_token.date
            + Duration::seconds(oauth_token.value.expires_in as i64);

        self.upsert(exec).await?;

        Ok(())
    }

    /// Returns online profile data when the cached copy is still recent enough.
    #[tracing::instrument(skip(self))]
    pub async fn online_profile(&self) -> Option<Arc<MinecraftProfile>> {
        self.online_profile_with_cache_intent(
            OnlineProfileCacheIntent::NormalRead,
        )
        .await
    }

    /// Returns profile data recent enough for skin and cape state.
    ///
    /// Reuses a profile read from the last few seconds so opening the skins page
    /// does not send several identical Mojang requests.
    #[tracing::instrument(skip(self))]
    pub async fn online_profile_fresh(&self) -> Option<Arc<MinecraftProfile>> {
        self.online_profile_with_cache_intent(
            OnlineProfileCacheIntent::LiveStateRead,
        )
        .await
    }

    /// Fetches the online profile from Mojang after a skin or cape change.
    #[tracing::instrument(skip(self))]
    pub async fn refresh_online_profile(
        &self,
    ) -> Option<Arc<MinecraftProfile>> {
        self.online_profile_with_cache_intent(
            OnlineProfileCacheIntent::RefreshFromMojang,
        )
        .await
    }

    async fn online_profile_with_cache_intent(
        &self,
        cache_intent: OnlineProfileCacheIntent,
    ) -> Option<Arc<MinecraftProfile>> {
        let max_age = cache_intent.max_age();
        let stale_profile = {
            let mut profile_cache = PROFILE_CACHE.lock().await;
            let mut remove_cached_entry = false;

            let stale_profile = if let Some(cache_entry) =
                profile_cache.get(&self.offline_profile.id)
            {
                match cache_entry {
                    ProfileCacheEntry::Hit(profile)
                        if profile.is_fresh(max_age) =>
                    {
                        return Some(Arc::clone(profile));
                    }
                    ProfileCacheEntry::Hit(profile) => {
                        Some(Arc::clone(profile))
                    }
                    // Auth errors must be handled with a backoff strategy because it
                    // has been experimentally found that Mojang quickly rate limits
                    // the profile data endpoint on repeated attempts with bad auth
                    ProfileCacheEntry::AuthErrorBackoff {
                        likely_expired_token,
                        last_attempt,
                    } if &self.access_token != likely_expired_token
                        || Instant::now()
                            .saturating_duration_since(*last_attempt)
                            > ONLINE_PROFILE_AUTH_ERROR_BACKOFF =>
                    {
                        remove_cached_entry = true;
                        None
                    }
                    ProfileCacheEntry::AuthErrorBackoff { .. } => {
                        return None;
                    }
                }
            } else {
                None
            };

            if remove_cached_entry {
                profile_cache.remove(&self.offline_profile.id);
            }

            stale_profile
        };

        match minecraft_profile(&self.access_token).await {
            Ok(profile) => {
                let profile = Arc::new(profile);
                let cache_entry = ProfileCacheEntry::Hit(Arc::clone(&profile));

                let mut profile_cache = PROFILE_CACHE.lock().await;
                if self.offline_profile.id != profile.id {
                    profile_cache.remove(&self.offline_profile.id);
                }
                profile_cache.insert(profile.id, cache_entry);

                Some(profile)
            }
            Err(
                err @ MinecraftAuthenticationError::DeserializeResponse {
                    status_code: StatusCode::UNAUTHORIZED,
                    ..
                },
            ) => {
                tracing::warn!(
                    "Failed to fetch online profile for UUID {} likely due to stale credentials, backing off: {err}",
                    self.offline_profile.id
                );

                let mut profile_cache = PROFILE_CACHE.lock().await;
                profile_cache.insert(
                    self.offline_profile.id,
                    ProfileCacheEntry::AuthErrorBackoff {
                        likely_expired_token: self.access_token.clone(),
                        last_attempt: Instant::now(),
                    },
                );

                None
            }
            Err(err) => {
                tracing::warn!(
                    "Failed to fetch online profile for UUID {}: {err}",
                    self.offline_profile.id
                );

                if cache_intent.can_use_stale_on_fetch_error() {
                    stale_profile
                } else {
                    None
                }
            }
        }
    }

    /// Attempts to fetch the online profile for this user if possible, and if that fails
    /// falls back to the known offline profile data.
    ///
    /// See also the [`online_profile`](Self::online_profile) method.
    pub async fn maybe_online_profile(
        &self,
    ) -> MaybeOnlineMinecraftProfile<'_> {
        let online_profile = self.online_profile().await;
        online_profile.map_or_else(
            || MaybeOnlineMinecraftProfile::Offline(&self.offline_profile),
            MaybeOnlineMinecraftProfile::Online,
        )
    }

    /// Like [`get_active`](Self::get_active), but enforces credentials to be
    /// successfully refreshed unless the network is unreachable or times out.
    #[tracing::instrument]
    pub async fn get_default_credential(
        exec: &sqlx::Pool<sqlx::Sqlite>,
    ) -> crate::Result<Option<Credentials>> {
        let credentials = Self::get_active(exec).await?;

        if let Some(mut creds) = credentials {
            let res = creds.refresh(exec).await;

            match res {
                Ok(_) => Ok(Some(creds)),
                Err(err) => {
                    if let ErrorKind::MinecraftAuthenticationError(
                        MinecraftAuthenticationError::Request {
                            ref source,
                            ..
                        },
                    ) = *err.raw
                        && (source.is_connect() || source.is_timeout())
                    {
                        return Ok(Some(creds));
                    }

                    Err(err)
                }
            }
        } else {
            Ok(None)
        }
    }

    /// Fetches the currently selected credentials from secure product storage,
    /// attempting to refresh them if they are expired.
    pub async fn get_active(
        exec: &sqlx::Pool<sqlx::Sqlite>,
    ) -> crate::Result<Option<Self>> {
        let mut credentials = load_product_session(exec)
            .await?
            .minecraft_credentials
            .into_iter()
            .find(|value| value.active)
            .map(Self::from);
        if let Some(value) = credentials.as_mut() {
            value.refresh(exec).await.ok();
        }
        Ok(credentials)
    }

    pub async fn get_all(
        exec: &sqlx::Pool<sqlx::Sqlite>,
    ) -> crate::Result<DashMap<Uuid, Self>> {
        let result = DashMap::new();
        for stored in load_product_session(exec).await?.minecraft_credentials {
            let uuid = stored.profile.id;
            let mut credentials = Self::from(stored);
            credentials.refresh(exec).await.ok();
            result.insert(uuid, credentials);
        }
        Ok(result)
    }

    pub async fn upsert(
        &self,
        exec: &sqlx::Pool<sqlx::Sqlite>,
    ) -> crate::Result<()> {
        let profile = self.maybe_online_profile().await;
        let expires = self.expires.timestamp();
        let uuid = profile.id.as_hyphenated().to_string();
        let mut bundle = load_product_session(exec).await?;
        bundle.version = PRODUCT_SESSION_VERSION;
        merge_minecraft_credential(&mut bundle, self);

        let mut transaction = exec.begin().await?;
        write_product_session_row(&mut transaction, &bundle).await?;
        if self.active {
            sqlx::query("UPDATE minecraft_users SET active = FALSE")
                .execute(&mut *transaction)
                .await?;
        }
        sqlx::query(
            "INSERT INTO minecraft_users (uuid, active, username, access_token, refresh_token, expires) VALUES (?, ?, ?, '', '', ?) ON CONFLICT(uuid) DO UPDATE SET active = excluded.active, username = excluded.username, access_token = '', refresh_token = '', expires = excluded.expires",
        )
        .bind(uuid)
        .bind(self.active)
        .bind(&profile.name)
        .bind(expires)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn remove(
        uuid: Uuid,
        exec: &sqlx::Pool<sqlx::Sqlite>,
    ) -> crate::Result<()> {
        let mut bundle = load_product_session(exec).await?;
        bundle.version = PRODUCT_SESSION_VERSION;
        bundle
            .minecraft_credentials
            .retain(|value| value.profile.id != uuid);
        let mut transaction = exec.begin().await?;
        write_product_session_row(&mut transaction, &bundle).await?;
        sqlx::query("DELETE FROM minecraft_users WHERE uuid = ?")
            .bind(uuid.as_hyphenated().to_string())
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }
}

impl Serialize for Credentials {
    fn serialize<S: Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        // Opportunistically hydrate the profile with its online data if possible for frontend
        // consumption, transparently handling all the possible Tokio runtime states the current
        // thread may be in the most efficient way
        let profile = match Handle::try_current().ok() {
            Some(runtime)
                if runtime.runtime_flavor() == RuntimeFlavor::CurrentThread =>
            {
                runtime.block_on(self.maybe_online_profile())
            }
            Some(runtime) => task::block_in_place(|| {
                runtime.block_on(self.maybe_online_profile())
            }),
            None => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_or_else(
                    |_| {
                        MaybeOnlineMinecraftProfile::Offline(
                            &self.offline_profile,
                        )
                    },
                    |runtime| runtime.block_on(self.maybe_online_profile()),
                ),
        };

        let mut ser = serializer.serialize_struct("Credentials", 3)?;
        ser.serialize_field("profile", &*profile)?;
        ser.serialize_field("expires", &self.expires)?;
        ser.serialize_field("active", &self.active)?;
        ser.end()
    }
}

pub struct DeviceTokenPair {
    pub token: DeviceToken,
    pub key: DeviceTokenKey,
}

impl DeviceTokenPair {
    #[tracing::instrument(skip(exec))]
    async fn refresh_and_get_device_token(
        current_date: DateTime<Utc>,
        exec: &sqlx::Pool<sqlx::Sqlite>,
        persist: bool,
    ) -> crate::Result<(Self, DateTime<Utc>)> {
        let pair = Self::get(exec).await?;

        if let Some(mut pair) = pair {
            if pair.token.not_after > current_date {
                Ok((pair, current_date))
            } else {
                let res = device_token(&pair.key, current_date).await?;

                pair.token = res.value;
                if persist {
                    pair.upsert(exec).await?;
                }

                Ok((pair, res.date))
            }
        } else {
            let key = generate_key()?;
            let res = device_token(&key, current_date).await?;

            let pair = Self {
                key,
                token: res.value,
            };

            if persist {
                pair.upsert(exec).await?;
            }

            Ok((pair, res.date))
        }
    }

    async fn get(
        exec: &sqlx::Pool<sqlx::Sqlite>,
    ) -> crate::Result<Option<Self>> {
        let Some(stored) = load_product_session(exec).await?.device_token
        else {
            return Ok(None);
        };
        let private_key = SigningKey::from_pkcs8_pem(&stored.private_key)
            .map_err(product_session_error)?;
        Ok(Some(Self {
            token: stored.token,
            key: DeviceTokenKey {
                id: stored.key_id,
                key: private_key,
                x: stored.x,
                y: stored.y,
            },
        }))
    }

    pub async fn upsert(
        &self,
        exec: &sqlx::Pool<sqlx::Sqlite>,
    ) -> crate::Result<()> {
        let mut bundle = load_product_session(exec).await?;
        bundle.version = PRODUCT_SESSION_VERSION;
        bundle.device_token = Some(StoredDeviceTokenPair::try_from(self)?);

        let mut transaction = exec.begin().await?;
        write_product_session_row(&mut transaction, &bundle).await?;
        sqlx::query(
            "INSERT INTO minecraft_device_tokens (id, uuid, private_key, x, y, issue_instant, not_after, token, display_claims) VALUES (0, ?, '', ?, ?, ?, ?, '', json('{}')) ON CONFLICT(id) DO UPDATE SET uuid = excluded.uuid, private_key = '', x = excluded.x, y = excluded.y, issue_instant = excluded.issue_instant, not_after = excluded.not_after, token = '', display_claims = json('{}')",
        )
        .bind(self.key.id.as_hyphenated().to_string())
        .bind(&self.key.x)
        .bind(&self.key.y)
        .bind(self.token.issue_instant.timestamp())
        .bind(self.token.not_after.timestamp())
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }
}

const MICROSOFT_CLIENT_ID: &str = "9e9c8504-c7f8-4b04-93a9-41a729924249";
const AUTH_REPLY_URL: &str = "https://login.live.com/oauth20_desktop.srf";
const REQUESTED_SCOPE: &str = "service::user.auth.xboxlive.com::MBI_SSL";
pub const MINECRAFT_SERVICES_USER_AGENT: &str =
    "Modrinth App (support@modrinth.com; https://modrinth.com/app)";

pub struct RequestWithDate<T> {
    pub date: DateTime<Utc>,
    pub value: T,
}

// flow steps
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct DeviceToken {
    pub issue_instant: DateTime<Utc>,
    pub not_after: DateTime<Utc>,
    pub token: String,
    pub display_claims: HashMap<String, serde_json::Value>,
}

#[tracing::instrument(skip(key))]
pub async fn device_token(
    key: &DeviceTokenKey,
    current_date: DateTime<Utc>,
) -> Result<RequestWithDate<DeviceToken>, MinecraftAuthenticationError> {
    let res = send_signed_request(
        None,
        "https://device.auth.xboxlive.com/device/authenticate",
        "/device/authenticate",
        json!({
            "Properties": {
                "AuthMethod": "ProofOfPossession",
                "Id": format!("{{{}}}", key.id.to_string().to_uppercase()),
                "DeviceType": "Win32",
                "Version": "10.16.0",
                "ProofKey": {
                    "kty": "EC",
                    "x": key.x,
                    "y": key.y,
                    "crv": "P-256",
                    "alg": "ES256",
                    "use": "sig"
                }
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"

        }),
        key,
        MinecraftAuthStep::GetDeviceToken,
        current_date,
    )
    .await?;

    Ok(RequestWithDate {
        date: res.current_date,
        value: res.body,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RedirectUri {
    pub msa_oauth_redirect: String,
}

#[tracing::instrument(skip(key))]
async fn sisu_authenticate(
    token: &str,
    challenge: &str,
    key: &DeviceTokenKey,
    current_date: DateTime<Utc>,
    select_account: bool,
) -> Result<(String, RequestWithDate<RedirectUri>), MinecraftAuthenticationError>
{
    let prompt = if select_account {
        "select_account"
    } else {
        "login"
    };
    let res = send_signed_request::<RedirectUri>(
        None,
        "https://sisu.xboxlive.com/authenticate",
        "/authenticate",
        json!({
          "AppId": MICROSOFT_CLIENT_ID,
          "DeviceToken": token,
          "Offers": [
            REQUESTED_SCOPE
          ],
          "Query": {
            "code_challenge": challenge,
            "code_challenge_method": "S256",
            "state": generate_oauth_challenge(),
            "prompt": prompt
          },
          "RedirectUri": AUTH_REPLY_URL,
          "Sandbox": "RETAIL",
          "TokenType": "code",
          "TitleId": "1794566092",
        }),
        key,
        MinecraftAuthStep::SisuAuthenticate,
        current_date,
    )
    .await?;

    let session_id = res
        .headers
        .get("X-SessionId")
        .and_then(|x| x.to_str().ok())
        .ok_or_else(|| MinecraftAuthenticationError::NoSessionId)?
        .to_string();

    Ok((
        session_id,
        RequestWithDate {
            date: res.current_date,
            value: res.body,
        },
    ))
}

#[derive(Deserialize)]
struct OAuthToken {
    // pub token_type: String,
    pub expires_in: u64,
    // pub scope: String,
    pub access_token: String,
    pub refresh_token: String,
    // pub user_id: String,
    // pub foci: String,
}

#[tracing::instrument]
async fn oauth_token(
    code: &str,
    verifier: &str,
) -> Result<RequestWithDate<OAuthToken>, MinecraftAuthenticationError> {
    let mut query = HashMap::new();
    query.insert("client_id", MICROSOFT_CLIENT_ID);
    query.insert("code", code);
    query.insert("code_verifier", verifier);
    query.insert("grant_type", "authorization_code");
    query.insert("redirect_uri", AUTH_REPLY_URL);
    query.insert("scope", REQUESTED_SCOPE);

    let res = auth_retry(|| {
        INSECURE_REQWEST_CLIENT
            .post("https://login.live.com/oauth20_token.srf")
            .header("Accept", "application/json")
            .form(&query)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::GetOAuthToken,
    })?;

    let status = res.status();
    let current_date = get_date_header(res.headers());
    let text = res.text().await.map_err(|source| {
        MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::GetOAuthToken,
        }
    })?;

    let body = serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step: MinecraftAuthStep::GetOAuthToken,
            status_code: status,
        }
    })?;

    Ok(RequestWithDate {
        date: current_date,
        value: body,
    })
}

#[tracing::instrument]
async fn oauth_refresh(
    refresh_token: &str,
) -> Result<RequestWithDate<OAuthToken>, MinecraftAuthenticationError> {
    let mut query = HashMap::new();
    query.insert("client_id", MICROSOFT_CLIENT_ID);
    query.insert("refresh_token", refresh_token);
    query.insert("grant_type", "refresh_token");
    query.insert("redirect_uri", AUTH_REPLY_URL);
    query.insert("scope", REQUESTED_SCOPE);

    let res = auth_retry(|| {
        INSECURE_REQWEST_CLIENT
            .post("https://login.live.com/oauth20_token.srf")
            .header("Accept", "application/json")
            .form(&query)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::RefreshOAuthToken,
    })?;

    let status = res.status();
    let current_date = get_date_header(res.headers());
    let text = res.text().await.map_err(|source| {
        MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::RefreshOAuthToken,
        }
    })?;

    let body = serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step: MinecraftAuthStep::RefreshOAuthToken,
            status_code: status,
        }
    })?;

    Ok(RequestWithDate {
        date: current_date,
        value: body,
    })
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct SisuAuthorize {
    // pub authorization_token: DeviceToken,
    // pub device_token: String,
    // pub sandbox: String,
    pub title_token: DeviceToken,
    pub user_token: DeviceToken,
    // pub web_page: String,
}

#[tracing::instrument(skip(key))]
async fn sisu_authorize(
    session_id: Option<&str>,
    access_token: &str,
    device_token: &str,
    key: &DeviceTokenKey,
    current_date: DateTime<Utc>,
) -> Result<RequestWithDate<SisuAuthorize>, MinecraftAuthenticationError> {
    let res = send_signed_request(
        None,
        "https://sisu.xboxlive.com/authorize",
        "/authorize",
        json!({
            "AccessToken": format!("t={access_token}"),
            "AppId": MICROSOFT_CLIENT_ID,
            "DeviceToken": device_token,
            "ProofKey": {
                "kty": "EC",
                "x": key.x,
                "y": key.y,
                "crv": "P-256",
                "alg": "ES256",
                "use": "sig"
            },
            "Sandbox": "RETAIL",
            "SessionId": session_id,
            "SiteName": "user.auth.xboxlive.com",
            "RelyingParty": "http://xboxlive.com",
            "UseModernGamertag": true
        }),
        key,
        MinecraftAuthStep::SisuAuthorize,
        current_date,
    )
    .await?;

    Ok(RequestWithDate {
        date: res.current_date,
        value: res.body,
    })
}

#[tracing::instrument(skip(key))]
async fn xsts_authorize(
    authorize: SisuAuthorize,
    device_token: &str,
    key: &DeviceTokenKey,
    current_date: DateTime<Utc>,
) -> Result<RequestWithDate<DeviceToken>, MinecraftAuthenticationError> {
    let res = send_signed_request(
        None,
        "https://xsts.auth.xboxlive.com/xsts/authorize",
        "/xsts/authorize",
        json!({
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [authorize.user_token.token],
                "DeviceToken": device_token,
                "TitleToken": authorize.title_token.token,
            },
        }),
        key,
        MinecraftAuthStep::XstsAuthorize,
        current_date,
    )
    .await?;

    Ok(RequestWithDate {
        date: res.current_date,
        value: res.body,
    })
}

#[derive(Deserialize)]
struct MinecraftToken {
    // pub username: String,
    pub access_token: String,
    // pub token_type: String,
    // pub expires_in: u64,
}

#[tracing::instrument]
async fn minecraft_token(
    token: DeviceToken,
) -> Result<MinecraftToken, MinecraftAuthenticationError> {
    let uhs = token
        .display_claims
        .get("xui")
        .and_then(|x| x.get(0))
        .and_then(|x| x.get("uhs"))
        .and_then(|x| x.as_str().map(String::from))
        .ok_or_else(|| MinecraftAuthenticationError::NoUserHash)?;

    let token = token.token;

    let res = auth_retry(|| {
        INSECURE_REQWEST_CLIENT
            .post("https://api.minecraftservices.com/launcher/login")
            .header("Accept", "application/json")
            .header("User-Agent", MINECRAFT_SERVICES_USER_AGENT)
            .json(&json!({
                "platform": "PC_LAUNCHER",
                "xtoken": format!("XBL3.0 x={uhs};{token}"),
            }))
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::MinecraftToken,
    })?;

    let status = res.status();
    let text = res.text().await.map_err(|source| {
        MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::MinecraftToken,
        }
    })?;

    serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step: MinecraftAuthStep::MinecraftToken,
            status_code: status,
        }
    })
}

#[derive(
    sqlx::Type, Deserialize, Serialize, Debug, Copy, Clone, PartialEq, Eq,
)]
#[serde(rename_all = "UPPERCASE")]
#[sqlx(rename_all = "UPPERCASE")]
pub enum MinecraftSkinVariant {
    /// The classic player model, with arms that are 4 pixels wide.
    Classic,
    /// The slim player model, with arms that are 3 pixels wide.
    Slim,
    /// The player model is unknown.
    #[serde(other)]
    Unknown, // Defensive handling of unexpected Mojang API return values to
             // prevent breaking the entire profile parsing
}

#[derive(Deserialize, Serialize, Debug, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum MinecraftCharacterExpressionState {
    /// This expression is selected for being displayed ingame.
    ///
    /// At the moment, at most one expression can be selected at a time.
    Active,
    /// This expression is not selected for being displayed ingame.
    Inactive,
    /// The expression selection status is unknown.
    #[serde(other)]
    Unknown, // Defensive handling of unexpected Mojang API return values to
             // prevent breaking the entire profile parsing
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct MinecraftSkin {
    /// The UUID of this skin object.
    ///
    /// As of 2025-04-08, in the production Mojang profile endpoint this UUID
    /// changes every time the player changes their skin, even if the skin
    /// texture is the same as before.
    pub id: Uuid,
    /// The selection state of the skin.
    ///
    /// As of 2025-04-08, in the production Mojang profile endpoint this
    /// is always `ACTIVE`, as only a single skin representing the current
    /// skin is returned.
    pub state: MinecraftCharacterExpressionState,
    /// The URL to the skin texture.
    ///
    /// As of 2025-04-08, in the production Mojang profile endpoint the file
    /// name for this URL is a hash of the skin texture, so that different
    /// players using the same skin texture will share a texture URL.
    pub url: Arc<Url>,
    /// A hash of the skin texture.
    ///
    /// As of 2025-04-08, in the production Mojang profile endpoint this
    /// is always set and the same as the file name of the skin texture URL.
    #[serde(
        default, // Defensive handling of unexpected Mojang API return values to
                 // prevent breaking the entire profile parsing
        rename = "textureKey"
    )]
    pub texture_key: Option<Arc<str>>,
    /// The player model variant this skin is for.
    pub variant: MinecraftSkinVariant,
    /// User-friendly name for the skin.
    ///
    /// As of 2025-04-08, in the production Mojang profile endpoint this is
    /// only set if the player has not set a custom skin, and this skin object
    /// is therefore the default skin for the player's UUID.
    #[serde(
        default,
        rename = "alias",
        deserialize_with = "normalize_skin_alias_case"
    )]
    pub name: Option<String>,
}

impl MinecraftSkin {
    /// Robustly computes the texture key for this skin, falling back to its
    /// URL file name and finally to the skin UUID when necessary.
    pub fn texture_key(&self) -> Arc<str> {
        self.texture_key.as_ref().cloned().unwrap_or_else(|| {
            self.url
                .path_segments()
                .and_then(|mut path_segments| {
                    path_segments.next_back().map(String::from)
                })
                .unwrap_or_else(|| self.id.as_simple().to_string())
                .into()
        })
    }
}

fn normalize_skin_alias_case<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    // Skin aliases have been spotted to be returned in all caps, so make sure
    // they are normalized to a prettier title case
    Ok(<Option<Cow<'_, str>>>::deserialize(deserializer)?
        .map(|alias| alias.to_title_case()))
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct MinecraftCape {
    /// The UUID of the cape.
    pub id: Uuid,
    /// The selection state of the cape.
    pub state: MinecraftCharacterExpressionState,
    /// The URL to the cape texture.
    pub url: Arc<Url>,
    /// The user-friendly name for the cape.
    #[serde(rename = "alias")]
    pub name: Arc<str>,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct MinecraftProfile {
    /// The UUID of the player.
    #[serde(default)]
    pub id: Uuid,
    /// The username of the player.
    pub name: String,
    /// The skins the player is known to have.
    ///
    /// As of 2025-04-08, in the production Mojang profile endpoint every
    /// player has a single skin.
    pub skins: Vec<MinecraftSkin>,
    /// The capes the player is known to have.
    pub capes: Vec<MinecraftCape>,
    /// The instant when the profile was fetched. See also [Self::is_fresh].
    #[serde(skip)]
    pub fetch_time: Option<Instant>,
}

impl MinecraftProfile {
    /// Checks whether the profile data is fresh (i.e., highly likely to be
    /// up-to-date because it was fetched recently) or stale. If it is not
    /// known when this profile data has been fetched from Mojang servers (i.e.,
    /// `fetch_time` is `None`), the profile is considered stale.
    ///
    /// This can be used to determine if the profile data should be fetched again
    /// from the Mojang API: the vanilla launcher was seen refreshing profile
    /// data every 60 seconds when re-entering the skin selection screen, and
    /// external applications may change this data at any time.
    fn is_fresh(&self, max_age: std::time::Duration) -> bool {
        self.fetch_time.is_some_and(|last_profile_fetch_time| {
            Instant::now().saturating_duration_since(last_profile_fetch_time)
                < max_age
        })
    }

    /// Returns the currently selected skin for this profile.
    pub fn current_skin(&self) -> crate::Result<&MinecraftSkin> {
        Ok(self
            .skins
            .iter()
            .find(|skin| {
                skin.state == MinecraftCharacterExpressionState::Active
            })
            // There should always be one active skin, even when the player uses their default skin
            .ok_or_else(|| {
                ErrorKind::OtherError("No active skin found".into())
            })?)
    }

    /// Returns the currently selected cape for this profile.
    pub fn current_cape(&self) -> Option<&MinecraftCape> {
        self.capes.iter().find(|cape| {
            cape.state == MinecraftCharacterExpressionState::Active
        })
    }
}

pub enum MaybeOnlineMinecraftProfile<'profile> {
    /// An online profile, fetched from the Mojang API.
    Online(Arc<MinecraftProfile>),
    /// An offline profile, which has not been fetched from the Mojang API.
    Offline(&'profile MinecraftProfile),
}

impl Deref for MaybeOnlineMinecraftProfile<'_> {
    type Target = MinecraftProfile;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Online(profile) => profile,
            Self::Offline(profile) => profile,
        }
    }
}

#[tracing::instrument(skip(token))]
async fn minecraft_profile(
    token: &str,
) -> Result<MinecraftProfile, MinecraftAuthenticationError> {
    let res = auth_retry(|| {
        INSECURE_REQWEST_CLIENT
            .get("https://api.minecraftservices.com/minecraft/profile")
            .header("Accept", "application/json")
            .header("User-Agent", MINECRAFT_SERVICES_USER_AGENT)
            .bearer_auth(token)
            // Profiles may be refreshed periodically in response to user actions,
            // so we want each refresh to be fast
            .timeout(std::time::Duration::from_secs(10))
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::MinecraftProfile,
    })?;

    let status = res.status();
    let text = res.text().await.map_err(|source| {
        MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::MinecraftProfile,
        }
    })?;

    let mut profile =
        serde_json::from_str::<MinecraftProfile>(&text).map_err(|source| {
            MinecraftAuthenticationError::DeserializeResponse {
                source,
                raw: text,
                step: MinecraftAuthStep::MinecraftProfile,
                status_code: status,
            }
        })?;
    profile.fetch_time = Some(Instant::now());

    tracing::debug!(
        "Successfully fetched Minecraft profile for {}",
        profile.name
    );

    Ok(profile)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MinecraftEntitlements {}

#[tracing::instrument]
async fn minecraft_entitlements(
    token: &str,
) -> Result<MinecraftEntitlements, MinecraftAuthenticationError> {
    let res = auth_retry(|| {
		INSECURE_REQWEST_CLIENT
			.get(format!("https://api.minecraftservices.com/entitlements/license?requestId={}", Uuid::new_v4()))
			.header("Accept", "application/json")
			.header("User-Agent", MINECRAFT_SERVICES_USER_AGENT)
			.bearer_auth(token)
			.send()
	})
    .await.map_err(|source| MinecraftAuthenticationError::Request { source, step: MinecraftAuthStep::MinecraftEntitlements })?;

    let status = res.status();
    let text = res.text().await.map_err(|source| {
        MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::MinecraftEntitlements,
        }
    })?;

    serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step: MinecraftAuthStep::MinecraftEntitlements,
            status_code: status,
        }
    })
}

// auth utils
#[tracing::instrument(skip(reqwest_request))]
async fn auth_retry<F>(
    reqwest_request: impl Fn() -> F,
) -> Result<reqwest::Response, reqwest::Error>
where
    F: Future<Output = Result<Response, reqwest::Error>>,
{
    const RETRY_COUNT: usize = 5; // Does command 9 times
    const RETRY_WAIT: std::time::Duration =
        std::time::Duration::from_millis(250);

    let mut resp = reqwest_request().await;
    for i in 0..RETRY_COUNT {
        match &resp {
            Ok(_) => {
                break;
            }
            Err(err) => {
                if err.is_connect() || err.is_timeout() {
                    if i < RETRY_COUNT - 1 {
                        tracing::debug!(
                            "Request failed with connect error, retrying...",
                        );
                        tokio::time::sleep(RETRY_WAIT).await;
                        resp = reqwest_request().await;
                    } else {
                        break;
                    }
                }
            }
        }
    }

    resp
}

pub struct DeviceTokenKey {
    pub id: Uuid,
    pub key: SigningKey,
    pub x: String,
    pub y: String,
}

#[tracing::instrument]
fn generate_key() -> Result<DeviceTokenKey, MinecraftAuthenticationError> {
    let uuid = Uuid::new_v4();

    let signing_key = SigningKey::random(&mut OsRng);
    let public_key = VerifyingKey::from(&signing_key);

    let encoded_point = public_key.to_encoded_point(false);

    Ok(DeviceTokenKey {
        id: uuid,
        key: signing_key,
        x: BASE64_URL_SAFE_NO_PAD.encode(
            encoded_point.x().ok_or_else(|| {
                MinecraftAuthenticationError::ReadingPublicKey
            })?,
        ),
        y: BASE64_URL_SAFE_NO_PAD.encode(
            encoded_point.y().ok_or_else(|| {
                MinecraftAuthenticationError::ReadingPublicKey
            })?,
        ),
    })
}

struct SignedRequestResponse<T> {
    pub headers: HeaderMap,
    pub current_date: DateTime<Utc>,
    pub body: T,
}

#[tracing::instrument(skip(key))]
async fn send_signed_request<T: DeserializeOwned>(
    authorization: Option<&str>,
    url: &str,
    url_path: &str,
    raw_body: serde_json::Value,
    key: &DeviceTokenKey,
    step: MinecraftAuthStep,
    current_date: DateTime<Utc>,
) -> Result<SignedRequestResponse<T>, MinecraftAuthenticationError> {
    let auth = authorization.map_or(Vec::new(), |v| v.as_bytes().to_vec());

    let body = serde_json::to_vec(&raw_body).map_err(|source| {
        MinecraftAuthenticationError::SerializeBody { source, step }
    })?;
    let time: u128 =
        { ((current_date.timestamp() as u128) + 11644473600) * 10000000 };

    let mut buffer = Vec::new();
    buffer.extend_from_slice(&1_u32.to_be_bytes()[..]);
    buffer.push(0_u8);
    buffer.extend_from_slice(&(time as u64).to_be_bytes()[..]);
    buffer.push(0_u8);
    buffer.extend_from_slice("POST".as_bytes());
    buffer.push(0_u8);
    buffer.extend_from_slice(url_path.as_bytes());
    buffer.push(0_u8);
    buffer.extend_from_slice(&auth);
    buffer.push(0_u8);
    buffer.extend_from_slice(&body);
    buffer.push(0_u8);

    let ecdsa_sig: Signature = key.key.sign(&buffer);

    let mut sig_buffer = Vec::new();
    sig_buffer.extend_from_slice(&1_i32.to_be_bytes()[..]);
    sig_buffer.extend_from_slice(&(time as u64).to_be_bytes()[..]);
    sig_buffer.extend_from_slice(&ecdsa_sig.r().to_bytes());
    sig_buffer.extend_from_slice(&ecdsa_sig.s().to_bytes());

    let signature = BASE64_STANDARD.encode(&sig_buffer);

    let res = auth_retry(|| {
        let mut request = INSECURE_REQWEST_CLIENT
            .post(url)
            .header("Content-Type", "application/json; charset=utf-8")
            .header("Accept", "application/json")
            .header("Signature", &signature);

        if url != "https://sisu.xboxlive.com/authorize" {
            request = request.header("x-xbl-contract-version", "1");
        }

        if let Some(auth) = authorization {
            request = request.header("Authorization", auth);
        }

        request.body(body.clone()).send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request { source, step })?;

    let status = res.status();
    let headers = res.headers().clone();

    let current_date = get_date_header(&headers);

    let body = res.text().await.map_err(|source| {
        MinecraftAuthenticationError::Request { source, step }
    })?;

    let body = serde_json::from_str(&body).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: body,
            step,
            status_code: status,
        }
    })?;
    Ok(SignedRequestResponse {
        headers,
        current_date,
        body,
    })
}

#[tracing::instrument]
fn get_date_header(headers: &HeaderMap) -> DateTime<Utc> {
    headers
        .get(reqwest::header::DATE)
        .and_then(|x| x.to_str().ok())
        .and_then(|x| DateTime::parse_from_rfc2822(x).ok())
        .map_or(Utc::now(), |x| x.with_timezone(&Utc))
}

#[tracing::instrument]
fn generate_oauth_challenge() -> String {
    let mut rng = rand::thread_rng();

    let bytes: Vec<u8> = (0..64).map(|_| rng.r#gen::<u8>()).collect();
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod product_session_tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .expect("product-session test lock")
    }

    async fn test_pool() -> sqlx::Pool<sqlx::Sqlite> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite");
        sqlx::query(
            "CREATE TABLE minecraft_users (uuid TEXT PRIMARY KEY NOT NULL, active INTEGER NOT NULL, username TEXT NOT NULL, access_token TEXT NOT NULL, refresh_token TEXT NOT NULL, expires INTEGER NOT NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE minecraft_device_tokens (id INTEGER PRIMARY KEY CHECK (id = 0), uuid TEXT NOT NULL, private_key TEXT NOT NULL, x TEXT NOT NULL, y TEXT NOT NULL, issue_instant INTEGER NOT NULL, not_after INTEGER NOT NULL, token TEXT NOT NULL, display_claims TEXT NOT NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE amberite_product_session (id INTEGER PRIMARY KEY CHECK (id = 0), version INTEGER NOT NULL, encrypted_bundle BLOB NULL, nonce BLOB NULL, remembered_identity TEXT NULL, signed_out INTEGER NOT NULL, updated_at INTEGER NOT NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    fn credentials(uuid: Uuid, name: &str, active: bool) -> Credentials {
        Credentials {
            offline_profile: MinecraftProfile {
                id: uuid,
                name: name.to_string(),
                ..MinecraftProfile::default()
            },
            access_token: format!("minecraft-access-{name}"),
            refresh_token: format!("minecraft-refresh-{name}"),
            expires: Utc::now() + Duration::hours(2),
            active,
        }
    }

    fn remembered(uuid: Uuid, name: &str) -> RememberedAmberiteIdentity {
        RememberedAmberiteIdentity {
            minecraft_uuid: uuid,
            verified_minecraft_handle: name.to_string(),
            display_name: name.to_string(),
            avatar_url: None,
            last_successful_sign_in: Utc::now(),
        }
    }

    fn amberite_session(uuid: Uuid, suffix: &str) -> AmberiteNativeSession {
        let updated_at = Utc::now();
        AmberiteNativeSession {
            access_token: format!("amberite-access-{suffix}"),
            refresh_token: format!("amberite-refresh-{suffix}"),
            user: json!({ "minecraftUuid": uuid.as_hyphenated().to_string() }),
            active_identity_uuid: uuid,
            expires_at: updated_at + Duration::hours(1),
            updated_at,
        }
    }

    fn staged(credentials: Credentials) -> StagedMinecraftLogin {
        StagedMinecraftLogin {
            credentials,
            device_token: StoredDeviceTokenPair {
                token: DeviceToken {
                    issue_instant: Utc::now(),
                    not_after: Utc::now() + Duration::hours(1),
                    token: "device-token".to_string(),
                    display_claims: HashMap::new(),
                },
                key_id: Uuid::new_v4(),
                private_key: "private-key".to_string(),
                x: "x".to_string(),
                y: "y".to_string(),
            },
        }
    }

    #[tokio::test]
    async fn migrates_plaintext_secrets_and_blanks_legacy_columns() {
        let _guard = test_guard();
        configure_product_session_account("test-migration");
        clear_product_session_key();
        let pool = test_pool().await;
        let uuid = Uuid::new_v4();
        sqlx::query("INSERT INTO minecraft_users VALUES (?, 1, 'Steve', 'legacy-access', 'legacy-refresh', ?)")
            .bind(uuid.as_hyphenated().to_string())
            .bind((Utc::now() + Duration::hours(1)).timestamp())
            .execute(&pool)
            .await
            .unwrap();

        migrate_legacy_product_session(&pool).await.unwrap();

        let row = sqlx::query("SELECT encrypted_bundle FROM amberite_product_session WHERE id = 0")
            .fetch_one(&pool)
            .await
            .unwrap();
        let ciphertext: Vec<u8> = row.get("encrypted_bundle");
        assert!(
            !ciphertext
                .windows(b"legacy-access".len())
                .any(|window| window == b"legacy-access")
        );
        let row = sqlx::query("SELECT access_token, refresh_token FROM minecraft_users WHERE uuid = ?")
            .bind(uuid.as_hyphenated().to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.get::<String, _>("access_token"), "");
        assert_eq!(row.get::<String, _>("refresh_token"), "");
        let bundle = load_product_session(&pool).await.unwrap();
        assert!(bundle.signed_out);
        assert_eq!(bundle.minecraft_credentials.len(), 1);
    }

    #[tokio::test]
    async fn corruption_clears_secrets_and_preserves_remembered_identity() {
        let _guard = test_guard();
        configure_product_session_account("test-corruption");
        clear_product_session_key();
        let pool = test_pool().await;
        let uuid = Uuid::new_v4();
        let mut bundle = ProductSessionBundle {
            remembered_identity: Some(remembered(uuid, "Alex")),
            ..ProductSessionBundle::default()
        };
        merge_minecraft_credential(
            &mut bundle,
            &credentials(uuid, "Alex", true),
        );
        save_product_session(&pool, &bundle).await.unwrap();
        sqlx::query(
            "UPDATE amberite_product_session SET encrypted_bundle = x'010203'",
        )
        .execute(&pool)
        .await
        .unwrap();

        let recovered = load_product_session(&pool).await.unwrap();

        assert!(recovered.signed_out);
        assert!(recovered.minecraft_credentials.is_empty());
        assert_eq!(recovered.remembered_identity.unwrap().minecraft_uuid, uuid);
        assert!(read_product_session_key().unwrap().is_none());
    }

    #[tokio::test]
    async fn expected_uuid_failure_leaves_previous_session_untouched() {
        let _guard = test_guard();
        configure_product_session_account("test-staged-rollback");
        clear_product_session_key();
        let pool = test_pool().await;
        let previous_uuid = Uuid::new_v4();
        let mut previous = ProductSessionBundle {
            amberite: Some(amberite_session(previous_uuid, "previous")),
            remembered_identity: Some(remembered(previous_uuid, "Previous")),
            signed_out: false,
            ..ProductSessionBundle::default()
        };
        merge_minecraft_credential(
            &mut previous,
            &credentials(previous_uuid, "Previous", true),
        );
        save_product_session(&pool, &previous).await.unwrap();

        let next_uuid = Uuid::new_v4();
        let result = commit_amberite_product_session(
            &pool,
            staged(credentials(next_uuid, "Next", true)),
            Some(previous_uuid),
            amberite_session(next_uuid, "next"),
            remembered(next_uuid, "Next"),
        )
        .await;

        assert!(result.is_err());
        let unchanged = load_product_session(&pool).await.unwrap();
        assert_eq!(
            unchanged.amberite.unwrap().access_token,
            "amberite-access-previous"
        );
        assert_eq!(
            unchanged.remembered_identity.unwrap().minecraft_uuid,
            previous_uuid
        );
    }

    #[tokio::test]
    async fn clear_is_atomic_and_preserves_identity_and_metadata() {
        let _guard = test_guard();
        configure_product_session_account("test-clear");
        clear_product_session_key();
        let pool = test_pool().await;
        let uuid = Uuid::new_v4();
        let mut bundle = ProductSessionBundle {
            amberite: Some(amberite_session(uuid, "active")),
            remembered_identity: Some(remembered(uuid, "Steve")),
            signed_out: false,
            ..ProductSessionBundle::default()
        };
        merge_minecraft_credential(
            &mut bundle,
            &credentials(uuid, "Steve", true),
        );
        save_product_session(&pool, &bundle).await.unwrap();
        sqlx::query("INSERT INTO minecraft_users VALUES (?, 1, 'Steve', 'plaintext-access', 'plaintext-refresh', ?)")
            .bind(uuid.as_hyphenated().to_string())
            .bind((Utc::now() + Duration::hours(1)).timestamp())
            .execute(&pool)
            .await
            .unwrap();

        clear_product_session_preserving_identity(&pool)
            .await
            .unwrap();

        let cleared = load_product_session(&pool).await.unwrap();
        assert!(cleared.signed_out);
        assert!(cleared.minecraft_credentials.is_empty());
        assert!(cleared.amberite.is_none());
        assert_eq!(cleared.remembered_identity.unwrap().minecraft_uuid, uuid);
        let row = sqlx::query("SELECT username, access_token, refresh_token FROM minecraft_users WHERE uuid = ?")
            .bind(uuid.as_hyphenated().to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.get::<String, _>("username"), "Steve");
        assert_eq!(row.get::<String, _>("access_token"), "");
        assert_eq!(row.get::<String, _>("refresh_token"), "");
    }

    #[test]
    fn account_selection_does_not_change_explicit_sign_out() {
        let mut bundle = ProductSessionBundle {
            remembered_identity: Some(remembered(Uuid::new_v4(), "Remembered")),
            ..ProductSessionBundle::default()
        };
        merge_minecraft_credential(
            &mut bundle,
            &credentials(Uuid::new_v4(), "Launcher", true),
        );

        assert!(bundle.signed_out);
        assert!(bundle.amberite.is_none());
        assert!(bundle.remembered_identity.is_some());
        validate_product_session(&bundle).unwrap();
    }

    #[test]
    fn account_summary_serialization_is_redacted() {
        let summary = MinecraftAccountSummary {
            profile: MinecraftProfile {
                id: Uuid::new_v4(),
                name: "Steve".to_string(),
                ..MinecraftProfile::default()
            },
            expires: Utc::now() + Duration::hours(1),
            active: true,
        };

        let serialized = serde_json::to_value(summary).unwrap();
        assert!(serialized.get("profile").is_some());
        assert!(serialized.get("expires").is_some());
        assert!(serialized.get("active").is_some());
        assert!(serialized.get("access_token").is_none());
        assert!(serialized.get("refresh_token").is_none());
        assert!(serialized.get("device_token").is_none());
        assert!(serialized.get("private_key").is_none());
    }
}
