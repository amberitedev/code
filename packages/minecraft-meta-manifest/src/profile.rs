use indexmap::IndexMap;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::minecraft::{Argument, ArgumentType, Library, VersionType};

/// Launch metadata combined with optional Forge-style installation metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileMetadata {
    pub id: String,
    pub inherits_from: String,
    #[serde(deserialize_with = "crate::util::deserialize_timestamp")]
    pub release_time: Timestamp,
    #[serde(deserialize_with = "crate::util::deserialize_timestamp")]
    pub time: Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minecraft_arguments: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<IndexMap<ArgumentType, Vec<Argument>>>,
    pub libraries: Vec<Library>,
    #[serde(rename = "type")]
    pub ty: VersionType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<IndexMap<String, SidedDataEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processors: Option<Vec<Processor>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidedDataEntry {
    pub client: String,
    pub server: String,
}

/// Installation step executed by the launcher after downloading libraries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Processor {
    pub jar: String,
    pub classpath: Vec<String>,
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<IndexMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sides: Option<Vec<String>>,
}

/// The latest version of the format the fabric model structs deserialize to
pub const CURRENT_FABRIC_FORMAT_VERSION: usize = 0;
/// The latest Forge launcher-meta format version.
pub const CURRENT_FORGE_FORMAT_VERSION: usize = 0;
/// The latest version of the format the quilt model structs deserialize to
pub const CURRENT_QUILT_FORMAT_VERSION: usize = 1;
/// The latest version of the format the neoforge model structs deserialize to
pub const CURRENT_NEOFORGE_FORMAT_VERSION: usize = 0;

/// The dummy replace string library names, inheritsFrom, and version names should be replaced with
pub const DUMMY_REPLACE_STRING: &str = "${modrinth.gameVersion}";

/// What a Forge or NeoForge server needs beyond its launcher-meta profile, which only describes a
/// client. Amberite's `server-meta` generator publishes one beside each loader version, at the same path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerProfile {
    /// The Java arguments that start the server, after the host's own JVM flags. Holds the
    /// `SERVER_*` placeholders below.
    pub arguments: Vec<String>,
    /// Libraries the server starts with that the profile does not list.
    pub libraries: Vec<Library>,
    /// The server's install steps, in place of the profile's. None reads the installer, and what
    /// the installer's steps extract into the server folder's `libraries/` goes to `{LIBRARY_DIR}`.
    pub processors: Vec<Processor>,
}

/// In [`ServerProfile::arguments`]: the shared libraries folder.
pub const SERVER_LIBRARY_DIRECTORY: &str = "${library_directory}";
/// In [`ServerProfile::arguments`]: the platform's classpath separator.
pub const SERVER_CLASSPATH_SEPARATOR: &str = "${classpath_separator}";
/// In [`ServerProfile::arguments`]: the vanilla server jar.
pub const SERVER_JAR: &str = "${server_jar}";
