//! A Core's own routes, besides the engine's. The owner's device signs in with a ticket from the
//! backend ([`SignIn`]) and sends the session it gets as its bearer; the local `copal` CLI is the
//! owner and sends neither. Every route but [`CheckHealth`] and [`SignIn`] answers the owner only.
//! A dangerous route also needs a fresh ticket in the `X-Amberite-Ticket` header.

mod access;
mod backups;
mod events;
mod hosting;
mod machine;
mod servers;

pub use access::*;
pub use backups::*;
pub use events::*;
pub use hosting::*;
pub use machine::*;
pub use servers::*;

use serde::{Deserialize, Serialize};

use crate::{Empty, Json, Stream, Upgrade};

/// The header a dangerous route's fresh ticket travels in.
pub const TICKET_HEADER: &str = "x-amberite-ticket";

/// A long op's start: the route answers 202 once it runs, and the socket's `OpEnded` with this id
/// says how it ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct OpStart {
    /// Made by the caller, unique among its running ops.
    pub op: String,
}

/// What `GET /health` answers, to anyone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct Health {
    /// The `PROTOCOL_VERSION` the Core speaks.
    pub protocol: u32,
    pub version: String,
    pub core_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ServerId {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SessionId {
    pub id: String,
}

crate::endpoints! {
    /// A Core's own routes.
    group core;

    /// Whether the Core is up, which Core it is, and the protocol it speaks.
    CheckHealth: Get "/health", None, (), (), Empty => Json<Health>;
    /// The events socket: the caller subscribes to topics with [`ClientMessage`] and gets the
    /// Core's events and the engine's for them.
    EventSocket: Get "/events", Session, (), (), Empty => Upgrade<CoreEvent, ClientMessage>;
    /// Trade a ticket for a session on this device. 10 a minute per address.
    SignIn: Post "/sessions", None, (), (), Json<NewSession> => Json<SignedIn>;
    /// Every signed-in device.
    ListSessions: Get "/sessions", Session, (), (), Empty => Json<Vec<CoreSession>>;
    /// End this device's session.
    SignOut: Delete "/sessions/current", Session, (), (), Empty => Empty;
    /// End another device's session.
    RevokeSession: Delete "/sessions/{id}", Session, SessionId, (), Empty => Empty, dangerous;
    /// The activity log, filtered and paged as the query asks.
    ListActivity: Get "/activity", Session, (), ActivityQuery, Empty => Json<ActionLogResponse>;
    /// Whether this Core is paired, and the code it shows while it is not.
    GetPairing: Get "/pairing", Session, (), (), Empty => Json<PairingStatus>;
    /// Drop the code shown and get another, within 20 seconds, while unpaired.
    RequestPairingCode: Post "/pairing/code", Session, (), (), Empty => Json<PairingStatus>;
    /// The owner is removing this Core from their account: every session ends and the Core shows
    /// a new pairing code.
    Unpair: Delete "/pairing", Session, (), (), Empty => Empty, dangerous;
    /// The running version, the newest release and the update settings.
    GetUpdate: Get "/update", Session, (), (), Empty => Json<UpdateStatus>;
    /// Turning auto-update on or changing the channel needs the ticket.
    ChangeUpdateSettings: Patch "/update", Session, (), (), Json<UpdateSettings> => Json<UpdateStatus>, dangerous;
    /// Look up the newest release now.
    CheckForUpdate: Post "/update/check", Session, (), (), Empty => Json<UpdateStatus>;
    /// The Core restarts into the new version right after answering. Its shape never changes, so
    /// an App can update a Core that speaks an older protocol.
    InstallUpdate: Post "/update/apply", Session, (), (), Json<ApplyUpdate> => Json<Restarting>, dangerous, untimed;
    /// Back to the previous version, with the database from before the update.
    RollBack: Post "/update/rollback", Session, (), (), Empty => Json<Restarting>, dangerous;
    /// What this Core gives to other people's shares, and what it holds.
    GetStorage: Get "/storage", Session, (), (), Empty => Json<StorageStatus>;
    /// Change what is given; the rest stays.
    ChangeStorage: Patch "/storage", Session, (), (), Json<StorageSettings> => Json<StorageStatus>;
    /// Whether the owner accepted the Minecraft EULA.
    GetEula: Get "/eula", Session, (), (), Empty => Json<Eula>;
    /// Accept the Minecraft EULA for every server on this Core.
    AcceptEula: Post "/eula", Session, (), (), Empty => Json<Eula>;
    /// The end of the Core's own log.
    GetLogs: Get "/logs", Session, (), LogsQuery, Empty => Json<CoreLogs>;
    /// The machine the Core runs on.
    GetMachine: Get "/machine", Session, (), (), Empty => Json<Machine>;
    /// Every server on this Core.
    ListServers: Get "/servers", Session, (), (), Empty => Json<Vec<Server>>;
    /// An empty server, to install a loader or pack on.
    CreateServer: Post "/servers", Session, (), (), Json<NewServer> => Json<ServerId>;
    /// One server.
    GetServer: Get "/servers/{id}", Session, ServerId, (), Empty => Json<Server>;
    /// Rename a server or change its update channel.
    ChangeServer: Patch "/servers/{id}", Session, ServerId, (), Json<ServerUpdate> => Empty;
    /// A stopped server, with its world, files and backups.
    DeleteServer: Delete "/servers/{id}", Session, ServerId, (), Empty => Empty, dangerous;
    /// A long op: start ends once Java runs, stop and kill once the server has the request.
    ChangePower: Post "/servers/{id}/power", Session, ServerId, (), Json<PowerChange> => Empty;
    /// A long op: install a loader on a stopped server.
    InstallLoader: Post "/servers/{id}/reinstall", Session, ServerId, (), Json<LoaderInstall> => Empty;
    /// Install an `.mrpack`, the body, over a stopped server. It goes on in the background.
    InstallModpackFile: Post "/servers/{id}/modpack-file", Session, ServerId, (), Stream => Empty;
    /// Push a version of the server's own share, which the first push creates.
    ShareServer: Post "/servers/{id}/share", Session, ServerId, (), Json<SharePush> => Json<SharedServer>, untimed;
    /// Apply the latest version of an App instance's share, which the server follows from then
    /// on and its own share goes. A stopped server installs it now; a running one when it next
    /// stops.
    FollowInstance: Post "/servers/{id}/follow", Session, ServerId, (), Json<InstanceFollow> => Empty, untimed;
    /// How the server starts: its Java and its command.
    GetStartup: Get "/servers/{id}/startup", Session, ServerId, (), Empty => Json<RuntimeOptions>;
    /// The managed Java the server runs, from its next start.
    ChangeStartup: Patch "/servers/{id}/startup", Session, ServerId, (), Json<PatchRuntimeOptions> => Empty;
    /// The server's `server.properties`.
    GetProperties: Get "/servers/{id}/properties", Session, ServerId, (), Empty => Json<PropertiesFields>;
    /// Answers with the file as it is now.
    ChangeProperties: Patch "/servers/{id}/properties", Session, ServerId, (), Json<PatchPropertiesFields> => Json<PropertiesFields>;
    /// The server's backups, their options, and the operations queued on them.
    ListBackups: Get "/servers/{id}/backups", Session, ServerId, (), Empty => Json<BackupsQueue>;
    /// Back up the server as it is, hot when it runs, into a free manual slot.
    CreateBackup: Post "/servers/{id}/backups", Session, ServerId, (), Json<BackupRequest> => Json<PostBackupResponse>;
    /// Deleting the excess scheduled backups (`ar`) needs the ticket.
    ChangeBackupOptions: Patch "/servers/{id}/backups/options", Session, ServerId, RotationAck, Json<PatchBackupOptions> => Json<PatchBackupOptionsResponse>;
    /// Delete several backups.
    DeleteBackups: Post "/servers/{id}/backups/delete", Session, ServerId, (), Json<DeleteManyBackupRequest> => Empty, dangerous;
    /// Rename a backup.
    RenameBackup: Patch "/servers/{id}/backups/{backup_id}", Session, BackupRef, (), Json<PatchBackup> => Empty;
    /// Delete one backup.
    DeleteBackup: Delete "/servers/{id}/backups/{backup_id}", Session, BackupRef, (), Empty => Empty, dangerous;
    /// Queue a restore over the stopped server's world, after a backup of it.
    RestoreBackup: Post "/servers/{id}/backups/{backup_id}/restore", Session, BackupRef, (), Empty => Empty, dangerous;
    /// Queue a failed backup again.
    RetryBackup: Post "/servers/{id}/backups/{backup_id}/retry", Session, BackupRef, (), Empty => Empty;
    /// Stop showing an operation that ended.
    AckOperation: Post "/servers/{id}/backups/operations/{operation_id}/ack", Session, OperationRef, (), Empty => Empty;
    /// Drop a queued operation; a cancelled backup goes with it.
    CancelOperation: Post "/servers/{id}/backups/operations/{operation_id}/cancel", Session, OperationRef, (), Empty => Empty;
    /// The server's memory, JVM flags and restart settings.
    GetPerformance: Get "/servers/{id}/performance", Session, ServerId, (), Empty => Json<Performance>;
    /// Used from the server's next start.
    SetMemory: Put "/servers/{id}/memory", Session, ServerId, (), Json<Memory> => Json<Performance>;
    /// Used from the server's next start.
    SetFlags: Put "/servers/{id}/flags", Session, ServerId, (), Json<JvmFlags> => Json<Performance>;
    /// When the server restarts on its own.
    SetRestarts: Put "/servers/{id}/restarts", Session, ServerId, (), Json<Restarts> => Json<Performance>;
    /// Send one command to the server's console, and learn why one was refused.
    SendCommand: Post "/servers/{id}/console", Session, ServerId, (), Json<ConsoleCommand> => Empty;
    /// A custom Java for the server, or the Core's own.
    SetJava: Put "/servers/{id}/java", Session, ServerId, (), Json<JavaPath> => Empty;
    /// What the server is doing and why it last stopped.
    GetInsight: Get "/servers/{id}/insight", Session, ServerId, (), Empty => Json<ServerInsight>;
    /// A long op: check a running server's port again. `GetInsight` has the result.
    CheckReachability: Post "/servers/{id}/reachability", Session, ServerId, (), Json<OpStart> => Empty;
    /// The ids of the running long ops: one missing from it has ended, its `OpEnded` perhaps lost.
    ListOps: Get "/ops", Session, (), (), Empty => Json<Vec<String>>;

    /// A backup as a zip, streamed.
    DownloadBackup: Get "/servers/{id}/backups/{backup_id}/download", Session, BackupRef, (), Empty => Stream;
    /// The server's files as a zip, streamed.
    DownloadWorld: Get "/servers/{id}/download", Session, ServerId, (), Empty => Stream;
    /// The server's content as an `.mrpack`: its content folders and `config/`.
    ExportModpack: Get "/servers/{id}/export.mrpack", Session, ServerId, (), Empty => Stream;
}

#[cfg(test)]
pub(crate) fn ts() -> String {
    use ts_rs::TS;

    let cfg = crate::ts::config();
    let declarations = [
        Health::decl(&cfg),
        ServerId::decl(&cfg),
        SessionId::decl(&cfg),
        OpStart::decl(&cfg),
    ]
    .into_iter()
    .chain(access::ts_types())
    .chain(events::ts_types())
    .chain(servers::ts_types())
    .chain(backups::ts_types())
    .chain(machine::ts_types())
    .chain(hosting::ts_types())
    .map(|declaration| format!("\nexport {declaration}\n"))
    .collect::<String>();
    format!(
        "\n/** The header a dangerous route's fresh ticket travels in. */\n\
         export const TICKET_HEADER = {TICKET_HEADER:?}\n{declarations}{}",
        ts_module().ts(false)
    )
}
