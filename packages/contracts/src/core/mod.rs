//! A Core's own routes, besides the engine's. A device signs in with a ticket from the backend
//! ([`SignIn`]) and sends the session it gets as its bearer; the local `copal` CLI is the owner
//! and sends neither. A dangerous route also needs a fresh ticket in the `X-Amberite-Ticket`
//! header.

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

use crate::{Auth, Empty, Endpoint, Json, Method};

/// The header a dangerous route's fresh ticket travels in.
pub const TICKET_HEADER: &str = "x-amberite-ticket";

/// What `GET /health` answers, to anyone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Health {
    /// The `PROTOCOL_VERSION` the Core speaks. One that reports none speaks version 1.
    #[serde(default = "first_protocol")]
    #[cfg_attr(feature = "ts", ts(as = "Option<u32>", optional))]
    pub protocol: u32,
    pub version: String,
    pub core_id: String,
}

fn first_protocol() -> u32 {
    1
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ServerId {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MemberId {
    /// The server.
    pub id: String,
    pub user_id: String,
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
    /// Every session for the owner, your own for anyone else.
    ListSessions: Get "/sessions", Session, (), (), Empty, Json<Vec<CoreSession>>;
    /// End this device's session.
    SignOut: Delete "/sessions/current", Session, (), (), Empty, Empty;
    /// End a session. Someone else's is the owner's to end, and needs the ticket.
    RevokeSession: Delete "/sessions/{id}", Session, SessionId, (), Empty, Empty, dangerous;

    /// The caller's own access.
    GetMyAccess: Get "/access/me", Session, (), (), Empty, Json<MyAccess>;
    /// A server's owner and members. Owner only.
    ListMembers: Get "/servers/{id}/access", Session, ServerId, (), Empty, Json<ServerAccess>;
    /// Give someone a role on a server, at once; the backend notifies them. Inviting someone who
    /// has a role sets the new one. Owner only.
    InviteMember: Post "/servers/{id}/access", Session, ServerId, (), Json<NewMember>, Json<Member>, dangerous;
    /// Owner only.
    ChangeRole: Patch "/servers/{id}/access/{user_id}", Session, MemberId, (), Json<MemberUpdate>, Json<Member>, dangerous;
    /// Take someone's role on a server, ending every session they have on the Core. Owner only,
    /// except that a member may remove themselves to leave, which needs no ticket.
    RemoveMember: Delete "/servers/{id}/access/{user_id}", Session, MemberId, (), Empty, Empty, dangerous;

    /// The activity log, newest first. Owner only.
    ListActivity: Get "/activity", Session, (), ActivityQuery, Empty, Json<ActivityPage>;

    /// Whether this Core is paired, and the code it shows while it is not. Owner only.
    GetPairing: Get "/pairing", Session, (), (), Empty, Json<PairingStatus>;
    /// Drop the code shown and get another, within 20 seconds, while unpaired. Owner only.
    RequestPairingCode: Post "/pairing/code", Session, (), (), Empty, Json<PairingStatus>;
    /// The owner is removing this Core from their account: every session ends and the Core shows
    /// a new pairing code.
    Unpair: Delete "/pairing", Session, (), (), Empty, Empty, dangerous;

    /// Owner only.
    GetUpdate: Get "/update", Session, (), (), Empty, Json<UpdateStatus>;
    /// Owner only. Turning auto-update on or changing the channel needs the ticket.
    ChangeUpdateSettings: Patch "/update", Session, (), (), Json<UpdateSettings>, Json<UpdateStatus>, dangerous;
    /// Look up the newest release now. Owner only.
    CheckForUpdate: Post "/update/check", Session, (), (), Empty, Json<UpdateStatus>;
    /// The Core restarts into the new version right after answering. Its shape never changes, so
    /// an App can update a Core that speaks an older protocol.
    InstallUpdate: Post "/update/apply", Session, (), (), Json<ApplyUpdate>, Json<Restarting>, dangerous;
    /// Back to the previous version, with the database from before the update.
    RollBack: Post "/update/rollback", Session, (), (), Empty, Json<Restarting>, dangerous;

    /// Owner only.
    GetStorage: Get "/storage", Session, (), (), Empty, Json<StorageStatus>;
    /// Change what is given; the rest stays. Owner only.
    ChangeStorage: Patch "/storage", Session, (), (), Json<StorageSettings>, Json<StorageStatus>;

    /// Owner only.
    GetEula: Get "/eula", Session, (), (), Empty, Json<Eula>;
    /// Accept the Minecraft EULA for every server on this Core. Owner only.
    AcceptEula: Post "/eula", Session, (), (), Empty, Json<Eula>;

    /// The end of the Core's own log. Owner only.
    GetLogs: Get "/logs", Session, (), LogsQuery, Empty, Json<CoreLogs>;
    /// Owner only.
    GetMachine: Get "/machine", Session, (), (), Empty, Json<Machine>;

    /// Every server the caller has a role on.
    ListServers: Get "/servers", Session, (), (), Empty, Json<Vec<CoreServer>>;
    /// An empty server, to install a loader or pack on. Owner only.
    CreateServer: Post "/servers", Session, (), (), Json<NewServer>, Json<ServerId>;
    /// Viewers and up.
    GetServer: Get "/servers/{id}", Session, ServerId, (), Empty, Json<CoreServer>;
    /// Editors and up.
    ChangeServer: Patch "/servers/{id}", Session, ServerId, (), Json<ServerUpdate>, Empty;
    /// A stopped server, with its world, files and backups, and every role on it. Owner only.
    DeleteServer: Delete "/servers/{id}", Session, ServerId, (), Empty, Empty, dangerous;
    /// Start answers once Java runs, stop and kill once the server has the request, restart at
    /// once. Editors and up.
    ChangePower: Post "/servers/{id}/power", Session, ServerId, (), Json<PowerChange>, Empty;
    /// Install a loader on a stopped server, answering once it is done. Editors and up.
    InstallLoader: Post "/servers/{id}/reinstall", Session, ServerId, (), Json<LoaderInstall>, Empty;
    /// Viewers and up.
    GetSetup: Get "/servers/{id}/setup", Session, ServerId, (), Empty, Json<ServerSetup>;
    /// The managed Java the server runs, from its next start. Editors and up.
    SetJavaVersion: Put "/servers/{id}/java-version", Session, ServerId, (), Json<JavaMajor>, Empty;
    /// Editors and up: the file can hold secrets, such as a resource pack URL.
    GetProperties: Get "/servers/{id}/properties", Session, ServerId, (), Empty, Json<ServerProperties>;
    /// Editors and up. Answers with the file as it is now.
    ChangeProperties: Patch "/servers/{id}/properties", Session, ServerId, (), Json<PropertiesUpdate>, Json<ServerProperties>;
    /// Viewers and up.
    ListBackups: Get "/servers/{id}/backups", Session, ServerId, (), Empty, Json<ServerBackups>;
    /// Back up the server as it is, hot when it runs, into a free manual slot. Editors and up.
    CreateBackup: Post "/servers/{id}/backups", Session, ServerId, (), Json<NewBackup>, Json<CreatedBackup>;
    /// Editors and up. Deleting the excess scheduled backups is the owner's, with the ticket.
    ChangeBackupSchedule: Patch "/servers/{id}/backups/schedule", Session, ServerId, (), Json<ScheduleUpdate>, Json<ScheduleChange>;
    /// Owner only: backups are what undoes damage.
    DeleteBackups: Post "/servers/{id}/backups/delete", Session, ServerId, (), Json<BackupIds>, Empty, dangerous;
    /// Editors and up.
    RenameBackup: Patch "/servers/{id}/backups/{backup_id}", Session, BackupRef, (), Json<BackupUpdate>, Empty;
    /// Owner only.
    DeleteBackup: Delete "/servers/{id}/backups/{backup_id}", Session, BackupRef, (), Empty, Empty, dangerous;
    /// Queue a restore over the stopped server's world, after a backup of it. Owner only.
    RestoreBackup: Post "/servers/{id}/backups/{backup_id}/restore", Session, BackupRef, (), Empty, Empty, dangerous;
    /// Queue a failed backup again. Editors and up.
    RetryBackup: Post "/servers/{id}/backups/{backup_id}/retry", Session, BackupRef, (), Empty, Empty;
    /// Stop showing an operation that ended. Editors and up.
    AckOperation: Post "/servers/{id}/backups/operations/{operation_id}/ack", Session, OperationRef, (), Empty, Empty;
    /// Drop a queued operation; a cancelled backup goes with it. Editors and up.
    CancelOperation: Post "/servers/{id}/backups/operations/{operation_id}/cancel", Session, OperationRef, (), Empty, Empty;

    /// Viewers and up.
    GetPerformance: Get "/servers/{id}/performance", Session, ServerId, (), Empty, Json<Performance>;
    /// Used from the server's next start. Editors and up.
    SetMemory: Put "/servers/{id}/memory", Session, ServerId, (), Json<Memory>, Json<Performance>;
    /// Used from the server's next start. Editors and up.
    SetFlags: Put "/servers/{id}/flags", Session, ServerId, (), Json<JvmFlags>, Json<Performance>;
    /// Editors and up.
    SetRestarts: Put "/servers/{id}/restarts", Session, ServerId, (), Json<Restarts>, Json<Performance>;
    /// Send one command to the server's console, and learn why one was refused. Editors and up.
    SendCommand: Post "/servers/{id}/console", Session, ServerId, (), Json<ConsoleCommand>, Empty;
    /// A custom Java for the server, or the Core's own. Editors and up.
    SetJava: Put "/servers/{id}/java", Session, ServerId, (), Json<JavaPath>, Empty;
    /// Viewers and up; a crash's console lines are for editors and up.
    GetInsight: Get "/servers/{id}/insight", Session, ServerId, (), Empty, Json<ServerInsight>;
    /// Check a running server's port again, answering once checked. Editors and up.
    CheckReachability: Post "/servers/{id}/reachability", Session, ServerId, (), Empty, Json<ServerInsight>;
}

#[cfg(test)]
pub(crate) fn ts() -> String {
    use ts_rs::TS;

    let cfg = crate::ts::config();
    let declarations = [
        Health::decl(&cfg),
        ServerId::decl(&cfg),
        MemberId::decl(&cfg),
        SessionId::decl(&cfg),
    ]
    .into_iter()
    .chain(access::ts_types())
    .chain(events::ts_types())
    .chain(hosting::ts_types())
    .chain(machine::ts_types())
    .chain(servers::ts_types())
    .map(|declaration| format!("\nexport {declaration}\n"))
    .collect::<String>();
    format!(
        "\n/** The header a dangerous route's fresh ticket travels in. */\n\
         export const TICKET_HEADER = {TICKET_HEADER:?}\n{declarations}\n\
         /** A Core's own routes. */\nexport const core = {{\n{}}}\n",
        endpoints_ts()
    )
}
