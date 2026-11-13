// Generated from packages/contracts by ts-rs. Do not edit.
// Regenerate: cargo test -p contracts -- --ignored write_ts

export const PROTOCOL_VERSION = 1

export type Method = "GET" | "POST" | "PUT" | "PATCH" | "DELETE";

export type Auth = "none" | "session" | "core_key" | "account" | "ticket" | "host";

export type BodyKind = "json" | "empty" | "raw" | "stream" | "upgrade";

export type ApiError = { error: ErrorCode, 
/**
 * Ready to show.
 */
description: string, };

export type ErrorCode = "invalid_input" | "invalid_credentials" | "unauthorized" | "forbidden" | "not_found" | "method_not_allowed" | "conflict" | "rate_limited" | "internal_error" | "unsupported" | "account_locked" | "already_signed_in" | "code_expired" | "configuration_error" | "core_removed" | "duplicate_email" | "invalid_address" | "mail_error" | "no_storage" | "rate_limit" | "tickets_unavailable" | "too_many_attempts" | "too_many_invites" | "too_many_requests" | "unsupported_operation" | "username_taken" | "weak_password" | "wrong_code" | "blacklisted" | "core_not_found" | "current_version" | "endpoint_not_found" | "icon_not_found" | "instance_not_found" | "invite_invalid" | "invite_not_found" | "members_only" | "not_member" | "not_moderator" | "not_own_list" | "not_owner" | "push_in_flight" | "quarantined" | "share_too_large" | "storage_quota" | "too_many_files" | "version_not_found" | "already_friends" | "cannot_friend_self" | "own_friend_request" | "ticket_required" | "ticket_invalid" | "ticket_wrong_core" | "ticket_wrong_kind" | "ticket_expired" | "ticket_reused" | "clock_wrong" | "not_invited" | "disk_full" | "egress_spent" | "eula_not_accepted" | "not_managed" | "already_running" | "installing" | "no_ports" | "shared_instance_deleted" | "shared_instance_access_revoked" | "shared_instance_quarantined" | "shared_instances_api_error" | "shared_instance_file_not_allowed" | "shared_instance_owner_behind" | "shared_instance_limit_reached" | "pool_no_storage" | "pool_push_in_flight" | "pool_too_large" | "pool_disk_full" | "pool_egress_spent" | "pool_push_unconfirmed" | "pool_files_broken" | "pool_files_unavailable" | "hash_error" | "login_canceled" | "not_logged_in" | "no_loader_version" | "launcher_error" | "java_error" | "metadata_error" | "network_error" | "archive_error" | "path_error" | "filesystem_error" | "parse_error" | "database_error" | "directory_move_no_space" | "directory_move_permission_denied" | "directory_move_failed" | "permission_denied" | "api_error" | "minecraft_auth_no_profile" | "minecraft_auth_token_expired" | "minecraft_auth_clock_out_of_sync" | "minecraft_auth_rate_limited" | "minecraft_auth_service_unavailable" | "minecraft_auth_connection_refused" | "minecraft_auth_network" | "minecraft_auth_age_verification" | "minecraft_auth_banned" | "minecraft_auth_restricted" | "minecraft_auth_no_xbox_profile" | "minecraft_auth_terms_not_accepted" | "minecraft_auth_region_unavailable" | "minecraft_auth_adult_verification" | "minecraft_auth_underage" | "minecraft_auth_xsts_rejected" | "minecraft_auth_error" | "engine_unavailable" | "external_save_failed" | "sign_in_cancelled" | "cancelled" | "unknown";

export type Hash = string;

export type TicketKind = "session" | "store" | "fetch" | "pull" | "read";

export type SessionTicket = { iss: string, kind: TicketKind, 
/**
 * The user id.
 */
sub: string, name: string, avatar_url: string | null, 
/**
 * The Core it is for.
 */
aud: string, 
/**
 * Unix seconds.
 */
iat: number, 
/**
 * Unix seconds.
 */
exp: number, 
/**
 * Random one-time id. The Core records it in the same write that uses the ticket.
 */
jti: string, };

export type StoreTicket = { iss: string, kind: TicketKind, aud: Audience, 
/**
 * The version's hash.
 */
root: string, bytes: number, iat: number, exp: number, jti: string, };

export type FetchTicket = { iss: string, kind: TicketKind, aud: Audience, 
/**
 * The version's hash.
 */
root: string, iat: number, exp: number, jti: string, };

export type PullTicket = { iss: string, kind: TicketKind, aud: Audience, 
/**
 * The version's hash.
 */
root: string, source: PullSource, iat: number, exp: number, jti: string, };

export type ReadTicket = { iss: string, kind: TicketKind, 
/**
 * The user id.
 */
sub: string, 
/**
 * The Core it is for.
 */
aud: string, 
/**
 * The share id.
 */
share: string, iat: number, exp: number, jti: string, };

export type Audience = string | Array<string>;

export type PullSource = { core: StoreHost, ticket: string, };

export type StoreHost = { id: string, 
/**
 * `https://ip:port` on its own network.
 */
address: string, 
/**
 * `https://ip:port` from the internet, once known.
 */
public_address: string | null, 
/**
 * SHA-256 of its certificate, lowercase hex. The only certificate a caller accepts there.
 */
cert_fingerprint: string, };

export type PublicKeys = { keys: Array<PublicKey>, };

export type PublicKey = { 
/**
 * Always `OKP`.
 */
kty: string, 
/**
 * Always `Ed25519`.
 */
crv: string, 
/**
 * The id a ticket names in its `kid` header.
 */
kid: string, 
/**
 * The key, base64url.
 */
x: string, };

/** A route, as `ContractClient` calls it. `_types` only carries its Rust types. */
export interface Endpoint<P = unknown, Q = unknown, B = unknown, R = unknown> {
	method: Method
	path: string
	auth: Auth
	dangerous: boolean
	body: BodyKind
	response: BodyKind
	readonly _types?: { params: P; query: Q; body: B; response: R }
}

/**
 * The most plain bytes one share version may hold. The App checks it before it uploads
 * anything; the backend enforces it. Placeholder, decide later.
 */
export const MAX_SHARE_BYTES = 1073741824

/**
 * What every account may keep on other people's Cores. The backend enforces it.
 * Placeholder, decide later.
 */
export const ALLOWANCE_BYTES = 3221225472

/**
 * The most entries an archive a Core unpacks (a pack, a share, an upload) may have.
 */
export const MAX_ARCHIVE_ENTRIES = 200000

/**
 * The most an archive entry may grow when unpacked, as uncompressed bytes per compressed
 * byte. Past this it is a zip bomb.
 */
export const MAX_COMPRESSION_RATIO = 100

/**
 * The free disk space a Core keeps after anything it writes: what it writes must fit in the
 * free space less this.
 */
export const DISK_MARGIN_BYTES = 268435456

/**
 * The most a version's object list in a store may weigh: 4,096 objects take well under a
 * megabyte.
 */
export const MAX_VERSION_LIST_BYTES = 4194304

/**
 * The folder a server's shared files live in.
 */
export const SERVER_SHARE_FOLDER = "config"

/**
 * Extensions of the config files a server's share may carry.
 */
export const SHAREABLE_CONFIG_EXTENSIONS: readonly string[] = ["json","json5","jsonc","yml","yaml","css","toml","txt","ini","cfg","conf","properties","xml","nbt"]

/**
 * The `iss` of every ticket the backend signs.
 */
export const TICKET_ISSUER = "amberite"

/**
 * The header a Core reading a share's versions for a user sends the read ticket that user's
 * App got for it in. Never the query, which request logs keep.
 */
export const READ_TICKET_HEADER = "x-amberite-read-ticket"

export type NewSignInCode = { core_id: string, 
/**
 * The machine's name, shown to the owner before they approve.
 */
name: string, 
/**
 * Where the App reaches the Core, `https://ip:port`.
 */
address: string, 
/**
 * SHA-256 of the Core's certificate, lowercase hex.
 */
cert_fingerprint: string | null, };

export type SignInCode = { code: string, key: string, 
/**
 * Seconds.
 */
expires_in: number, 
/**
 * Seconds.
 */
poll_interval: number, };

export type SignInStatus = { "status": "pending" } | { "status": "approved", account: CoreOwner, };

export type CoreOwner = { id: string, username: string, };

export type StateReport = { version: string, 
/**
 * Where the App reaches this Core on its own network, `https://ip:port`.
 */
address: string, 
/**
 * Where the App reaches it from outside, once known.
 */
public_address: string | null, 
/**
 * SHA-256 of the TLS certificate, lowercase hex. The App pins it.
 */
cert_fingerprint: string, 
/**
 * The servers shown to the people their instance is shared with.
 */
servers: Array<VisibleServer>, };

export type VisibleServer = { id: string, name: string, 
/**
 * The Minecraft address, `host:port`.
 */
address: string, shared_instance_id: string, };

