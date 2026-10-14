// Generated from apps/core by ts-rs. Do not edit.
// Regenerate: pnpm --filter @amberite/core types

export type ApiError = { error: ErrorCode, 
/**
 * Ready to show.
 */
description: string, };

export type ErrorCode = "unauthorized" | "forbidden" | "ticket_required" | "ticket_invalid" | "ticket_wrong_core" | "ticket_expired" | "ticket_reused" | "clock_wrong" | "not_invited" | "not_found" | "rate_limited" | "invalid_request";

export type Role = "viewer" | "editor";

export type User = { id: string, name: string, avatar_url: string | null, };

export type Member = { user: User, role: Role, 
/**
 * Unix seconds.
 */
added_at: number, };

export type ServerAccess = { owner: User, members: Array<Member>, };

export type Invite = { 
/**
 * Shown until the invited user signs in and their own ticket names them.
 */
user: User, role: Role, };

export type ChangeRole = { role: Role, };

export type MyAccess = { user: User, 
/**
 * The owner of this Core, with every role on every server.
 */
owner: boolean, 
/**
 * Empty for the owner.
 */
servers: Array<ServerRole>, };

export type ServerRole = { server_id: string, role: Role, };

export type SignIn = { 
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
 * 256 random bits. Kept in the OS keychain by the App's Rust side; the WebView never sees
 * it.
 */
token: string, session: Session, };

export type Session = { 
/**
 * Public id for listing and revoking. Not the token.
 */
id: string, user: User, device_name: string, 
/**
 * Unix seconds.
 */
created_at: number, 
/**
 * Unix seconds, written at most once an hour.
 */
last_used_at: number, 
/**
 * Unix seconds. The session ends at this time or 30 days after `last_used_at`, whichever
 * is first.
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
user: User | null, 
/**
 * Null for actions on the Core itself.
 */
server_id: string | null, action: Action, };

export type Action = { "type": "invited", user_id: string, role: Role, } | { "type": "role_changed", user_id: string, role: Role, } | { "type": "removed", user_id: string, } | { "type": "signed_in", session_id: string, device_name: string, } | { "type": "session_revoked", session_id: string, } | { "type": "server_created" } | { "type": "server_deleted" } | { "type": "server_started" } | { "type": "server_stopped" } | { "type": "server_restarted" } | { "type": "server_killed" } | { "type": "console_command", command: string, } | { "type": "settings_changed" } | { "type": "content_changed" } | { "type": "backup_created", backup_id: string, } | { "type": "backup_restored", backup_id: string, } | { "type": "backup_downloaded", backup_id: string, } | { "type": "world_downloaded" } | { "type": "eula_accepted" } | { "type": "core_updated", version: string, };

export type TicketClaims = { 
/**
 * The backend's issuer name.
 */
iss: string, 
/**
 * The user id.
 */
sub: string, name: string, avatar_url: string | null, 
/**
 * The Core id this ticket is for.
 */
aud: string, 
/**
 * Unix seconds.
 */
iat: number, 
/**
 * Unix seconds, five minutes after `iat`.
 */
exp: number, 
/**
 * Random one-time id. Core records it in the same write that uses the ticket.
 */
jti: string, };

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
cert_fingerprint: string, };

export type AccessDirectory = { user_ids: Array<string>, };

export type OwedCall = { "type": "grant", user_id: string, } | { "type": "drop", user_id: string, } | { "type": "report_state" };
