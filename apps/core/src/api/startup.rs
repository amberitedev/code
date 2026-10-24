//! How a server starts: `GET|POST /modrinth/v0/servers/{id}/startup` and
//! `GET|PATCH /v1/servers/{id}/worlds/{wid}/options/startup`, and Amberite's
//! `PUT /servers/{id}/java`. Mirrors `Archon.Servers.v0.StartupConfig` (`archon/servers/v0.ts`)
//! and the runtime options of `Archon.Content.v1` (`archon/options/v1.ts`).
//!
//! The command line is reported, not edited: memory and flags are step 8. What can change is the
//! Java: a managed major other than the one the Minecraft version needs, or, as an advanced
//! override, a custom Java path. Both run as `copal-servers` like every server. Editors and the
//! owner only, since choosing what runs is running code.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{
    AppState,
    activity::Action,
    content::world,
    error::{ApiError, ErrorCode},
};
use crate::{
    access::{Caller, Level, activity},
    installs::java,
    servers::{ServerError, launch, records},
};

/// Mirrors the `jdk_version` union of Archon.Servers.v0.StartupConfig.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JdkVersion {
    Lts8,
    Lts11,
    Lts17,
    Lts21,
}

impl JdkVersion {
    /// The nearest of these to a Java major. Java 25 has no name here and shows as 21; the v1
    /// route has the exact number.
    fn from_major(major: u32) -> Self {
        match major {
            ..=8 => Self::Lts8,
            9..=11 => Self::Lts11,
            12..=17 => Self::Lts17,
            _ => Self::Lts21,
        }
    }

    fn major(self) -> u32 {
        match self {
            Self::Lts8 => 8,
            Self::Lts11 => 11,
            Self::Lts17 => 17,
            Self::Lts21 => 21,
        }
    }
}

/// Mirrors Archon.Content.v1.JreVendor, and the `jdk_build` union of
/// Archon.Servers.v0.StartupConfig, which takes the same values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JreVendor {
    Temurin,
    Corretto,
    Graal,
}

/// Response of GET `/modrinth/v0/servers/{id}/startup`. `invocation` is the command the server
/// runs with, `original_invocation` the default. Mirrors Archon.Servers.v0.StartupConfig.
#[derive(Debug, Clone, Serialize)]
pub struct StartupConfig {
    pub invocation: String,
    pub original_invocation: String,
    pub jdk_version: JdkVersion,
    pub jdk_build: JreVendor,
}

/// Body of POST `/modrinth/v0/servers/{id}/startup`. Null resets a field to its default. Mirrors
/// the `config` of `ArchonServersV0Module.updateStartupConfig`.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateStartupConfig {
    pub invocation: Option<String>,
    pub jdk_version: Option<JdkVersion>,
    #[allow(dead_code)] // Core runs Azul Zulu only.
    pub jdk_build: Option<JreVendor>,
}

/// Response of GET `/v1/servers/{id}/worlds/{wid}/options/startup`. Mirrors
/// Archon.Content.v1.RuntimeOptions.
#[derive(Debug, Clone, Serialize)]
pub struct RuntimeOptions {
    pub java_version: Option<u32>,
    pub jre_vendor: Option<JreVendor>,
    pub original_invocation: Option<String>,
    pub startup_command: Option<String>,
}

/// Body of PATCH `/v1/servers/{id}/worlds/{wid}/options/startup`. The App always sends every
/// field; null resets it to the default. Mirrors Archon.Content.v1.PatchRuntimeOptions.
#[derive(Debug, Clone, Deserialize)]
pub struct PatchRuntimeOptions {
    #[serde(default)]
    pub java_version: Option<u32>,
    #[allow(dead_code)] // As above.
    #[serde(default)]
    pub jre_vendor: Option<JreVendor>,
    #[serde(default)]
    pub startup_command: Option<String>,
}

/// Body of `PUT /servers/{id}/java`: a custom Java binary for this server, or `null` for the Java
/// Core manages. An advanced override; it runs as `copal-servers`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct JavaPath {
    pub path: Option<String>,
}

/// Java majors Core installs.
const MAJORS: [u32; 5] = [8, 11, 17, 21, 25];

