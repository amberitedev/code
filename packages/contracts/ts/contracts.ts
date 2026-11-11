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