export type StorageReport = { 
/**
 * This Core offers storage to other people's shares. Off, it holds only its owner's.
 */
enabled: boolean, 
/**
 * The Core is installed as a service.
 */
service: boolean, cap: number, used: number, 
/**
 * Free on the store's disk.
 */
free: number, 
/**
 * What it may serve a day, and has served today (UTC).
 */
egress_cap: number, egress_today: number, 
/**
 * Every version the store holds whole.
 */
versions: Array<HeldVersion>, };

export type HeldVersion = { 
/**
 * The version's hash.
 */
root: string, 
/**
 * The bytes of its objects.
 */
bytes: number, };

export type StorageAnswer = { 
/**
 * Versions to stop holding.
 */
drop: Array<string>, 
/**
 * Versions to copy from another Core.
 */
pull: Array<PullOrder>, };

export type PullOrder = { root: string, 
/**
 * A pull ticket naming the source Core.
 */
ticket: string, };

export type AccessDirectory = { user_ids: Array<string>, };

export type AccessGrant = { 
/**
 * A user id or username.
 */
user: string, server_id: string, server_name: string, 
/**
 * Shown in the invite.
 */
role: CoreRole, };

export type GrantedUser = { id: string, username: string, avatar_url: string | null, };

export type AccessRevocation = { user_id: string, server_id: string, keep_core: boolean, };

export type ForgottenServer = { server_id: string, share_id: string | null, };

export type EnteredCode = { code: string, };

export type PendingCore = { 
/**
 * The machine's name, shown so the user can tell it is their Core.
 */
name: string, };

export type ApprovedCore = { id: string, name: string, };

export type ListedCore = { id: string, name: string, 
/**
 * False for a Core the account was only invited to. Its owner manages it.
 */
owned: boolean, 
/**
 * The Core's own version. Null until it has started once since it learned to report it.
 */
version: string | null, 
/**
 * Where this Core is on its own network, reported by the Core itself.
 */
address: string, 
/**
 * Where this Core is on the internet, once it has proven it can be reached there.
 */
public_address: string | null, 
/**
 * SHA-256 of the certificate this Core serves HTTPS with. The App trusts only this
 * certificate for this Core. Null for a Core that does not serve HTTPS yet.
 */
cert_fingerprint: string | null, created: string, last_seen: string, };

export type CoreTicket = { ticket: string, };

export type CoreId = { id: string, };

