//! A Core's own routes, besides the engine's. A device signs in with a ticket from the backend
//! ([`SignIn`]) and sends the session it gets as its bearer; the local `copal` CLI is the owner
//! and sends neither. A dangerous route also needs a fresh ticket in the `X-Amberite-Ticket`
//! header.

mod access;
mod events;
mod machine;
mod servers;

pub use access::*;
pub use events::*;
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