/// What the startup routes report for a server: its command line and Java major.
async fn report(state: &AppState, id: &str) -> Result<(String, u32), ApiError> {
    let server = records::get(&state.db, id)
        .await?
        .ok_or_else(|| ServerError::NotFound(id.into()))?;
    let paths = state.servers.paths();
    let invocation = launch::args(paths, &server)
        .map(|args| launch::invocation(&args))
        .map_err(|error| {
            ApiError::new(ErrorCode::Conflict, format!("{error:#}"))
        })?;
    let major = launch::java_major(paths, &state.content.http, &server)
        .await
        .map_err(|error| {
            ApiError::new(ErrorCode::Internal, format!("{error:#}"))
        })?;
    Ok((invocation, major))
}

/// Choose the managed Java major, or `None` for the one the version needs.
async fn set_major(
    state: &AppState,
    caller: &Caller,
    id: &str,
    major: Option<u32>,
) -> Result<(), ApiError> {
    if let Some(major) = major
        && !MAJORS.contains(&major)
    {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            format!("Core installs Java {MAJORS:?}, not {major}."),
        ));
    }
    let server = records::get(&state.db, id)
        .await?
        .ok_or_else(|| ServerError::NotFound(id.into()))?;
    // The major the version needs is the default, not a choice: stored, it would pin the server
    // to it after a game version change.
    let needed = match &server.game_version {
        Some(game_version) => Some(
            java::java_requirement(
                &state.content.http,
                state.servers.paths().root(),
                game_version,
            )
            .await,
        ),
        None => None,
    };
    let chosen = major.filter(|major| Some(*major) != needed);
    records::set_java(
        &state.db,
        id,
        server.java_path.as_deref(),
        chosen.map(i64::from),
    )
    .await?;
    activity::log(&state.db, caller, Some(id), Action::SettingsChanged).await;
    Ok(())
}

/// A startup command other than the one Core builds is refused until memory and flags land.
fn custom_command(
    command: Option<&str>,
    current: &str,
) -> Result<(), ApiError> {
    match command {
        Some(command) if command.trim() != current => Err(ApiError::new(
            ErrorCode::InvalidRequest,
            "Core builds the startup command itself; memory and flags are set per server.",
        )),
        _ => Ok(()),
    }
}

/// `GET /modrinth/v0/servers/{id}/startup`.
pub async fn get_v0(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<StartupConfig>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    let (invocation, major) = report(&state, &id).await?;
    Ok(Json(StartupConfig {
        original_invocation: invocation.clone(),
        invocation,
        jdk_version: JdkVersion::from_major(major),
        // Core runs Azul Zulu, which this union has no name for.
        jdk_build: JreVendor::Temurin,
    }))
}

/// `POST /modrinth/v0/servers/{id}/startup`: choose the Java major; null resets it.
pub async fn update_v0(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateStartupConfig>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    let (invocation, major) = report(&state, &id).await?;
    custom_command(body.invocation.as_deref(), &invocation)?;
    // The App sends back what it read. That name is lossy (Java 25 reads as `lts21`), so the
    // same name means no change.
    if body.jdk_version != Some(JdkVersion::from_major(major)) {
        set_major(
            &state,
            &caller,
            &id,
            body.jdk_version.map(JdkVersion::major),
        )
        .await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /v1/servers/{id}/worlds/{wid}/options/startup`.
pub async fn get_v1(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> Result<Json<RuntimeOptions>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let (invocation, major) = report(&state, &id).await?;
    Ok(Json(RuntimeOptions {
        java_version: Some(major),
        jre_vendor: None,
        original_invocation: Some(invocation.clone()),
        startup_command: Some(invocation),
    }))
}

/// `PATCH /v1/servers/{id}/worlds/{wid}/options/startup`: the App sends every field; the startup
/// command must be the one Core builds, and a null Java version resets it.
pub async fn patch_v1(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(body): Json<PatchRuntimeOptions>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let (invocation, _) = report(&state, &id).await?;
    custom_command(body.startup_command.as_deref(), &invocation)?;
    set_major(&state, &caller, &id, body.java_version).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `PUT /servers/{id}/java`: a custom Java path, or null for Core's. It must be an absolute path
/// to a file.
pub async fn set_java_path(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<JavaPath>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    let server = records::get(&state.db, &id)
        .await?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    if let Some(path) = &body.path {
        let path = std::path::Path::new(path);
        if !path.is_absolute() || !path.is_file() {
            return Err(ApiError::new(
                ErrorCode::InvalidRequest,
                "A custom Java is an absolute path to its java binary on the Core's machine.",
            ));
        }
    }
    records::set_java(
        &state.db,
        &id,
        body.path.as_deref(),
        server.java_version,
    )
    .await?;
    activity::log(&state.db, &caller, Some(&id), Action::SettingsChanged).await;
    Ok(StatusCode::NO_CONTENT)
}