/** The backend's Core routes. */
export const cores = {
	RequestSignInCode: { method: "POST", path: "/core/auth/code", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, NewSignInCode, SignInCode>,
	PollSignIn: { method: "POST", path: "/core/auth/poll", auth: "core_key", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, SignInStatus>,
	WaitForSignIn: { method: "GET", path: "/core/auth/wait", auth: "core_key", dangerous: false, body: "empty", response: "upgrade" } as Endpoint<null, null, void, { receive: string, send: null }>,
	ReportState: { method: "POST", path: "/core/state", auth: "core_key", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, StateReport, void>,
	ReportStorage: { method: "POST", path: "/core/storage", auth: "core_key", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, StorageReport, StorageAnswer>,
	ReplaceAccess: { method: "PUT", path: "/core/access", auth: "core_key", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, AccessDirectory, void>,
	GrantAccess: { method: "POST", path: "/core/access", auth: "core_key", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, AccessGrant, GrantedUser>,
	RevokeAccess: { method: "DELETE", path: "/core/access", auth: "core_key", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, AccessRevocation, void>,
	ForgetPlayers: { method: "DELETE", path: "/core/players", auth: "core_key", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, ForgottenServer, void>,
	LookUpSignInCode: { method: "POST", path: "/cores/approve/lookup", auth: "account", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, EnteredCode, PendingCore>,
	ApproveSignInCode: { method: "POST", path: "/cores/approve", auth: "account", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, EnteredCode, ApprovedCore>,
	ListCores: { method: "GET", path: "/cores", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<ListedCore>>,
	IssueTicket: { method: "POST", path: "/cores/{id}/ticket", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<CoreId, null, void, CoreTicket>,
	RemoveCore: { method: "DELETE", path: "/cores/{id}", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<CoreId, null, void, void>,
	Keys: { method: "GET", path: "/core/keys.json", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, PublicKeys>,
}

export type SharedIconConfig = { background: SharedIconBackground, symbol: string, };

export type SharedIconBackground = { "type": "color", value: string, } | { "type": "linear-top-down-gradient", top_color: string, bottom_color: string, };

export type SharedInstanceId = { id: string, };

export type InstanceInviteId = { id: string, invite_id: string, };

export type InstanceVersionId = { id: string, version: number, };

export type InviteId = { id: string, };

export type IconUpload = { id: string, upload: string, };

export type BlacklistedUser = { user_id: string, };

export type ModeratedFile = { id: string, version: number, file: string, };

export type CoreServerId = { core_id: string, server_id: string, };

export type ServerInviteId = { core_id: string, server_id: string, invite_id: string, };

export type SharedInstanceFilter = { 
/**
 * Only the caller's own id is allowed.
 */
user?: string, 
/**
 * Also the shares the caller joined or plays through a server.
 */
include_joined?: boolean, };

export type NewSharedInstance = { name: string, 
/**
 * A Core key only: the server whose share this is. Its existing share is answered when it
 * has one.
 */
server_id?: string, };

export type CreatedId = { id: string, };

export type SharedInstanceUpdate = { name: string, };

export type SharedInstance = { name: string, 
/**
 * The URL of an uploaded image. Null for an icon editor icon, which `icon_config` holds.
 */
icon: string | null, icon_config: SharedIconConfig | null, quarantine: boolean, 
/**
 * Someone other than the owner joined it once.
 */
installed_once: boolean, 
/**
 * For a player, the server they came through, which the App adds to the instance's list.
 */
linked_server: LinkedServer | null, };

export type LinkedServer = { 
/**
 * The Minecraft address, `host:port`.
 */
domain: string, region: string, online_status: OnlineStatus | null, };

export type OnlineStatus = "unknown" | "running" | "stopped";

export type InstanceUsers = { users: Array<InstanceUser>, 
/**
 * Invite links that still work.
 */
tokens: number, };

export type InstanceUser = { id: string, 
/**
 * RFC 3339. Null while the invite is pending.
 */
joined_at: string | null, join_type: JoinType, 
/**
 * RFC 3339.
 */
last_played: string | null, };

export type JoinType = "owner" | "invite" | "link";

export type UserIds = { user_ids: Array<string>, };

export type FailedInvites = { failed: Array<string>, };

export type InviteLink = { id: string, 
/**
 * RFC 3339.
 */
expiration: string, 
/**
 * 0 for no limit.
 */
max_uses: number, uses: number, };

export type NewInviteLink = { 
/**
 * Seconds, a week when absent. 0 is a year.
 */
max_age?: number, 
/**
 * 0, the default, for no limit.
 */
max_uses?: number, };

export type Invite = { instance_id: string, instance_name: string, instance_icon: string | null, instance_icon_config: SharedIconConfig | null, 
/**
 * Empty while no version is ready.
 */
game_version: string, loader_version: string, inviter: Inviter | null, managers: Array<InviteManager>, 
/**
 * Empty for a server's link: players never see the share's members.
 */
instance_users: Array<InviteUser>, 
/**
 * With `?version=true`: the latest ready version, without file keys or download.
 */
version?: InstanceVersion, };

export type Inviter = { id: string, name: string, avatar: string | null, };

export type InviteManager = { "type": "user", id: string, name: string, avatar: string | null, } | { "type": "server", name: string, icon: string | null, };

export type InviteUser = { id: string, name: string, avatar: string | null, joined_at: string | null, };

export type InvitePreviewQuery = { 
/**
 * Adds the latest ready version.
 */
version?: boolean, };

export type RemovedFile = { "type": "version", version_id: string, } | { "type": "path", parent: string, filename: string, };

export type VersionContent = { 
/**
 * The Modrinth versions of its content.
 */
modrinth_ids: Array<string>, 
/**
 * The Modrinth version of the modpack it is based on.
 */
modpack_id: string | null, 
/**
 * Files of the modpack that members do not get. Only with a modpack.
 */
removed_files: Array<RemovedFile>, game_version: string, 
/**
 * `vanilla`, `fabric`, `forge`, `quilt` or `neoforge`.
 */
loader: string, 
/**
 * Empty for vanilla.
 */
loader_version: string, };

export type VersionManifest = { 
/**
 * The hash of its sorted distinct object hashes, which names it on the stores.
 */
version_hash: Hash, external_files: Array<ExternalFile>, 
/**
 * The Modrinth versions of its content.
 */
modrinth_ids: Array<string>, 
/**
 * The Modrinth version of the modpack it is based on.
 */
modpack_id: string | null, 
/**
 * Files of the modpack that members do not get. Only with a modpack.
 */
removed_files: Array<RemovedFile>, game_version: string, 
/**
 * `vanilla`, `fabric`, `forge`, `quilt` or `neoforge`.
 */
loader: string, 
/**
 * Empty for vanilla.
 */
loader_version: string, };

export type ExternalFile = { file_name: string, file_type: string, 
/**
 * Always null: the file is fetched from the stores by `object`.
 */
url: string | null, file_size: number, 
/**
 * What a member's install compares to skip a file it already has.
 */
sha1: string, 
/**
 * The SHA-256 of the plain bytes.
 */
sha256: Hash, 
/**
 * The SHA-256 of the encrypted object, which names it on the stores.
 */
object: Hash, object_size: number, 
/**
 * The file's own key. Absent for content files, whose key derives from `sha256`, and in an
 * invite's preview, which carries no keys.
 */
key?: Hash, };

export type InstanceVersion = { version: number, 
/**
 * Its files are on the stores, so members can install it.
 */
ready: boolean, 
/**
 * For a member, once it is ready and has files.
 */
download?: VersionDownload, 
/**
 * For its owner, while it is not ready.
 */
upload?: VersionUpload, 
/**
 * For its owner.
 */
reports?: VersionReports, 
/**
 * The hash of its sorted distinct object hashes, which names it on the stores.
 */
version_hash: Hash, external_files: Array<ExternalFile>, 
/**
 * The Modrinth versions of its content.
 */
modrinth_ids: Array<string>, 
/**
 * The Modrinth version of the modpack it is based on.
 */
modpack_id: string | null, 
/**
 * Files of the modpack that members do not get. Only with a modpack.
 */
removed_files: Array<RemovedFile>, game_version: string, 
/**
 * `vanilla`, `fabric`, `forge`, `quilt` or `neoforge`.
 */
loader: string, 
/**
 * Empty for vanilla.
 */
loader_version: string, };

export type VersionDownload = { holders: Array<StoreHost>, ticket: string, };

export type VersionUpload = { core: StoreHost, ticket: string, 
/**
 * RFC 3339.
 */
expires: string, };

export type VersionReports = { 
/**
 * Members whose files decrypted to the wrong bytes.
 */
content: number, };

export type NewInstanceVersion = { 
/**
 * The owner's Core that runs a server of this share, which then holds its files.
 */
server_core?: string, 
/**
 * The hash of its sorted distinct object hashes, which names it on the stores.
 */
version_hash: Hash, external_files: Array<ExternalFile>, 
/**
 * The Modrinth versions of its content.
 */
modrinth_ids: Array<string>, 
/**
 * The Modrinth version of the modpack it is based on.
 */
modpack_id: string | null, 
/**
 * Files of the modpack that members do not get. Only with a modpack.
 */
removed_files: Array<RemovedFile>, game_version: string, 
/**
 * `vanilla`, `fabric`, `forge`, `quilt` or `neoforge`.
 */
loader: string, 
/**
 * Empty for vanilla.
 */
loader_version: string, };

export type VersionFault = { core_id: string, kind: FaultKind, };

export type FaultKind = "object" | "content";

export type HostedServer = { core_id: string, server_id: string, name: string, 
/**
 * The Minecraft address to ping and join. Not shown to the member.
 */
address: string, host: ServerHost, };

export type ServerHost = { id: string, username: string, avatar_url: string | null, };

export type NewReadTicket = { core_id: string, };

export type SignedReadTicket = { ticket: string, 
/**
 * RFC 3339.
 */
expires: string, };

export type ShareStorage = { used: number, allowance: number, 
/**
 * Only the shares the account owns.
 */
shares: Array<StoredShare>, };

export type StoredShare = { id: string, name: string, 
/**
 * It is on someone else's Core, so it counts against the allowance.
 */
counted: boolean, 
/**
 * What it counts against the allowance, 0 when not counted.
 */
used: number, 
/**
 * What its kept versions take, counted or not.
 */
size: number, 
/**
 * The account's own Cores holding it.
 */
cores: Array<string>, 
/**
 * Newest first.
 */
versions: Array<StoredVersion>, };

export type StoredVersion = { version: number, 
/**
 * RFC 3339.
 */
created: string, bytes: number, ready: boolean, 
/**
 * The latest ready version, which members install. It cannot be deleted.
 */
current: boolean, };

export type BlacklistStatus = { blacklisted: boolean, };

export type FollowedShare = { instance_id: string, name: string, 
/**
 * The server's game port on its Core.
 */
port: number, 
/**
 * Where the Core's router opened it on the internet. Null while it has not, which keeps the
 * last one known.
 */
outside: OutsideAddress | null, };

export type OutsideAddress = { ip: string, port: number, };

export type NewPlayers = { user_ids: Array<string>, instance_id: string, name: string, 
/**
 * The server's game port on its Core.
 */
port: number, 
/**
 * Where the Core's router opened it on the internet. Null while it has not, which keeps the
 * last one known.
 */
outside: OutsideAddress | null, };

export type NewPlayerLink = { instance_id: string, name: string, 
/**
 * The server's game port on its Core.
 */
port: number, 
/**
 * Where the Core's router opened it on the internet. Null while it has not, which keeps the
 * last one known.
 */
outside: OutsideAddress | null, 
/**
 * Seconds, a week when absent. 0 is a year.
 */
max_age?: number, 
/**
 * 0, the default, for no limit.
 */
max_uses?: number, };

/** The backend's shared instance routes, on upstream's `sharedinstances` v1 paths. */
export const sharing = {
	ListSharedInstances: { method: "GET", path: "/v1/instances", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<null, SharedInstanceFilter, void, Array<string>>,
	CreateSharedInstance: { method: "POST", path: "/v1/instances", auth: "account", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, NewSharedInstance, CreatedId>,
	GetSharedInstance: { method: "GET", path: "/v1/instances/{id}", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<SharedInstanceId, null, void, SharedInstance>,
	UpdateSharedInstance: { method: "PATCH", path: "/v1/instances/{id}", auth: "account", dangerous: false, body: "json", response: "empty" } as Endpoint<SharedInstanceId, null, SharedInstanceUpdate, void>,
	DeleteSharedInstance: { method: "DELETE", path: "/v1/instances/{id}", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<SharedInstanceId, null, void, void>,
	ListInstanceUsers: { method: "GET", path: "/v1/instances/{id}/users", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<SharedInstanceId, null, void, InstanceUsers>,
	InviteInstanceUsers: { method: "POST", path: "/v1/instances/{id}/users", auth: "account", dangerous: false, body: "json", response: "json" } as Endpoint<SharedInstanceId, null, UserIds, FailedInvites>,
	RemoveInstanceUsers: { method: "DELETE", path: "/v1/instances/{id}/users", auth: "account", dangerous: false, body: "json", response: "empty" } as Endpoint<SharedInstanceId, null, UserIds, void>,
	ListInvites: { method: "GET", path: "/v1/instances/{id}/invites", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<SharedInstanceId, null, void, Array<InviteLink>>,
	CreateInvite: { method: "POST", path: "/v1/instances/{id}/invites", auth: "account", dangerous: false, body: "json", response: "json" } as Endpoint<SharedInstanceId, null, NewInviteLink, CreatedId>,
	AcceptPendingInvite: { method: "POST", path: "/v1/instances/{id}/invites/pending", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<SharedInstanceId, null, void, void>,
	DeclinePendingInvite: { method: "DELETE", path: "/v1/instances/{id}/invites/pending", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<SharedInstanceId, null, void, void>,
	AcceptInvite: { method: "POST", path: "/v1/instances/{id}/invites/{invite_id}", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<InstanceInviteId, null, void, void>,
	DeleteInvite: { method: "DELETE", path: "/v1/instances/{id}/invites/{invite_id}", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<InstanceInviteId, null, void, void>,
	GetInvite: { method: "GET", path: "/v1/invites/{id}", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<InviteId, InvitePreviewQuery, void, Invite>,
	GetLatestVersion: { method: "GET", path: "/v1/instances/{id}/versions", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<SharedInstanceId, null, void, InstanceVersion>,
	CreateVersion: { method: "POST", path: "/v1/instances/{id}/versions", auth: "account", dangerous: false, body: "json", response: "json" } as Endpoint<SharedInstanceId, null, NewInstanceVersion, InstanceVersion>,
	GetVersion: { method: "GET", path: "/v1/instances/{id}/versions/{version}", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<InstanceVersionId, null, void, InstanceVersion>,
	DeleteVersion: { method: "DELETE", path: "/v1/instances/{id}/versions/{version}", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<InstanceVersionId, null, void, void>,
	ReportVersion: { method: "POST", path: "/v1/instances/{id}/versions/{version}/reports", auth: "account", dangerous: false, body: "json", response: "empty" } as Endpoint<InstanceVersionId, null, VersionFault, void>,
	SetIcon: { method: "PUT", path: "/v1/instances/{id}/icon", auth: "account", dangerous: false, body: "raw", response: "empty" } as Endpoint<SharedInstanceId, null, Blob, void>,
	SetGeneratedIcon: { method: "PUT", path: "/v1/instances/{id}/icon/generated", auth: "account", dangerous: false, body: "json", response: "empty" } as Endpoint<SharedInstanceId, null, SharedIconConfig, void>,
	RemoveIcon: { method: "DELETE", path: "/v1/instances/{id}/icon", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<SharedInstanceId, null, void, void>,
	GetIcon: { method: "GET", path: "/v1/icons/{id}/{upload}", auth: "none", dangerous: false, body: "empty", response: "raw" } as Endpoint<IconUpload, null, void, Blob>,
	ListHostedServers: { method: "GET", path: "/v1/instances/{id}/servers", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<SharedInstanceId, null, void, Array<HostedServer>>,
	IssueReadTicket: { method: "POST", path: "/v1/instances/{id}/server-ticket", auth: "account", dangerous: false, body: "json", response: "json" } as Endpoint<SharedInstanceId, null, NewReadTicket, SignedReadTicket>,
	GetShareStorage: { method: "GET", path: "/v1/storage", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, ShareStorage>,
	GetBlacklistStatus: { method: "GET", path: "/v1/blacklist/{user_id}", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<BlacklistedUser, null, void, BlacklistStatus>,
	BlacklistUsers: { method: "POST", path: "/v1/moderation/blacklist", auth: "account", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, UserIds, void>,
	UnblacklistUsers: { method: "DELETE", path: "/v1/moderation/blacklist", auth: "account", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, UserIds, void>,
	DeleteModeratedFile: { method: "DELETE", path: "/v1/moderation/instances/{id}/versions/{version}/files/{file}", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<ModeratedFile, null, void, void>,
	ListPlayers: { method: "GET", path: "/v1/servers/{core_id}/{server_id}/players", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<CoreServerId, null, void, InstanceUsers>,
	InvitePlayers: { method: "POST", path: "/v1/servers/{core_id}/{server_id}/players", auth: "account", dangerous: false, body: "json", response: "json" } as Endpoint<CoreServerId, null, NewPlayers, FailedInvites>,
	RemovePlayers: { method: "DELETE", path: "/v1/servers/{core_id}/{server_id}/players", auth: "account", dangerous: false, body: "json", response: "empty" } as Endpoint<CoreServerId, null, UserIds, void>,
	ListPlayerLinks: { method: "GET", path: "/v1/servers/{core_id}/{server_id}/invites", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<CoreServerId, null, void, Array<InviteLink>>,
	CreatePlayerLink: { method: "POST", path: "/v1/servers/{core_id}/{server_id}/invites", auth: "account", dangerous: false, body: "json", response: "json" } as Endpoint<CoreServerId, null, NewPlayerLink, CreatedId>,
	DeletePlayerLink: { method: "DELETE", path: "/v1/servers/{core_id}/{server_id}/invites/{invite_id}", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<ServerInviteId, null, void, void>,
}

export type Never = never;

export type Role = "developer" | "moderator" | "admin";

export type UserCampaigns = { pride_26: Never | null, };

export type UserLock = { locked_by: string, reason: string, created: string, };

export type User = { id: string, username: string, avatar_url: string | null, raw_avatar_url: string | null, bio: string | null, created: string, role: Role, badges: number, campaigns: UserCampaigns, auth_providers: Array<Never> | null, email: string | null, email_verified: boolean | null, has_password: boolean | null, has_totp: boolean | null, payout_data: Never | null, stripe_customer_id: Never | null, github_id: Never | null, allow_friend_requests: boolean | null, eligibility_verified_at: string | null, lock?: UserLock | null, };

export type SearchUser = { id: string, username: string, avatar_url: string | null, };

export type UserFriend = { id: string, friend_id: string, accepted: boolean, created: string, };

export type UserUpdate = { username?: string, bio?: string | null, allow_friend_requests?: boolean, };

export type AllProjects = { projects: Array<Never>, organizations: Record<string, never>, };

export type Theme = "light" | "dark" | "oled" | "retro";

export type LayoutOption = "grid" | "rows";

export type FriendPrivacy = "none" | "mutual" | "everyone";

export type InvitePrivacy = "none" | "friends" | "everyone";

export type AppearancePreferences = { auto: boolean, theme: Theme, };

export type BehaviorPreferences = { minimize_app: boolean, refocus_on_game_close: boolean, hide_right_sidebar: boolean, show_jump_in: boolean, compact_instance_cards: boolean, show_play_time: boolean, hide_nametag: boolean, show_all_screenshots: boolean, show_files_tab_in_instances: boolean, show_worlds_tab_in_instances: boolean, show_screenshots_tab_in_instances: boolean, show_skin_selector_in_sidebar: boolean, 
/**
 * 0 to 255.
 */
quick_instance_count: number, warn_on_unknown_modpacks: boolean, skip_non_essential_warnings: boolean, };

export type LocalizationPreferences = { 
/**
 * Letters, digits and dashes, at most 35.
 */
locale: string, };

export type LayoutPreferences = { mods: LayoutOption, plugins: LayoutOption, datapacks: LayoutOption, shaders: LayoutOption, resourcepacks: LayoutOption, modpacks: LayoutOption, servers: LayoutOption, users: LayoutOption, };

export type SidebarPreferences = { right_aligned_search: boolean, left_aligned_content: boolean, };

export type SocialPreferences = { friend_privacy: FriendPrivacy, shared_instances_privacy: InvitePrivacy, hosting_access_privacy: InvitePrivacy, };

export type UserPreferences = { appearance: AppearancePreferences, behavior: BehaviorPreferences, localization: LocalizationPreferences, layouts: LayoutPreferences, sidebars: SidebarPreferences, social: SocialPreferences, };

export type PreferencesUpdate = { [S in keyof UserPreferences]?: Partial<UserPreferences[S]> };

export type NotificationAction = { name: string, 
/**
 * The method and the route to call.
 */
action_route: [string, string], };

export type Notification = { id: string, user_id: string, read: boolean, created: string, body: { type: string, [key: string]: unknown }, name: string, text: string, link: string, actions: Array<NotificationAction>, };

export type SharedInstanceList = "shared_instance_users" | "shared_instance_servers";

export type SharedInstanceChangeBody = { type: SharedInstanceList, shared_instance_id: string, };

export type SharedInstanceChange = { body: SharedInstanceChangeBody, };

export type UserStatus = { user_id: string, profile_name: string | null, last_update: string, };

export type ServerToClientMessage = { "type": "status_update", status: UserStatus, } | { "type": "user_offline", id: string, } | { "type": "friend_statuses", statuses: Array<UserStatus>, } | { "type": "friend_request", from: string, } | { "type": "friend_request_rejected", from: string, } | { "type": "friend_socket_listening", user: string, socket: string, } | { "type": "friend_socket_stopped_listening", user: string, } | { "type": "socket_connected", to_socket: string, new_socket: string, } | { "type": "socket_closed", socket: string, } | { "type": "socket_data", socket: string, data: Array<number>, };

export type ClientToServerMessage = { "type": "status_update", profile_name: string | null, } | { "type": "socket_listen", socket: string, } | { "type": "socket_close", socket: string, } | { "type": "socket_send", socket: string, data: Array<number>, };

export type FriendsSocketMessage = ServerToClientMessage | Notification | SharedInstanceChange;

export type UserRef = { id: string, };

export type NotificationRef = { id: string, };

export type AvatarRevision = { user_id: string, revision: string, };

export type IdList = { ids: string, };

export type UserSearch = { query: string, };

export type IconFormat = { 
/**
 * `png`, `jpg`, `jpeg`, `gif` or `webp`.
 */
ext: string, };

export type SocketCode = { code: string, };

/** The backend's social routes, in labrinth's shapes. */
export const social = {
	SearchUsers: { method: "GET", path: "/users/search", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, UserSearch, void, Array<SearchUser>>,
	GetUsers: { method: "GET", path: "/users", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<null, IdList, void, Array<User>>,
	ListFriends: { method: "GET", path: "/friends", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<UserFriend>>,
	ListFriendStatuses: { method: "GET", path: "/friends/status", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<UserStatus>>,
	AddFriend: { method: "POST", path: "/friend/{id}", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<UserRef, null, void, void>,
	RemoveFriend: { method: "DELETE", path: "/friend/{id}", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<UserRef, null, void, void>,
	ListBlocks: { method: "GET", path: "/blocks", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<string>>,
	BlockUser: { method: "POST", path: "/block/{id}", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<UserRef, null, void, void>,
	UnblockUser: { method: "DELETE", path: "/block/{id}", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<UserRef, null, void, void>,
	GetAvatar: { method: "GET", path: "/avatars/{user_id}/{revision}", auth: "none", dangerous: false, body: "empty", response: "raw" } as Endpoint<AvatarRevision, null, void, Blob>,
	GetUser: { method: "GET", path: "/user/{id}", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<UserRef, null, void, User>,
	ListUserProjects: { method: "GET", path: "/user/{id}/projects", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<UserRef, null, void, Array<Never>>,
	ListUserOrganizations: { method: "GET", path: "/user/{id}/organizations", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<UserRef, null, void, Array<Never>>,
	ListUserCollections: { method: "GET", path: "/user/{id}/collections", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<UserRef, null, void, Array<Never>>,
	ListFollowedProjects: { method: "GET", path: "/user/{id}/follows", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<UserRef, null, void, Array<Never>>,
	ListAllProjects: { method: "GET", path: "/user/{id}/all-projects", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<UserRef, null, void, AllProjects>,
	EditUser: { method: "PATCH", path: "/user/{id}", auth: "account", dangerous: false, body: "json", response: "empty" } as Endpoint<UserRef, null, UserUpdate, void>,
	DeleteUser: { method: "DELETE", path: "/user/{id}", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<UserRef, null, void, void>,
	GetPreferences: { method: "GET", path: "/user/{id}/preferences", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<UserRef, null, void, UserPreferences>,
	EditPreferences: { method: "PATCH", path: "/user/{id}/preferences", auth: "account", dangerous: false, body: "json", response: "json" } as Endpoint<UserRef, null, PreferencesUpdate, UserPreferences>,
	ChangeAvatar: { method: "PATCH", path: "/user/{id}/icon", auth: "account", dangerous: false, body: "raw", response: "empty" } as Endpoint<UserRef, IconFormat, Blob, void>,
	RemoveAvatar: { method: "DELETE", path: "/user/{id}/icon", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<UserRef, null, void, void>,
	ListUserNotifications: { method: "GET", path: "/user/{id}/notifications", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<UserRef, null, void, Array<Notification>>,
	GetNotification: { method: "GET", path: "/notification/{id}", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<NotificationRef, null, void, Notification>,
	ReadNotification: { method: "PATCH", path: "/notification/{id}", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<NotificationRef, null, void, void>,
	DeleteNotification: { method: "DELETE", path: "/notification/{id}", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<NotificationRef, null, void, void>,
	GetNotifications: { method: "GET", path: "/notifications", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<null, IdList, void, Array<Notification>>,
	ReadNotifications: { method: "PATCH", path: "/notifications", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<null, IdList, void, void>,
	DeleteNotifications: { method: "DELETE", path: "/notifications", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<null, IdList, void, void>,
	FriendsSocket: { method: "GET", path: "/launcher_socket", auth: "account", dangerous: false, body: "empty", response: "upgrade" } as Endpoint<null, SocketCode, void, { receive: FriendsSocketMessage, send: ClientToServerMessage }>,
}

export type Globals = { tax_compliance_thresholds: Record<string, number>, captcha_enabled: boolean, max_user_icon_size: number, max_project_icon_size: number, max_organization_icon_size: number, max_collection_icon_size: number, max_oauth_client_icon_size: number, max_gallery_image_size: number, };

export type NewAccount = { username: string, password: string, email: string, account_consent?: boolean, 
/**
 * Also issue the App a session of its own.
 */
app_session?: boolean, };

export type LoginCredentials = { 
/**
 * The username or the email.
 */
username: string, password: string, app_session?: boolean, };

export type TwoFactorLogin = { code: string, flow: string, app_session?: boolean, };

export type TwoFactorRequired = "2fa_required";

export type TwoFactorChallenge = { error: TwoFactorRequired, description: string, flow: string, };

export type LoginAnswer = TwoFactorChallenge | AuthSession;

export type UserSession = { session: Never | null, id: string, user_id: string, created: string, last_login: string, expires: string, 
/**
 * Until when [`RefreshSession`] can rotate it.
 */
refresh_expires: string, user_agent: string, ip: string, os: Never | null, platform: Never | null, city: Never | null, country: Never | null, 
/**
 * Whether it is the session making the request.
 */
current: boolean, };

export type AuthSession = { session: string, app_session?: string, id: string, user_id: string, created: string, last_login: string, expires: string, 
/**
 * Until when [`RefreshSession`] can rotate it.
 */
refresh_expires: string, user_agent: string, ip: string, os: Never | null, platform: Never | null, city: Never | null, country: Never | null, 
/**
 * Whether it is the session making the request.
 */
current: boolean, };

export type SessionRef = { id: string, };

export type PasswordReset = { 
/**
 * The username or the email.
 */
username: string, };

export type PasswordUpdate = { flow?: string, old_password?: string, new_password?: string, };

export type TwoFactorSecret = { 
/**
 * Base32, for the authenticator.
 */
secret: string, flow: string, };

export type TwoFactorSetup = { code: string, flow: string, };

export type BackupCodes = { backup_codes: Array<string>, };

export type TwoFactorCode = { code: string, };

export type EmailVerification = { flow: string, };

export type EmailUpdate = { email: string, };

export type LockReason = { reason: string, };

export type ForcedPasswordReset = { email?: string, };

/** The backend's account routes, in labrinth's shapes. */
export const accounts = {
	GetGlobals: { method: "GET", path: "/globals", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Globals>,
	CreateAccount: { method: "POST", path: "/auth/create", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, NewAccount, AuthSession>,
	ValidateNewAccount: { method: "POST", path: "/auth/create/validate", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, NewAccount, void>,
	LogIn: { method: "POST", path: "/auth/login", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, LoginCredentials, LoginAnswer>,
	LogInTwoFactor: { method: "POST", path: "/auth/login/2fa", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, TwoFactorLogin, AuthSession>,
	RefreshSession: { method: "POST", path: "/session/refresh", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, AuthSession>,
	ListSessions: { method: "GET", path: "/session/list", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<UserSession>>,
	DeleteSession: { method: "DELETE", path: "/session/{id}", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<SessionRef, null, void, void>,
	RequestPasswordReset: { method: "POST", path: "/auth/password/reset", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, PasswordReset, void>,
	ChangePassword: { method: "PATCH", path: "/auth/password", auth: "account", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, PasswordUpdate, void>,
	StartTwoFactorSetup: { method: "POST", path: "/auth/2fa/get_secret", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, TwoFactorSecret>,
	EnableTwoFactor: { method: "POST", path: "/auth/2fa", auth: "account", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, TwoFactorSetup, BackupCodes>,
	DisableTwoFactor: { method: "DELETE", path: "/auth/2fa", auth: "account", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, TwoFactorCode, void>,
	VerifyEmail: { method: "POST", path: "/auth/email/verify", auth: "none", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, EmailVerification, void>,
	ResendVerifyEmail: { method: "POST", path: "/auth/email/resend_verify", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<null, null, void, void>,
	ChangeEmail: { method: "PATCH", path: "/auth/email", auth: "account", dangerous: false, body: "json", response: "empty" } as Endpoint<null, null, EmailUpdate, void>,
	GetCurrentUser: { method: "GET", path: "/user", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, User>,
	ListPasskeys: { method: "GET", path: "/auth/passkey", auth: "account", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<Never>>,
	LockUser: { method: "PUT", path: "/admin/user/{id}/lock", auth: "account", dangerous: false, body: "json", response: "empty" } as Endpoint<UserRef, null, LockReason, void>,
	UnlockUser: { method: "DELETE", path: "/admin/user/{id}/lock", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<UserRef, null, void, void>,
	RevokeUserSessions: { method: "DELETE", path: "/admin/user/{id}/sessions", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<UserRef, null, void, void>,
	ForcePasswordReset: { method: "POST", path: "/admin/user/{id}/password-reset", auth: "account", dangerous: false, body: "json", response: "empty" } as Endpoint<UserRef, null, ForcedPasswordReset, void>,
	ResetUserTwoFactor: { method: "DELETE", path: "/admin/user/{id}/2fa", auth: "account", dangerous: false, body: "empty", response: "empty" } as Endpoint<UserRef, null, void, void>,
}

export type ScenarioLogin = { username: string, };

export type EmailKind = "verify_email" | "reset_password";

export type OutboxEmail = { id: string, recipient: string, kind: EmailKind, flow: string, created: string, };

/** The local dev backend's routes. */
export const dev = {
	Session: { method: "POST", path: "/_dev/session", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, ScenarioLogin, AuthSession>,
	Outbox: { method: "GET", path: "/_dev/outbox", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<OutboxEmail>>,
}

/** The backend's routes outside accounts, cores, dev, sharing and social, which have their own consts. */
export const backend = {
	CheckHealth: { method: "GET", path: "/health", auth: "none", dangerous: false, body: "empty", response: "empty" } as Endpoint<null, null, void, void>,
}

/** The header a dangerous route's fresh ticket travels in. */
export const TICKET_HEADER = "x-amberite-ticket"

export type Health = { 
/**
 * The `PROTOCOL_VERSION` the Core speaks. One that reports none speaks version 1.
 */
protocol?: number, version: string, core_id: string, };

export type ServerId = { id: string, };

export type MemberId = { 
/**
 * The server.
 */
id: string, user_id: string, };

export type SessionId = { id: string, };

export type CoreRole = "viewer" | "editor";

export type CoreUser = { id: string, name: string, avatar_url: string | null, };

export type Member = { user: CoreUser, role: CoreRole, 
/**
 * Unix seconds.
 */
added_at: number, };

export type ServerAccess = { 
/**
 * Null until the Core is paired.
 */
owner: CoreUser | null, members: Array<Member>, };

export type NewMember = { 
/**
 * Shown until the invited user signs in and their own ticket names them.
 */
user: CoreUser, role: CoreRole, };

export type MemberUpdate = { role: CoreRole, };

export type MyAccess = { user: CoreUser, 
/**
 * The owner of this Core, with every role on every server.
 */
owner: boolean, 
/**
 * Empty for the owner.
 */
servers: Array<ServerRole>, };

export type ServerRole = { server_id: string, role: CoreRole, };

export type NewSession = { 
/**
 * A ticket the backend signed for this Core, under five minutes old and never used.
 */
ticket: string, 
/**
 * Up to 64 characters.
 */
device_name: string, };

export type SignedIn = { 
/**
 * 256 random bits, sent as the bearer. The Core stores only its hash, and the App keeps it in
 * the OS keychain, out of the WebView.
 */
token: string, session: CoreSession, };

export type CoreSession = { 
/**
 * Public id for listing and revoking. Not the token.
 */
id: string, user: CoreUser, device_name: string, 
/**
 * Unix seconds.
 */
created_at: number, 
/**
 * Unix seconds, written at most once an hour.
 */
last_used_at: number, 
/**
 * Unix seconds. The session ends at this time unless it is used before then; it never
 * lasts past a year from `created_at`.
 */
expires_at: number, 
/**
 * The session making this request.
 */
current: boolean, };

export type ActivityQuery = { 
/**
 * Only this server's entries.
 */
server_id?: string, 
/**
 * Entries with an id below this, for the next page.
 */
before?: number, 
/**
 * At most 100. 50 when left out.
 */
limit?: number, };

export type ActivityPage = { entries: Array<ActivityEntry>, 
/**
 * Pass as `before` for the next page. Null on the last page.
 */
next_before: number | null, };

export type ActivityEntry = { id: number, 
/**
 * Unix seconds.
 */
at: number, 
/**
 * Null for the local CLI, which is the owner.
 */
user: CoreUser | null, 
/**
 * Null for actions on the Core itself.
 */
server_id: string | null, action: Action, };

export type Action = { "type": "invited", user_id: string, role: CoreRole, } | { "type": "role_changed", user_id: string, role: CoreRole, } | { "type": "removed", user_id: string, } | { "type": "signed_in", session_id: string, device_name: string, } | { "type": "session_revoked", session_id: string, } | { "type": "server_created" } | { "type": "server_deleted" } | { "type": "server_started" } | { "type": "server_stopped" } | { "type": "server_restarted" } | { "type": "server_killed" } | { "type": "console_command", command: string, } | { "type": "settings_changed" } | { "type": "content_changed" } | { "type": "backup_created", backup_id: string, } | { "type": "backup_restored", backup_id: string, } | { "type": "backup_deleted", backup_id: string, } | { "type": "backup_renamed", backup_id: string, } | { "type": "backup_downloaded", backup_id: string, } | { "type": "world_downloaded" } | { "type": "eula_accepted" } | { "type": "core_updated", version: string, } | { "type": "core_removed" };

export type PairingStatus = { paired: boolean, 
/**
 * The code to enter in the App, like `MZUX-D576`. Null once paired, and while the Core is
 * still getting one from Amberite.
 */
code: string | null, 
/**
 * Unix seconds.
 */
expires_at: number | null, };

export type NewDownloadLink = { 
/**
 * The exact path and query the link will GET, such as
 * `/servers/{id}/v1/worlds/{id}/files/contents-raw?path=server.properties`.
 */
path: string, };

export type DownloadLink = { token: string, };

export type Topic = { "type": "instance", id: string, } | { "type": "installs" } | { "type": "console", id: string, } | { "type": "server", id: string, } | { "type": "backups", id: string, };

export type ClientMessage = { "type": "subscribe", topic: Topic, } | { "type": "unsubscribe", topic: Topic, };

export type CoreEvent = { "type": "console", "payload": { server: string, line: string, } } | { "type": "power", "payload": { server: string, state: PowerState, uptime: number | null, } } | { "type": "backup", "payload": { server: string, id: string, task: BackupTask, state: BackupTaskState, progress: number, start_time: number | null, finish_time: number | null, } } | { "type": "ended", "payload": { topic: Topic, error: ApiError, } } | { "type": "resync" };

export type PowerState = "running" | "stopped" | "starting" | "stopping" | "crashed";

export type BackupTask = "create" | "restore";

export type BackupTaskState = "ongoing" | "done" | "failed";

export type CoreServer = { id: string, name: string, 
/**
 * Null until the Core is paired.
 */
owner_id: string | null, 
/**
 * The caller's role. Null for the owner, who has every role.
 */
role: CoreRole | null, 
/**
 * Its loader or pack is installing now.
 */
installing: boolean, 
/**
 * `vanilla`, `fabric`, `quilt`, `forge`, `neoforge` or `paper`. Null until installed.
 */
loader: string | null, loader_version: string | null, game_version: string | null, net: ServerNet, power: ServerPower, 
/**
 * The App instance it follows.
 */
linked_instance: LinkedInstance | null, 
/**
 * Whether players can join from the internet. Null while it does not run.
 */
reachability: ReachabilityReport | null, 
/**
 * Who is online. Null while it does not run.
 */
players: Array<string> | null, memory_mb: number, 
/**
 * Unix seconds.
 */
created_at: number, update_channel: UpdateChannel, 
/**
 * The owner accepted the Minecraft EULA for every server on this Core. None starts before.
 */
eula_accepted: boolean, };

export type ServerNet = { 
/**
 * With `port`, where players join from the internet: the router's address when it forwarded
 * another port than the game port, otherwise the Core's.
 */
ip: string | null, port: number, 
/**
 * Where the server is on the internet, while it runs and is forwarded or public.
 */
outside: JoinAddress | null, 
/**
 * The port the server listens on.
 */
game_port: number, };

export type JoinAddress = { ip: string, port: number, };

export type ServerPower = "offline" | "starting" | "running" | "stopping" | "crashed" | "killed";

export type LinkedInstance = { 
/**
 * The App's id for the instance.
 */
id: string, name: string, state: LinkedPackState, error: string | null, shared_instance_id: string | null, 
/**
 * The version of the instance's share the server runs.
 */
version: number | null, };

export type LinkedPackState = "up_to_date" | "downloading" | "pending" | "applying" | "failed";

export type UpdateChannel = "release" | "beta" | "alpha";

export type NewServer = { 
/**
 * 1 to 64 characters.
 */
name: string, };

export type ServerUpdate = { 
/**
 * 1 to 64 characters, trimmed.
 */
name: string | null, update_channel: UpdateChannel | null, };

export type PowerAction = "start" | "stop" | "restart" | "kill";

export type PowerChange = { action: PowerAction, };

export type LoaderInstall = { 
/**
 * As [`CoreServer::loader`] names it.
 */
loader: string, 
/**
 * Null for the newest.
 */
loader_version: string | null, game_version: string, };

export type SharePush = { 
/**
 * Files to send with this version. Files sent earlier stay shared while the server has
 * them, unless they are in `removed_paths`.
 */
file_paths: Array<string>, removed_paths: Array<string>, 
/**
 * Restart a running server once the version is out.
 */
restart: boolean, };

export type SharedServer = { shared_instance_id: string, version: number | null, restarted: boolean, };

export type ServerSetup = { 
/**
 * The Java major it runs. Null until installed.
 */
java_version: number | null, 
/**
 * The command line it runs, as people read it. Null until installed.
 */
invocation: string | null, 
/**
 * The share it reads: its instance's when it follows one, else its own once it is shared.
 */
shared_instance_id: string | null, 
/**
 * It has changes its own share's latest version does not. Always false while it follows an
 * instance.
 */
shared_instance_needs_update: boolean, 
/**
 * The Core machine's CPU threads.
 */
cpus: number, };

export type JavaMajor = { 
/**
 * A Java major the Core installs (8, 11, 17, 21 or 25), or null for the one the Minecraft
 * version needs.
 */
major: number | null, };

export type ServerProperties = { properties: { [key in string]: string }, };

export type PropertiesUpdate = { 
/**
 * Null removes the key.
 */
properties: { [key in string]: string | null }, };

export type ServerBackups = { backups: Array<ServerBackup>, schedule: BackupSchedule, 
/**
 * Unix seconds. Null while scheduled backups are off.
 */
next_scheduled_at: number | null, };

export type BackupSchedule = { interval_hours: number, keep: number, slots: number, };

export type ServerBackup = { id: string, name: string, 
/**
 * Unix seconds.
 */
created_at: number, state: BackupState, kind: BackupKind, 
/**
 * Unix seconds. Set on scheduled backups that hold a slot: when later scheduled backups push
 * it out.
 */
rotates_at: number | null, operations: Array<BackupOperation>, };

export type BackupState = "queued" | "running" | "done" | "failed";

export type BackupKind = "manual" | "scheduled" | "pre_update";

export type BackupOperation = { id: number, kind: OperationKind, state: OperationState, 
/**
 * Unix seconds, as are the other times.
 */
queued_at: number, started_at: number | null, ended_at: number | null, error: string | null, 
/**
 * Someone dismissed it once it ended.
 */
acked: boolean, 
/**
 * Who started it. Null for the schedule.
 */
user: CoreUser | null, };

export type OperationKind = "create" | "restore";

export type OperationState = "queued" | "running" | "done" | "failed" | "cancelled";

export type NewBackup = { 
/**
 * 1 to 64 characters.
 */
name: string, };

export type CreatedBackup = { id: string, };

export type BackupUpdate = { name: string, };

export type BackupIds = { ids: Array<string>, };

export type ScheduleUpdate = { 
/**
 * Hourly at most, every 744 hours at least.
 */
interval_hours: number | null, 
/**
 * 0 turns scheduled backups off.
 */
keep: number | null, 
/**
 * Keeping fewer than there are deletes the oldest scheduled backups. Without this the change
 * is only described; with it, it is the owner's dangerous action.
 */
delete_excess: boolean, };

export type ScheduleChange = { applied: boolean, 
/**
 * The scheduled backups the change deletes, or deleted once applied.
 */
excess: Array<string>, };

export type BackupRef = { 
/**
 * The server.
 */
id: string, backup_id: string, };

export type OperationRef = { 
/**
 * The server.
 */
id: string, operation_id: number, };

export type UpdateStatus = { 
/**
 * The running version.
 */
version: string, 
/**
 * Which release files this Core installs, such as `linux-x64`. Null on a platform Copal is
 * not released for, which cannot update.
 */
target: string | null, channel: Channel, 
/**
 * Updates install by themselves. Off: only when the owner asks.
 */
auto_update: boolean, 
/**
 * The newest release on the channel, as of the last check.
 */
latest: string | null, 
/**
 * `latest` is newer than the running version.
 */
update_available: boolean, 
/**
 * Unix seconds.
 */
checked_at: number | null, 
/**
 * Why the last check failed.
 */
check_error: string | null, 
/**
 * The version a rollback goes back to.
 */
previous: string | null, 
/**
 * The last version that was tried here, failed its trial, and was rolled back from. While
 * it is the newest release, `update_available` is false and applying it needs `retry`.
 */
failed: string | null, 
/**
 * How `failed` failed. Null when an older launcher did not record it.
 */
failed_reason: FailedTrial | null, };

export type Channel = "stable" | "beta" | "alpha";

export type FailedTrial = "not_started" | "exited" | "no_answer";

export type UpdateSettings = { channel?: Channel, auto_update?: boolean, };

export type ApplyUpdate = { 
/**
 * Install the channel's release even when it is older. Only the copal CLI may.
 */
allow_downgrade?: boolean, 
/**
 * Install the channel's release even when it already failed its trial on this Core.
 */
retry?: boolean, };

export type Restarting = { 
/**
 * The version the Core restarts into.
 */
version: string, };

export type StorageSettings = { 
/**
 * Offer storage to other people's shares. Off, the Core runs privately.
 */
enabled?: boolean, 
/**
 * The most the store may hold.
 */
cap?: number, 
/**
 * The most it may serve a day.
 */
egress_cap?: number, };

export type StorageStatus = { 
/**
 * Offers storage to other people's shares. Off, the Core runs privately.
 */
enabled: boolean, cap: number, egress_cap: number, 
/**
 * What the stored versions take, counting each object once.
 */
used: number, 
/**
 * Free on the store's disk.
 */
free: number, 
/**
 * Served today, UTC.
 */
egress_today: number, 
/**
 * The Core is installed as a service. The pool places shares only on Cores that are.
 */
service: boolean, 
/**
 * Versions the store holds.
 */
versions: number, };

export type Eula = { 
/**
 * The EULA's address.
 */
url: string, 
/**
 * Unix seconds. Null until the owner accepts it.
 */
accepted_at: number | null, };

export type LogsQuery = { 
/**
 * At most 1000. 100 when left out.
 */
lines?: number, };

export type CoreLogs = { 
/**
 * Oldest first.
 */
lines: Array<string>, };

export type Machine = { 
/**
 * This machine's memory. Null when the OS does not say.
 */
memory_mb: number | null, 
/**
 * The memory every server together may use.
 */
assigned_mb: number, 
/**
 * Set when the servers together may use more than the machine has.
 */
warning: string | null, 
/**
 * Whether the App can reach the Core's API from the internet.
 */
api_reachability: ReachabilityReport | null, };

export type ReachabilityReport = { state: Reach, 
/**
 * The cause in one line.
 */
title: string, 
/**
 * The one thing to do next, when there is something.
 */
next_step: string | null, 
/**
 * Each test, in order.
 */
checks: Array<ReachabilityCheck>, found: Array<string>, 
/**
 * Every step of the fix, `next_step` first.
 */
fix: Array<string>, 
/**
 * Unix seconds.
 */
checked_at: number, };

export type Reach = "reachable" | "unverified" | "unreachable";

export type ReachabilityCheck = { 
/**
 * Null when the test was not run or could not tell.
 */
passed: boolean | null, text: string, };

export type Performance = { 
/**
 * The most memory Java may use. Java starts with less and grows to it.
 */
memory_mb: number, 
/**
 * The JVM flags the server starts with.
 */
flags: Array<string>, 
/**
 * `flags` are the server's own rather than the Core's tuned ones.
 */
custom_flags: boolean, restarts: Restarts, };

export type Memory = { memory_mb: number, };

export type JvmFlags = { 
/**
 * The server's own flags, each starting with `-`. Null goes back to the tuned ones.
 */
flags: Array<string> | null, };

export type Restarts = { 
/**
 * Start the server again after a crash. On by default.
 */
crash: boolean, 
/**
 * A restart at a set time, with an in-game countdown.
 */
schedule: Schedule | null, 
/**
 * Experimental: restart once nobody is online and the server has run this many hours,
 * after a minute's warning in the game.
 */
smart_after_hours: number | null, };

export type Schedule = { "every": "daily", at: string, } | { "every": "weekly", on: Weekday, at: string, };

export type Weekday = "monday" | "tuesday" | "wednesday" | "thursday" | "friday" | "saturday" | "sunday";

export type ConsoleCommand = { 
/**
 * One line, as typed into the server console, such as `say hello`.
 */
command: string, };

export type JavaPath = { path: string | null, };

export type ServerInsight = { 
/**
 * Who is online, from the Core's health check and join and leave lines. Empty while the
 * server does not run.
 */
players: Array<string>, 
/**
 * The health check answered this run, so `players` is checked, not only what join and
 * leave lines said.
 */
players_checked: boolean, 
/**
 * The server stopped answering the Core's health check, so it is probably frozen. Offer a
 * restart.
 */
frozen: boolean, 
/**
 * Unix seconds of the server's last answer to the health check.
 */
answered_at: number | null, 
/**
 * The Core stopped starting the server again after it crashed 3 times in 10 minutes.
 */
crash_restart_stopped: boolean, 
/**
 * The last crash, with the console lines before it for editors and up.
 */
last_crash: Crash | null, 
/**
 * Whether players can join from the internet. Null while the server does not run or has
 * not been checked yet.
 */
reachability: ReachabilityReport | null, };

export type Crash = { 
/**
 * Unix seconds.
 */
at: number, exit_code: number | null, 
/**
 * The console lines before the crash. Null for viewers: they can hold chat, addresses,
 * and anything a mod printed, which the console route keeps to editors.
 */
last_lines: string | null, };

/** A Core's own routes. */
export const core = {
	CheckHealth: { method: "GET", path: "/health", auth: "none", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Health>,
	SignIn: { method: "POST", path: "/sessions", auth: "none", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, NewSession, SignedIn>,
	ListSessions: { method: "GET", path: "/sessions", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<CoreSession>>,
	SignOut: { method: "DELETE", path: "/sessions/current", auth: "session", dangerous: false, body: "empty", response: "empty" } as Endpoint<null, null, void, void>,
	RevokeSession: { method: "DELETE", path: "/sessions/{id}", auth: "session", dangerous: true, body: "empty", response: "empty" } as Endpoint<SessionId, null, void, void>,
	GetMyAccess: { method: "GET", path: "/access/me", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, MyAccess>,
	ListMembers: { method: "GET", path: "/servers/{id}/access", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<ServerId, null, void, ServerAccess>,
	InviteMember: { method: "POST", path: "/servers/{id}/access", auth: "session", dangerous: true, body: "json", response: "json" } as Endpoint<ServerId, null, NewMember, Member>,
	ChangeRole: { method: "PATCH", path: "/servers/{id}/access/{user_id}", auth: "session", dangerous: true, body: "json", response: "json" } as Endpoint<MemberId, null, MemberUpdate, Member>,
	RemoveMember: { method: "DELETE", path: "/servers/{id}/access/{user_id}", auth: "session", dangerous: true, body: "empty", response: "empty" } as Endpoint<MemberId, null, void, void>,
	CreateDownloadLink: { method: "POST", path: "/links", auth: "session", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, NewDownloadLink, DownloadLink>,
	ListActivity: { method: "GET", path: "/activity", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, ActivityQuery, void, ActivityPage>,
	GetPairing: { method: "GET", path: "/pairing", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, PairingStatus>,
	RequestPairingCode: { method: "POST", path: "/pairing/code", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, PairingStatus>,
	Unpair: { method: "DELETE", path: "/pairing", auth: "session", dangerous: true, body: "empty", response: "empty" } as Endpoint<null, null, void, void>,
	GetUpdate: { method: "GET", path: "/update", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, UpdateStatus>,
	ChangeUpdateSettings: { method: "PATCH", path: "/update", auth: "session", dangerous: true, body: "json", response: "json" } as Endpoint<null, null, UpdateSettings, UpdateStatus>,
	CheckForUpdate: { method: "POST", path: "/update/check", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, UpdateStatus>,
	InstallUpdate: { method: "POST", path: "/update/apply", auth: "session", dangerous: true, body: "json", response: "json" } as Endpoint<null, null, ApplyUpdate, Restarting>,
	RollBack: { method: "POST", path: "/update/rollback", auth: "session", dangerous: true, body: "empty", response: "json" } as Endpoint<null, null, void, Restarting>,
	GetStorage: { method: "GET", path: "/storage", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, StorageStatus>,
	ChangeStorage: { method: "PATCH", path: "/storage", auth: "session", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, StorageSettings, StorageStatus>,
	GetEula: { method: "GET", path: "/eula", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Eula>,
	AcceptEula: { method: "POST", path: "/eula", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Eula>,
	GetLogs: { method: "GET", path: "/logs", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, LogsQuery, void, CoreLogs>,
	GetMachine: { method: "GET", path: "/machine", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Machine>,
	ListServers: { method: "GET", path: "/servers", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<null, null, void, Array<CoreServer>>,
	CreateServer: { method: "POST", path: "/servers", auth: "session", dangerous: false, body: "json", response: "json" } as Endpoint<null, null, NewServer, ServerId>,
	GetServer: { method: "GET", path: "/servers/{id}", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<ServerId, null, void, CoreServer>,
	ChangeServer: { method: "PATCH", path: "/servers/{id}", auth: "session", dangerous: false, body: "json", response: "empty" } as Endpoint<ServerId, null, ServerUpdate, void>,
	DeleteServer: { method: "DELETE", path: "/servers/{id}", auth: "session", dangerous: true, body: "empty", response: "empty" } as Endpoint<ServerId, null, void, void>,
	ChangePower: { method: "POST", path: "/servers/{id}/power", auth: "session", dangerous: false, body: "json", response: "empty" } as Endpoint<ServerId, null, PowerChange, void>,
	InstallLoader: { method: "POST", path: "/servers/{id}/reinstall", auth: "session", dangerous: false, body: "json", response: "empty" } as Endpoint<ServerId, null, LoaderInstall, void>,
	InstallModpackFile: { method: "POST", path: "/servers/{id}/modpack-file", auth: "session", dangerous: false, body: "stream", response: "empty" } as Endpoint<ServerId, null, Blob, void>,
	ShareServer: { method: "POST", path: "/servers/{id}/share", auth: "session", dangerous: false, body: "json", response: "json" } as Endpoint<ServerId, null, SharePush, SharedServer>,
	GetSetup: { method: "GET", path: "/servers/{id}/setup", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<ServerId, null, void, ServerSetup>,
	SetJavaVersion: { method: "PUT", path: "/servers/{id}/java-version", auth: "session", dangerous: false, body: "json", response: "empty" } as Endpoint<ServerId, null, JavaMajor, void>,
	GetProperties: { method: "GET", path: "/servers/{id}/properties", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<ServerId, null, void, ServerProperties>,
	ChangeProperties: { method: "PATCH", path: "/servers/{id}/properties", auth: "session", dangerous: false, body: "json", response: "json" } as Endpoint<ServerId, null, PropertiesUpdate, ServerProperties>,
	ListBackups: { method: "GET", path: "/servers/{id}/backups", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<ServerId, null, void, ServerBackups>,
	CreateBackup: { method: "POST", path: "/servers/{id}/backups", auth: "session", dangerous: false, body: "json", response: "json" } as Endpoint<ServerId, null, NewBackup, CreatedBackup>,
	ChangeBackupSchedule: { method: "PATCH", path: "/servers/{id}/backups/schedule", auth: "session", dangerous: false, body: "json", response: "json" } as Endpoint<ServerId, null, ScheduleUpdate, ScheduleChange>,
	DeleteBackups: { method: "POST", path: "/servers/{id}/backups/delete", auth: "session", dangerous: true, body: "json", response: "empty" } as Endpoint<ServerId, null, BackupIds, void>,
	RenameBackup: { method: "PATCH", path: "/servers/{id}/backups/{backup_id}", auth: "session", dangerous: false, body: "json", response: "empty" } as Endpoint<BackupRef, null, BackupUpdate, void>,
	DeleteBackup: { method: "DELETE", path: "/servers/{id}/backups/{backup_id}", auth: "session", dangerous: true, body: "empty", response: "empty" } as Endpoint<BackupRef, null, void, void>,
	RestoreBackup: { method: "POST", path: "/servers/{id}/backups/{backup_id}/restore", auth: "session", dangerous: true, body: "empty", response: "empty" } as Endpoint<BackupRef, null, void, void>,
	RetryBackup: { method: "POST", path: "/servers/{id}/backups/{backup_id}/retry", auth: "session", dangerous: false, body: "empty", response: "empty" } as Endpoint<BackupRef, null, void, void>,
	AckOperation: { method: "POST", path: "/servers/{id}/backups/operations/{operation_id}/ack", auth: "session", dangerous: false, body: "empty", response: "empty" } as Endpoint<OperationRef, null, void, void>,
	CancelOperation: { method: "POST", path: "/servers/{id}/backups/operations/{operation_id}/cancel", auth: "session", dangerous: false, body: "empty", response: "empty" } as Endpoint<OperationRef, null, void, void>,
	GetPerformance: { method: "GET", path: "/servers/{id}/performance", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<ServerId, null, void, Performance>,
	SetMemory: { method: "PUT", path: "/servers/{id}/memory", auth: "session", dangerous: false, body: "json", response: "json" } as Endpoint<ServerId, null, Memory, Performance>,
	SetFlags: { method: "PUT", path: "/servers/{id}/flags", auth: "session", dangerous: false, body: "json", response: "json" } as Endpoint<ServerId, null, JvmFlags, Performance>,
	SetRestarts: { method: "PUT", path: "/servers/{id}/restarts", auth: "session", dangerous: false, body: "json", response: "json" } as Endpoint<ServerId, null, Restarts, Performance>,
	SendCommand: { method: "POST", path: "/servers/{id}/console", auth: "session", dangerous: false, body: "json", response: "empty" } as Endpoint<ServerId, null, ConsoleCommand, void>,
	SetJava: { method: "PUT", path: "/servers/{id}/java", auth: "session", dangerous: false, body: "json", response: "empty" } as Endpoint<ServerId, null, JavaPath, void>,
	GetInsight: { method: "GET", path: "/servers/{id}/insight", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<ServerId, null, void, ServerInsight>,
	CheckReachability: { method: "POST", path: "/servers/{id}/reachability", auth: "session", dangerous: false, body: "empty", response: "json" } as Endpoint<ServerId, null, void, ServerInsight>,
}

export type VersionList = { objects: Array<ListedObject>, };

export type ListedObject = { hash: Hash, size: number, };

export type VersionHash = { root: Hash, };

export type VersionObject = { 
/**
 * The version's hash.
 */
root: Hash, 
/**
 * The object's hash.
 */
object: Hash, };

export type ObjectHashes = { objects: Array<Hash>, };

export type UploadStatus = { missing: Array<Hash>, 
/**
 * The version is stored whole, so nothing is left to send.
 */
complete: boolean, };

export type UploadedVersion = { complete: boolean, 
/**
 * What the version still lacks, to send in another upload.
 */
missing?: Array<Hash>, };

export type PulledVersion = { complete: boolean, };

/** The store's routes, under a Core's `/store/`. */
export const store = {
	ListVersion: { method: "GET", path: "/store/versions/{root}", auth: "ticket", dangerous: false, body: "empty", response: "json" } as Endpoint<VersionHash, null, void, VersionList>,
	UploadVersion: { method: "POST", path: "/store/versions/{root}", auth: "ticket", dangerous: false, body: "stream", response: "json" } as Endpoint<VersionHash, null, Blob, UploadedVersion>,
	MissingObjects: { method: "POST", path: "/store/versions/{root}/have", auth: "ticket", dangerous: false, body: "json", response: "json" } as Endpoint<VersionHash, null, ObjectHashes, UploadStatus>,
	GetObject: { method: "GET", path: "/store/versions/{root}/objects/{object}", auth: "ticket", dangerous: false, body: "empty", response: "stream" } as Endpoint<VersionObject, null, void, Response>,
	PullVersion: { method: "POST", path: "/store/versions/{root}/pull", auth: "ticket", dangerous: false, body: "empty", response: "json" } as Endpoint<VersionHash, null, void, PulledVersion>,
}
