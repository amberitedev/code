//! A Core's own routes, besides the engine's. The owner's device signs in with a ticket from the
//! backend ([`SignIn`]) and sends the session it gets as its bearer; the local `copal` CLI is the
//! owner and sends neither. Every route but [`CheckHealth`] and [`SignIn`] answers the owner only.
//! A dangerous route also needs a fresh ticket in the `X-Amberite-Ticket` header.

mod access;
mod events;
mod hosting;
mod machine;
mod servers;

pub use access::*;
pub use events::*;
pub use hosting::*;
pub use machine::*;
pub use servers::*;

use serde::{Deserialize, Serialize};

use crate::{Auth, Empty, Endpoint, Json, Method, Stream};

/// The header a dangerous route's fresh ticket travels in.
pub const TICKET_HEADER: &str = "x-amberite-ticket";

/// What `GET /health` answers, to anyone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Health {
    /// The `PROTOCOL_VERSION` the Core speaks.
    pub protocol: u32,
    pub version: String,
    pub core_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ServerId {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SessionId {
    pub id: String,
}

/// Declares each route as a unit struct and, for the tests, the TypeScript object listing them.
macro_rules! endpoints {
    (@dangerous) => { false };
    (@dangerous dangerous) => { true };
    ($(
        $(#[doc = $doc:literal])+
        $name:ident: $method:ident $path:literal, $auth:ident, $params:ty, $query:ty, $body:ty, $response:ty $(, $dangerous:ident)?;
    )+) => {
        $(
            $(#[doc = $doc])+
            pub struct $name;

            impl Endpoint for $name {
                const METHOD: Method = Method::$method;
                const PATH: &'static str = $path;
                const AUTH: Auth = Auth::$auth;
                const DANGEROUS: bool = endpoints!(@dangerous $($dangerous)?);
                type Params = $params;
                type Query = $query;
                type Body = $body;
                type Response = $response;
            }
        )+

        #[cfg(test)]
        fn endpoints_ts() -> String {
            [$((stringify!($name), crate::ts::endpoint::<$name>())),+]
                .map(|(name, endpoint)| format!("\t{name}: {endpoint},\n"))
                .concat()
        }
    };
}

endpoints! {
    /// Whether the Core is up, which Core it is, and the protocol it speaks.
    CheckHealth: Get "/health", None, (), (), Empty, Json<Health>;

    /// Trade a ticket for a session on this device. 10 a minute per address.
    SignIn: Post "/sessions", None, (), (), Json<NewSession>, Json<SignedIn>;
    /// Every signed-in device.
    ListSessions: Get "/sessions", Session, (), (), Empty, Json<Vec<CoreSession>>;
    /// End this device's session.
    SignOut: Delete "/sessions/current", Session, (), (), Empty, Empty;
    /// End another device's session.
    RevokeSession: Delete "/sessions/{id}", Session, SessionId, (), Empty, Empty, dangerous;

    /// A link for a download that cannot send the session, such as a native save.
    CreateDownloadLink: Post "/links", Session, (), (), Json<NewDownloadLink>, Json<DownloadLink>;

    /// The activity log, newest first.
    ListActivity: Get "/activity", Session, (), ActivityQuery, Empty, Json<ActivityPage>;

    /// Whether this Core is paired, and the code it shows while it is not.
    GetPairing: Get "/pairing", Session, (), (), Empty, Json<PairingStatus>;
    /// Drop the code shown and get another, within 20 seconds, while unpaired.
    RequestPairingCode: Post "/pairing/code", Session, (), (), Empty, Json<PairingStatus>;
    /// The owner is removing this Core from their account: every session ends and the Core shows
    /// a new pairing code.
    Unpair: Delete "/pairing", Session, (), (), Empty, Empty, dangerous;

    /// The running version, the newest release and the update settings.
    GetUpdate: Get "/update", Session, (), (), Empty, Json<UpdateStatus>;
    /// Turning auto-update on or changing the channel needs the ticket.
    ChangeUpdateSettings: Patch "/update", Session, (), (), Json<UpdateSettings>, Json<UpdateStatus>, dangerous;
    /// Look up the newest release now.
    CheckForUpdate: Post "/update/check", Session, (), (), Empty, Json<UpdateStatus>;
    /// The Core restarts into the new version right after answering. Its shape never changes, so
    /// an App can update a Core that speaks an older protocol.
    InstallUpdate: Post "/update/apply", Session, (), (), Json<ApplyUpdate>, Json<Restarting>, dangerous;
    /// Back to the previous version, with the database from before the update.
    RollBack: Post "/update/rollback", Session, (), (), Empty, Json<Restarting>, dangerous;

    /// What this Core gives to other people's shares, and what it holds.
    GetStorage: Get "/storage", Session, (), (), Empty, Json<StorageStatus>;
    /// Change what is given; the rest stays.
    ChangeStorage: Patch "/storage", Session, (), (), Json<StorageSettings>, Json<StorageStatus>;

    /// Whether the owner accepted the Minecraft EULA.
    GetEula: Get "/eula", Session, (), (), Empty, Json<Eula>;
    /// Accept the Minecraft EULA for every server on this Core.
    AcceptEula: Post "/eula", Session, (), (), Empty, Json<Eula>;

    /// The end of the Core's own log.
    GetLogs: Get "/logs", Session, (), LogsQuery, Empty, Json<CoreLogs>;
    /// The machine the Core runs on.
    GetMachine: Get "/machine", Session, (), (), Empty, Json<Machine>;

    /// Every server on this Core.
    ListServers: Get "/servers", Session, (), (), Empty, Json<Vec<CoreServer>>;
    /// An empty server, to install a loader or pack on.
    CreateServer: Post "/servers", Session, (), (), Json<NewServer>, Json<ServerId>;
    /// One server.
    GetServer: Get "/servers/{id}", Session, ServerId, (), Empty, Json<CoreServer>;
    /// Rename a server or change its update channel.
    ChangeServer: Patch "/servers/{id}", Session, ServerId, (), Json<ServerUpdate>, Empty;
    /// A stopped server, with its world, files and backups.
    DeleteServer: Delete "/servers/{id}", Session, ServerId, (), Empty, Empty, dangerous;
    /// Start answers once Java runs, stop and kill once the server has the request, restart at
    /// once.
    ChangePower: Post "/servers/{id}/power", Session, ServerId, (), Json<PowerChange>, Empty;
    /// Install a loader on a stopped server, answering once it is done.
    InstallLoader: Post "/servers/{id}/reinstall", Session, ServerId, (), Json<LoaderInstall>, Empty;
    /// Install an `.mrpack`, the body, over a stopped server. It goes on in the background.
    InstallModpackFile: Post "/servers/{id}/modpack-file", Session, ServerId, (), Stream, Empty;
    /// Push a version of the server's own share, which the first push creates.
    ShareServer: Post "/servers/{id}/share", Session, ServerId, (), Json<SharePush>, Json<SharedServer>;
    /// Apply the latest version of an App instance's share, which the server follows from then
    /// on and its own share goes. A stopped server installs it now; a running one when it next
    /// stops.
    FollowInstance: Post "/servers/{id}/follow", Session, ServerId, (), Json<InstanceFollow>, Empty;
    /// How the server runs: its command, Java and shared instance.
    GetSetup: Get "/servers/{id}/setup", Session, ServerId, (), Empty, Json<ServerSetup>;
    /// The managed Java the server runs, from its next start.
    SetJavaVersion: Put "/servers/{id}/java-version", Session, ServerId, (), Json<JavaMajor>, Empty;
    /// The server's `server.properties`.
    GetProperties: Get "/servers/{id}/properties", Session, ServerId, (), Empty, Json<ServerProperties>;
    /// Answers with the file as it is now.
    ChangeProperties: Patch "/servers/{id}/properties", Session, ServerId, (), Json<PropertiesUpdate>, Json<ServerProperties>;
    /// The server's backups, their schedule, and the operations queued on them.
    ListBackups: Get "/servers/{id}/backups", Session, ServerId, (), Empty, Json<ServerBackups>;
    /// Back up the server as it is, hot when it runs, into a free manual slot.
    CreateBackup: Post "/servers/{id}/backups", Session, ServerId, (), Json<NewBackup>, Json<CreatedBackup>;
    /// Deleting the excess scheduled backups needs the ticket.
    ChangeBackupSchedule: Patch "/servers/{id}/backups/schedule", Session, ServerId, (), Json<ScheduleUpdate>, Json<ScheduleChange>;
    /// Delete several backups.
    DeleteBackups: Post "/servers/{id}/backups/delete", Session, ServerId, (), Json<BackupIds>, Empty, dangerous;
    /// Rename a backup.
    RenameBackup: Patch "/servers/{id}/backups/{backup_id}", Session, BackupRef, (), Json<BackupUpdate>, Empty;
    /// Delete one backup.
    DeleteBackup: Delete "/servers/{id}/backups/{backup_id}", Session, BackupRef, (), Empty, Empty, dangerous;
    /// Queue a restore over the stopped server's world, after a backup of it.
    RestoreBackup: Post "/servers/{id}/backups/{backup_id}/restore", Session, BackupRef, (), Empty, Empty, dangerous;
    /// Queue a failed backup again.
    RetryBackup: Post "/servers/{id}/backups/{backup_id}/retry", Session, BackupRef, (), Empty, Empty;
    /// Stop showing an operation that ended.
    AckOperation: Post "/servers/{id}/backups/operations/{operation_id}/ack", Session, OperationRef, (), Empty, Empty;
    /// Drop a queued operation; a cancelled backup goes with it.
    CancelOperation: Post "/servers/{id}/backups/operations/{operation_id}/cancel", Session, OperationRef, (), Empty, Empty;

    /// The server's memory, JVM flags and restart settings.
    GetPerformance: Get "/servers/{id}/performance", Session, ServerId, (), Empty, Json<Performance>;
    /// Used from the server's next start.
    SetMemory: Put "/servers/{id}/memory", Session, ServerId, (), Json<Memory>, Json<Performance>;
    /// Used from the server's next start.
    SetFlags: Put "/servers/{id}/flags", Session, ServerId, (), Json<JvmFlags>, Json<Performance>;
    /// When the server restarts on its own.
    SetRestarts: Put "/servers/{id}/restarts", Session, ServerId, (), Json<Restarts>, Json<Performance>;
    /// Send one command to the server's console, and learn why one was refused.
    SendCommand: Post "/servers/{id}/console", Session, ServerId, (), Json<ConsoleCommand>, Empty;
    /// A custom Java for the server, or the Core's own.
    SetJava: Put "/servers/{id}/java", Session, ServerId, (), Json<JavaPath>, Empty;
    /// What the server is doing and why it last stopped.
    GetInsight: Get "/servers/{id}/insight", Session, ServerId, (), Empty, Json<ServerInsight>;
    /// Check a running server's port again, answering once checked.
    CheckReachability: Post "/servers/{id}/reachability", Session, ServerId, (), Empty, Json<ServerInsight>;
}

#[cfg(test)]
pub(crate) fn ts() -> String {
    use ts_rs::TS;

    let cfg = crate::ts::config();
    let declarations = [
        Health::decl(&cfg),
        ServerId::decl(&cfg),
        SessionId::decl(&cfg),
    ]
    .into_iter()
    .chain(access::ts_types())
    .chain(events::ts_types())
    .chain(servers::ts_types())
    .chain(machine::ts_types())
    .chain(hosting::ts_types())
    .map(|declaration| format!("\nexport {declaration}\n"))
    .collect::<String>();
    format!(
        "\n/** The header a dangerous route's fresh ticket travels in. */\n\
         export const TICKET_HEADER = {TICKET_HEADER:?}\n{declarations}\n\
         /** A Core's own routes. */\nexport const core = {{\n{}}}\n",
        endpoints_ts()
    )
}
