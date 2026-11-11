use serde::{Deserialize, Serialize};

/// The error body of every route we own.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ApiError {
    pub error: ErrorCode,
    /// Ready to show.
    pub description: String,
}

impl ApiError {
    pub fn new(error: ErrorCode, description: impl Into<String>) -> Self {
        Self {
            error,
            description: description.into(),
        }
    }
}

/// Every error code of the backend, Cores, stores, the engine and the App. The engine's keep the
/// exact strings `engine::error::error_code` returns, which the App's helpers switch on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidInput,
    InvalidCredentials,
    Unauthorized,
    Forbidden,
    NotFound,
    MethodNotAllowed,
    Conflict,
    RateLimited,
    InternalError,
    Unsupported,

    AccountLocked,
    AlreadySignedIn,
    CodeExpired,
    ConfigurationError,
    CoreRemoved,
    DuplicateEmail,
    InvalidAddress,
    MailError,
    NoStorage,
    RateLimit,
    TicketsUnavailable,
    TooManyAttempts,
    TooManyInvites,
    TooManyRequests,
    UnsupportedOperation,
    UsernameTaken,
    WeakPassword,
    WrongCode,
    Blacklisted,
    CoreNotFound,
    CurrentVersion,
    EndpointNotFound,
    IconNotFound,
    InstanceNotFound,
    InviteInvalid,
    InviteNotFound,
    MembersOnly,
    NotMember,
    NotModerator,
    NotOwnList,
    NotOwner,
    PushInFlight,
    Quarantined,
    ShareTooLarge,
    StorageQuota,
    TooManyFiles,
    VersionNotFound,
    AlreadyFriends,
    CannotFriendSelf,
    OwnFriendRequest,

    TicketRequired,
    TicketInvalid,
    TicketWrongCore,
    TicketWrongKind,
    TicketExpired,
    TicketReused,
    ClockWrong,
    NotInvited,
    DiskFull,
    EgressSpent,
    EulaNotAccepted,
    NotManaged,
    AlreadyRunning,
    Installing,
    NoPorts,

    SharedInstanceDeleted,
    SharedInstanceAccessRevoked,
    SharedInstanceQuarantined,
    SharedInstancesApiError,
    SharedInstanceFileNotAllowed,
    SharedInstanceOwnerBehind,
    SharedInstanceLimitReached,
    PoolNoStorage,
    PoolPushInFlight,
    PoolTooLarge,
    PoolDiskFull,
    PoolEgressSpent,
    PoolPushUnconfirmed,
    PoolFilesBroken,
    PoolFilesUnavailable,
    HashError,
    LoginCanceled,
    NotLoggedIn,
    NoLoaderVersion,
    LauncherError,
    JavaError,
    MetadataError,
    NetworkError,
    ArchiveError,
    PathError,
    FilesystemError,
    ParseError,
    DatabaseError,
    DirectoryMoveNoSpace,
    DirectoryMovePermissionDenied,
    DirectoryMoveFailed,
    PermissionDenied,
    /// Modrinth's API failed.
    #[serde(rename = "api_error")]
    ModrinthApi,
    MinecraftAuthNoProfile,
    MinecraftAuthTokenExpired,
    MinecraftAuthClockOutOfSync,
    MinecraftAuthRateLimited,
    MinecraftAuthServiceUnavailable,
    MinecraftAuthConnectionRefused,
    MinecraftAuthNetwork,
    MinecraftAuthAgeVerification,
    MinecraftAuthBanned,
    MinecraftAuthRestricted,
    MinecraftAuthNoXboxProfile,
    MinecraftAuthTermsNotAccepted,
    MinecraftAuthRegionUnavailable,
    MinecraftAuthAdultVerification,
    MinecraftAuthUnderage,
    MinecraftAuthXstsRejected,
    MinecraftAuthError,

    EngineUnavailable,
    ExternalSaveFailed,
    SignInCancelled,

    #[serde(other)]
    Unknown,
}

