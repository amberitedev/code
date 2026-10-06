# Upstream sync audit, October 4–6, 2026

Integrated Modrinth upstream through `036631086ea63274a539f2205657491395599704`,
fetched October 6. The initial target was `71f35eb80`, followed by the latest delta.
Amberite started at `1bfbe1f48` on `restore-account-sharing`. Git ancestry started
at `feb3c3ee3`; the restored launcher was also compared with its copied baseline,
`10b1d7002`, to avoid duplicate old implementations from apparently clean merges.

The source [changelog](https://modrinth.com/news/changelog) ends with App 0.21.6,
September 27, at these targets. The audit also covered later commits absent from
that entry, especially account administration, sharing, and install recovery.

## Accepted product decisions

- "Amplify" meant `packages/app-lib`, which is explicitly in scope.
- Port staff account tools to the Cloudflare Worker/D1 backend with upstream permissions.
- Copy upstream UI directly. Mocks are recommended when useful for new design, not required,
  and not needed for upstream copying. The global AGENTS rule was updated accordingly.
- Signing out retains saved accounts. Explicit Remove account revokes and removes its account.
- Port compatible hosting API changes to Core. Report larger capabilities without building them.

## Completed ports

The merge imports inherited Labrinth, launcher/app-lib, desktop client, website,
shared UI, typed API client, translations, generated events and dependencies.
This includes settings synchronization, the download manager, content caching,
account switching, install recovery and debug export.

The current account service is `apps/backend`, a Cloudflare Worker/D1 port.
Older project documentation describing Convex accounts does not describe this milestone.

| Account change | Port and upstream source |
| --- | --- |
| Lock/unlock | D1 migration, admin-only operations, staff target protection, staff-visible metadata, blocked authentication and session issuance. [#7749](https://github.com/modrinth/code/pull/7749) |
| Revoke access | Admin operation revokes every supported session and closes native friends sockets. PATs are not issued by this backend. [#7755](https://github.com/modrinth/code/pull/7755) |
| Forced recovery and 2FA removal | Admin-only, protected staff targets, credential invalidation, durable local recovery outbox, email-bound single-use flows and notifications. Recovery can change a password without unlocking its account. [#7762](https://github.com/modrinth/code/pull/7762) |
| Preferences and profile contracts | Incoming launcher behavior preferences, raw avatar URLs, 512 KiB avatar limit and signup error codes. Admin requests route to our service. |
| Saved accounts | Logout deactivates accounts. Removal authenticates revocation as the selected account. Imported credentials from another service origin cannot appear in the switcher or become active. |

Sharing retains capability-authorized direct storage, SHA-256 verification, immutable
owner snapshots, pending transfers, replica recovery and instance locks. Incoming
streamed config bundles, SHA-512 upload headers, no-timeout uploads, content-set
caching, deleted public versions and multiple versions of one project are integrated.
Multiple-version behavior is covered by upstream [#7770](https://github.com/modrinth/code/pull/7770).

Core now returns node metadata with the actual scheme, port and server path prefix.
World stat and ZIP routes enforce server-files permissions and world ownership,
using existing safe paths and archive operations. ZIP reports actual completion.
Single and bulk addon updates accept a selected compatible version and preserve
disabled state. Download URL construction preserves Core's server path prefix.

Private account credentials stay separate from public Modrinth content requests.
Fork development commands and deployment decisions remain. The new upstream Docker
workflow runs only in `modrinth/code`, where its infrastructure exists. The upstream
language coverage file is generated during dependency installation for fresh worktrees.

## Capabilities requiring separate work

| Capability | Current behavior and next work |
| --- | --- |
| Full-world downloads | Core returns `method_type: unavailable`; the client disables the action. Needs archive production and short-lived download authorization. Existing backup downloads remain supported. |
| SFTP | Core has no SFTP service; its client hides those controls. Needs a service and credential lifecycle. |
| Internal subdomain lookup | Core uses its configured URL. No domain infrastructure was added. |
| Server locks and support | Latest upstream adds hosting locks and a support tab. Inherited Modrinth operations are retained; Modrinth-specific controls are excluded from Core. A Core implementation needs a separate product/API decision. |
| Addon update discovery | Existing Core responses still have `has_update: null`, and discovery GET routes remain unsupported. Selected compatible versions work; discovery needs separate work. |
| Cloud retention and billing fallback | Remain Modrinth-specific. Core retains local backup/storage behavior. |
| PATs and Discord lookup | D1 does not issue PATs or implement OAuth provider links. Revocation covers all supported sessions; Discord-ID lookup depends on absent provider links. |
| Production authentication configuration | OAuth, outbound email and production captcha remain unconfigured as before. Local durable outbox recovery works. Unsupported operations fail explicitly. |

## Verification

Passed focused checks:

- API-client TypeScript check and declaration build.
- Account backend TypeScript check.
- Six API routing tests, including admin routes and public-content credential isolation.
- Worker/D1 sharing contract test.
- Worker/D1 admin contract test, covering permissions, staff protection, lock visibility,
  session revocation, forced recovery, single-use flows, retained locks and 2FA removal.
- Seven native self-hosted integrity, sharing recovery and account-origin tests.
- Core `cargo check --locked --lib`.
- App frontend production bundle and locale coverage generation.
- Native launcher/library and Tauri shell `cargo check -p theseus_gui`, including the final upstream delta.

The App strict typecheck does not pass. Source comparison identified 113 affected files
identical to the initial upstream target; errors include inherited implicit parameter types,
shared-provider declarations, Vue query types and SVG module declarations. The public user
profile adapter was updated to the v3 project response rather than its old v2 contract.
Full App type cleanup is not represented as completed. The bundle succeeds independently.

The inherited bulk pre-commit hook launched 53 parallel formatter tasks and failed with
`SIGKILL`/worker-exit errors. Its rollback was verified against its index backup. Merge
commits bypass that hook; targeted formatting and checks were used. Upstream patch-file
whitespace and generated/translated source formatting were retained.
Logs live in the worktree's gitignored `.data/upstream-sync/` directory.

No browser or native UI inspection was performed. The user has not authorized it.
No pull request, push or deployment was requested or performed. Original uncommitted
Core, API, planning and untracked files were restored separately from merge commits and verified
against the saved tracked patches and untracked blobs. The earlier research audit alone is replaced
by this completed audit. Merge commits are `d1ed314db` and `eac3ea1ff`.
