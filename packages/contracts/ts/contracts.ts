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

export type ErrorCode = "invalid_input" | "invalid_credentials" | "unauthorized" | "forbidden" | "not_found" | "method_not_allowed" | "conflict" | "rate_limited" | "internal_error" | "unsupported" | "account_locked" | "already_signed_in" | "code_expired" | "configuration_error" | "core_removed" | "duplicate_email" | "invalid_address" | "mail_error" | "no_storage" | "rate_limit" | "tickets_unavailable" | "too_many_attempts" | "too_many_invites" | "too_many_requests" | "unsupported_operation" | "username_taken" | "weak_password" | "wrong_code" | "blacklisted" | "core_not_found" | "current_version" | "endpoint_not_found" | "icon_not_found" | "instance_not_found" | "invite_invalid" | "invite_not_found" | "members_only" | "not_member" | "not_moderator" | "not_own_list" | "not_owner" | "push_in_flight" | "quarantined" | "share_too_large" | "storage_quota" | "too_many_files" | "version_not_found" | "already_friends" | "cannot_friend_self" | "own_friend_request" | "ticket_required" | "ticket_invalid" | "ticket_wrong_core" | "ticket_wrong_kind" | "ticket_expired" | "ticket_reused" | "clock_wrong" | "not_invited" | "disk_full" | "egress_spent" | "eula_not_accepted" | "not_managed" | "already_running" | "installing" | "no_ports" | "shared_instance_deleted" | "shared_instance_access_revoked" | "shared_instance_quarantined" | "shared_instances_api_error" | "shared_instance_file_not_allowed" | "shared_instance_owner_behind" | "shared_instance_limit_reached" | "pool_no_storage" | "pool_push_in_flight" | "pool_too_large" | "pool_disk_full" | "pool_egress_spent" | "pool_push_unconfirmed" | "pool_files_broken" | "pool_files_unavailable" | "hash_error" | "login_canceled" | "not_logged_in" | "no_loader_version" | "launcher_error" | "java_error" | "metadata_error" | "network_error" | "archive_error" | "path_error" | "filesystem_error" | "parse_error" | "database_error" | "directory_move_no_space" | "directory_move_permission_denied" | "directory_move_failed" | "permission_denied" | "api_error" | "minecraft_auth_no_profile" | "minecraft_auth_token_expired" | "minecraft_auth_clock_out_of_sync" | "minecraft_auth_rate_limited" | "minecraft_auth_service_unavailable" | "minecraft_auth_connection_refused" | "minecraft_auth_network" | "minecraft_auth_age_verification" | "minecraft_auth_banned" | "minecraft_auth_restricted" | "minecraft_auth_no_xbox_profile" | "minecraft_auth_terms_not_accepted" | "minecraft_auth_region_unavailable" | "minecraft_auth_adult_verification" | "minecraft_auth_underage" | "minecraft_auth_xsts_rejected" | "minecraft_auth_error" | "engine_unavailable" | "external_save_failed" | "sign_in_cancelled" | "unknown";

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
 * `viewer` or `editor`, shown in the invite.
 */
role: string, };

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