impl ErrorCode {
    /// The HTTP status a Rust server answers with. The backend sets its statuses itself.
    pub fn status(self) -> u16 {
        match self {
            Self::InvalidInput
            | Self::InvalidAddress
            | Self::DuplicateEmail
            | Self::UsernameTaken
            | Self::WeakPassword
            | Self::CannotFriendSelf
            | Self::OwnFriendRequest
            | Self::NoLoaderVersion
            | Self::PathError
            | Self::ParseError => 400,
            Self::InvalidCredentials
            | Self::Unauthorized
            | Self::TicketRequired
            | Self::TicketInvalid
            | Self::TicketWrongCore
            | Self::TicketWrongKind
            | Self::TicketExpired
            | Self::TicketReused
            | Self::ClockWrong
            | Self::NotLoggedIn
            | Self::MinecraftAuthTokenExpired => 401,
            Self::Forbidden
            | Self::AccountLocked
            | Self::Blacklisted
            | Self::MembersOnly
            | Self::NotMember
            | Self::NotModerator
            | Self::NotOwnList
            | Self::NotOwner
            | Self::Quarantined
            | Self::NotInvited
            | Self::EulaNotAccepted
            | Self::SharedInstanceAccessRevoked
            | Self::SharedInstanceQuarantined
            | Self::SharedInstanceFileNotAllowed
            | Self::PermissionDenied
            | Self::DirectoryMovePermissionDenied
            | Self::MinecraftAuthAgeVerification
            | Self::MinecraftAuthBanned
            | Self::MinecraftAuthRestricted
            | Self::MinecraftAuthNoXboxProfile
            | Self::MinecraftAuthNoProfile
            | Self::MinecraftAuthTermsNotAccepted
            | Self::MinecraftAuthRegionUnavailable
            | Self::MinecraftAuthAdultVerification
            | Self::MinecraftAuthUnderage
            | Self::MinecraftAuthXstsRejected => 403,
            Self::NotFound
            | Self::WrongCode
            | Self::CoreNotFound
            | Self::EndpointNotFound
            | Self::IconNotFound
            | Self::InstanceNotFound
            | Self::InviteInvalid
            | Self::InviteNotFound
            | Self::VersionNotFound => 404,
            Self::MethodNotAllowed => 405,
            Self::Conflict
            | Self::AlreadySignedIn
            | Self::CurrentVersion
            | Self::PushInFlight
            | Self::AlreadyFriends
            | Self::NotManaged
            | Self::AlreadyRunning
            | Self::Installing
            | Self::PoolPushInFlight
            | Self::SharedInstanceOwnerBehind
            | Self::LoginCanceled
            | Self::SignInCancelled => 409,
            Self::CodeExpired
            | Self::CoreRemoved
            | Self::SharedInstanceDeleted => 410,
            Self::ShareTooLarge
            | Self::StorageQuota
            | Self::TooManyFiles
            | Self::PoolTooLarge
            | Self::SharedInstanceLimitReached => 413,
            Self::RateLimited
            | Self::RateLimit
            | Self::TooManyAttempts
            | Self::TooManyInvites
            | Self::TooManyRequests
            | Self::EgressSpent
            | Self::PoolEgressSpent
            | Self::MinecraftAuthRateLimited => 429,
            Self::Unsupported | Self::UnsupportedOperation => 501,
            Self::NetworkError
            | Self::ModrinthApi
            | Self::SharedInstancesApiError
            | Self::PoolFilesUnavailable
            | Self::PoolPushUnconfirmed
            | Self::MinecraftAuthClockOutOfSync
            | Self::MinecraftAuthConnectionRefused
            | Self::MinecraftAuthNetwork
            | Self::MinecraftAuthError => 502,
            Self::ConfigurationError
            | Self::MailError
            | Self::NoStorage
            | Self::TicketsUnavailable
            | Self::NoPorts
            | Self::PoolNoStorage
            | Self::EngineUnavailable
            | Self::MinecraftAuthServiceUnavailable => 503,
            Self::DiskFull
            | Self::PoolDiskFull
            | Self::DirectoryMoveNoSpace => 507,
            Self::InternalError
            | Self::PoolFilesBroken
            | Self::HashError
            | Self::LauncherError
            | Self::JavaError
            | Self::MetadataError
            | Self::ArchiveError
            | Self::FilesystemError
            | Self::DatabaseError
            | Self::DirectoryMoveFailed
            | Self::ExternalSaveFailed
            | Self::Unknown => 500,
        }
    }
}
