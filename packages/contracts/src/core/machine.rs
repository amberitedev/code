//! The Core itself: updates, storage for the pool, the EULA, its log, and the machine it runs on.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct UpdateStatus {
    /// The running version.
    pub version: String,
    /// Which release files this Core installs, such as `linux-x64`. Null on a platform Copal is
    /// not released for, which cannot update.
    pub target: Option<String>,
    pub channel: Channel,
    /// Updates install by themselves. Off: only when the owner asks.
    pub auto_update: bool,
    /// The newest release on the channel, as of the last check.
    pub latest: Option<String>,
    /// `latest` is newer than the running version.
    pub update_available: bool,
    pub checked_at: Option<DateTime<Utc>>,
    /// Why the last check failed.
    pub check_error: Option<String>,
    /// The version a rollback goes back to.
    pub previous: Option<String>,
    /// The last version that was tried here, failed its trial, and was rolled back from. While
    /// it is the newest release, `update_available` is false and applying it needs `retry`.
    pub failed: Option<String>,
    /// How `failed` failed. Null when an older launcher did not record it.
    pub failed_reason: Option<FailedTrial>,
}

/// A release channel. Each one also gets every release of the channels before it: beta gets
/// stable releases, alpha gets both.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
#[cfg_attr(
    feature = "sqlx",
    derive(sqlx::Type),
    sqlx(rename_all = "lowercase")
)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Stable,
    Beta,
    Alpha,
}

/// How a version on trial failed, as the launcher saw it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum FailedTrial {
    /// The OS would not run its binary.
    NotStarted,
    /// It ran, and exited before it answered a request.
    Exited,
    /// It ran, and did not answer a request within a minute.
    NoAnswer,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct UpdateSettings {
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub channel: Option<Channel>,
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub auto_update: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ApplyUpdate {
    /// Install the channel's release even when it is older. Only the copal CLI may.
    #[serde(default)]
    #[cfg_attr(feature = "export-ts", ts(as = "Option<bool>", optional))]
    pub allow_downgrade: bool,
    /// Install the channel's release even when it already failed its trial on this Core.
    #[serde(default)]
    #[cfg_attr(feature = "export-ts", ts(as = "Option<bool>", optional))]
    pub retry: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct Restarting {
    /// The version the Core restarts into.
    pub version: String,
}

/// The settings to change. Absent ones stay. Bytes throughout.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct StorageSettings {
    /// Offer storage to other people's shares. Off, the Core runs privately.
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub enabled: Option<bool>,
    /// The most the store may hold.
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub cap: Option<i64>,
    /// The most it may serve a day.
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub egress_cap: Option<i64>,
}

/// Bytes throughout. What the store holds is shown whether or not storage is offered: a Core
/// that does not offer it still holds its owner's shares.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct StorageStatus {
    /// Offers storage to other people's shares. Off, the Core runs privately.
    pub enabled: bool,
    pub cap: u64,
    pub egress_cap: u64,
    /// What the stored versions take, counting each object once.
    pub used: u64,
    /// Free on the store's disk.
    pub free: u64,
    /// Served today, UTC.
    pub egress_today: u64,
    /// The Core is installed as a service. The pool places shares only on Cores that are.
    pub service: bool,
    /// Versions the store holds.
    pub versions: u32,
}

/// The Minecraft EULA, accepted once by the owner for every server on the Core. No server starts
/// before that.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct Eula {
    /// The EULA's address.
    pub url: String,
    /// Null until the owner accepts it.
    pub accepted_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct LogsQuery {
    /// At most 1000. 100 when left out.
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub lines: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct CoreLogs {
    /// Oldest first.
    pub lines: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct Machine {
    /// This machine's memory. Null when the OS does not say.
    pub memory_mb: Option<u64>,
    /// This machine's CPU threads.
    pub cpus: u32,
    /// The memory every server together may use.
    pub assigned_mb: i64,
    /// Set when the servers together may use more than the machine has.
    pub warning: Option<String>,
    /// Whether the Core's store can be reached from the internet, on the Core's TCP port. The
    /// App reaches the API over Connect and does not need that port.
    pub store_reachability: Option<ReachabilityReport>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ReachabilityReport {
    pub state: Reach,
    /// The cause in one line.
    pub title: String,
    /// The one thing to do next, when there is something.
    pub next_step: Option<String>,
    /// Each test, in order.
    pub checks: Vec<ReachabilityCheck>,
    pub found: Vec<String>,
    /// Every step of the fix, `next_step` first.
    pub fix: Vec<String>,
    pub checked_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Reach {
    Reachable,
    /// Forwarded, but not tested from the internet.
    Unverified,
    Unreachable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ReachabilityCheck {
    /// Null when the test was not run or could not tell.
    pub passed: Option<bool>,
    pub text: String,
}

/// What the Core itself can tell `copal doctor`. The CLI adds what only an admin on the machine
/// can see: the service and the firewall.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct Doctor {
    /// The Core's port, which the firewall must let in: TCP and UDP.
    pub port: u16,
    /// Installed as a service. False for a Core someone runs by hand.
    pub installed: bool,
    pub checks: Vec<DoctorCheck>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct DoctorCheck {
    /// One lowercase word that names the check, such as `pairing`.
    pub name: String,
    pub state: CheckState,
    /// What was found, in one line.
    pub summary: String,
    /// The facts behind the summary.
    pub details: Vec<CheckDetail>,
    /// What to do about it, first step first. Empty when nothing is wrong.
    pub fix: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum CheckState {
    Ok,
    /// Works, but not as well as it could.
    Warn,
    Fail,
    /// Not run, or could not tell.
    Skip,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct CheckDetail {
    pub label: String,
    pub value: String,
}

/// What takes up space on the Core's disk. Bytes throughout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct DiskUsage {
    /// The size of the disk the data folder is on.
    pub total: u64,
    pub free: u64,
    pub servers: Vec<ServerUsage>,
    /// The shared files the store holds.
    pub shared: u64,
    /// Minecraft, loaders and Java, which the servers share.
    pub game_files: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ServerUsage {
    pub id: String,
    pub name: String,
    /// The server's folder: its world, mods and settings.
    pub files: u64,
    pub backups: u64,
    pub backup_count: u32,
}

#[cfg(test)]
pub(super) fn ts_types() -> Vec<String> {
    use ts_rs::TS;

    let cfg = crate::ts::config();
    vec![
        UpdateStatus::decl(&cfg),
        Channel::decl(&cfg),
        FailedTrial::decl(&cfg),
        UpdateSettings::decl(&cfg),
        ApplyUpdate::decl(&cfg),
        Restarting::decl(&cfg),
        StorageSettings::decl(&cfg),
        StorageStatus::decl(&cfg),
        Eula::decl(&cfg),
        LogsQuery::decl(&cfg),
        CoreLogs::decl(&cfg),
        Machine::decl(&cfg),
        ReachabilityReport::decl(&cfg),
        Reach::decl(&cfg),
        ReachabilityCheck::decl(&cfg),
        Doctor::decl(&cfg),
        DoctorCheck::decl(&cfg),
        CheckState::decl(&cfg),
        CheckDetail::decl(&cfg),
        DiskUsage::decl(&cfg),
        ServerUsage::decl(&cfg),
    ]
}
