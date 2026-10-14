//! How a server starts: `GET|POST /modrinth/v0/servers/{id}/startup` and
//! `GET|PATCH /v1/servers/{id}/worlds/{wid}/options/startup`. Mirrors
//! `Archon.Servers.v0.StartupConfig` (`archon/servers/v0.ts`) and the runtime options of
//! `Archon.Content.v1` (`archon/options/v1.ts`).

#![allow(dead_code)] // Wired in step 2: server routes.

use serde::{Deserialize, Serialize};

/// Mirrors the `jdk_version` union of Archon.Servers.v0.StartupConfig.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JdkVersion {
    Lts8,
    Lts11,
    Lts17,
    Lts21,
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
    #[serde(default)]
    pub jre_vendor: Option<JreVendor>,
    #[serde(default)]
    pub startup_command: Option<String>,
}
