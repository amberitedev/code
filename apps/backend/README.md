# Local accounts and sharing backend

One Cloudflare Worker owns account, social, and sharing metadata in D1. Shared files are objects in
an R2 bucket, keyed by shared instance and SHA-256, which wrangler emulates locally. There are no
remote bindings or deployment scripts. The root development runner supplies local secrets and keeps
persistent state under the worktree's `.data/`.

Wrangler is pinned to 4.113.0 to avoid the local ProxyWorker connection-loss regression observed
with 4.115.0. See the [upstream report](https://github.com/cloudflare/workers-sdk/issues/15002).

Account and social handlers port the public Labrinth behavior from:

- `apps/labrinth/src/routes/internal/flows.rs` and `session.rs`
- `apps/labrinth/src/routes/v3/users.rs`, `friends.rs`, `blocked_users.rs`, `notifications.rs`
- `apps/labrinth/src/models/v3/users.rs`, `sessions.rs`, `notifications.rs`
- `packages/ariadne/src/networking/message.rs` and `users.rs`

The port retains Labrinth's AGPL-3.0 license. See `LICENSE.txt` and those source files for the original
implementation. It does not contain Modrinth production users, credentials, or private sharing code.

The `/v2` and `/v3` routes preserve the user/session/friend response shapes. Sessions use `mra_`
tokens, expire after 14 days, and can rotate within a fixed 60-day refresh window. D1 stores token
hashes. Passwords use Workers Web Crypto PBKDF2-SHA256 with a unique salt rather than Labrinth's
Argon2 implementation. Password strength uses zxcvbn. No legacy passwords are imported.

Password sign-up/login, TOTP with backup codes, session listing/revocation/rotation, profile and
avatar changes, friends, blocking, preferences, and notifications have local implementations.
The upstream admin account endpoints under `/_internal/admin/user/:id` support locks, session
revocation, forced password recovery, and 2FA removal. Only admins may call them; moderators and
admins cannot be locked or have their credentials reset. Lock details are only exposed to staff.
Locked accounts cannot authenticate, and a forced recovery flow can change their password without
unlocking them. Reset links are tied to the account's email address. Forced recovery and its local
outbox entry commit together. Revocation closes native friends sockets as well as HTTP sessions.
The `/sessions` admin operation revokes all supported sessions; this backend does not issue PATs.
Upstream's Discord-ID account lookup still depends on the unimplemented OAuth provider links.
Preferences include the upstream launcher behavior settings, and avatars accept up to 512 KiB.
Email verification and password resets write a local outbox, accessible only through the
dev-runner secret. OAuth providers, outbound email, passkey authentication, newsletter delivery,
PATs, and third-party OAuth authorization are not implemented. The corresponding auth methods
return an explicit unsupported-operation error rather than accepting an unverified identity.
Captcha is disabled only for loopback local development. A production captcha provider is not
configured, so password authentication outside local development returns a configuration error.

Local scenario accounts have password hashes and can log in through the real password endpoint.
The runner's `/_dev/session` shortcut can access only scenario accounts and requires the local
secret. Scenario passwords are `Scenario-<username>-Local-only!`; this path is disabled without
`LOCAL_DEV=true` and the correct secret on a loopback hostname.

Public content stays on Modrinth. Replacement accounts do not own same-named Modrinth profiles.
Local profile content lists are empty; public author requests must be routed to Modrinth by the
API-client boundary. The local service does not proxy account session tokens to Modrinth.

`node apps/backend/tests/accounts.mjs <backend-url>` exercises real password accounts against the
running local backend. Its private account data persists in `.data/backend-proof/accounts.json` so
running it again after a backend restart also proves persistence. It does not start a server.

`pnpm test` in `apps/backend` runs the contract tests in wrangler's local runtime. The sharing test
sends every request the App's sharing client sends and checks the files in R2.
