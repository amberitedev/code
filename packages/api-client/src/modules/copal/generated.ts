// Generated from apps/core by ts-rs. Do not edit.
// Regenerate: pnpm --filter @amberite/core types

export type ApiError = { error: ErrorCode, 
/**
 * Ready to show.
 */
description: string, };

export type ErrorCode = "unauthorized" | "forbidden" | "ticket_required" | "ticket_invalid" | "ticket_wrong_core" | "ticket_expired" | "ticket_reused" | "clock_wrong" | "not_invited" | "not_found" | "rate_limited" | "invalid_request" | "conflict" | "internal";

export type Role = "viewer" | "editor";

export type User = { id: string, name: string, avatar_url: string | null, };

export type Member = { user: User, role: Role, 
/**
 * Unix seconds.
 */
added_at: number, };

export type ServerAccess = { 
/**
 * Null until the Core is paired.
 */
owner: User | null, members: Array<Member>, };

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
user: User | null, 
/**
 * Null for actions on the Core itself.
 */
server_id: string | null, action: Action, };

export type Action = { "type": "invited", user_id: string, role: Role, } | { "type": "role_changed", user_id: string, role: Role, } | { "type": "removed", user_id: string, } | { "type": "signed_in", session_id: string, device_name: string, } | { "type": "session_revoked", session_id: string, } | { "type": "server_created" } | { "type": "server_deleted" } | { "type": "server_started" } | { "type": "server_stopped" } | { "type": "server_restarted" } | { "type": "server_killed" } | { "type": "console_command", command: string, } | { "type": "settings_changed" } | { "type": "content_changed" } | { "type": "backup_created", backup_id: string, } | { "type": "backup_restored", backup_id: string, } | { "type": "backup_deleted", backup_id: string, } | { "type": "backup_downloaded", backup_id: string, } | { "type": "world_downloaded" } | { "type": "eula_accepted" } | { "type": "core_updated", version: string, } | { "type": "core_removed" };

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
cert_fingerprint: string, 
/**
 * The servers shown to the people their instance is shared with. Empty until servers
 * follow shared instances (step 4).
 */
servers: Array<VisibleServer>, };

export type VisibleServer = { id: string, name: string, 
/**
 * The Minecraft address, `host:port`.
 */
address: string, shared_instance_id: string, };

export type AccessDirectory = { user_ids: Array<string>, };

export type Grant = { 
/**
 * The user id.
 */
user: string, server_id: string, server_name: string, role: Role, };

export type Remove = { user_id: string, server_id: string, keep_core: boolean, };

export type OwedCall = { "type": "grant" } & Grant | { "type": "remove" } & Remove | { "type": "report_state" };

export type EditedFiles = { files: Array<EditedFile>, };

export type EditedFile = { 
/**
 * Relative to the server folder, with forward slashes.
 */
path: string, 
/**
 * The owner deleted it (or disabled it) rather than changed it.
 */
deleted: boolean, 
/**
 * What the last pack update wants for it, waiting on a [`Decision`]; null when nothing waits.
 */
pending: PendingChange | null, };

export type PendingChange = "changed" | "removed";

export type Decision = { path: string, choice: Choice, };

export type Choice = "keep" | "replace";

export type JavaPath = { path: string | null, };

export type BackupSchedule = { interval_hours: number, keep: number, };

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

export type FailedTrial = "not_started" | "exited" | "no_answer";

export type Channel = "stable" | "beta" | "alpha";

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
 * The version Core restarts into.
 */
version: string, };

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
 * `flags` are the server's own rather than Core's tuned ones.
 */
custom_flags: boolean, restarts: Restarts, };

export type Memory = { memory_mb: number, };

export type SendCommand = { 
/**
 * One line, as typed into the server console, such as `say hello`.
 */
command: string, };

export type JvmFlags = { 
/**
 * The server's own flags, each starting with `-`. Null goes back to the tuned ones.
 */
flags: Array<string> | null, };

export type ServerInsight = { 
/**
 * Who is online, from join and leave lines. Empty while the server does not run.
 */
players: Array<string>, 
/**
 * `list` answered this run, so `players` is checked, not only what join and leave lines
 * said.
 */
players_checked: boolean, 
/**
 * The server stopped answering Core's probe, so it is probably frozen. Offer a restart.
 */
frozen: boolean, 
/**
 * Unix seconds of the server's last answer to the probe.
 */
answered_at: number | null, 
/**
 * Core stopped starting the server again after it crashed 3 times in 10 minutes.
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
 * Whether the App can reach Core's API from the internet.
 */
api_reachability: ReachabilityReport | null, };

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
