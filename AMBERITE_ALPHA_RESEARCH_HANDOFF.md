# Amberite Rescope Research and Audit Findings

Research snapshot: 2026-08-16.

This document contains the findings produced by the Terra and Sol research/audit work. It does not copy the product specification, Notion tasks, repository files, or implementation code. Those sources are referenced where relevant.

## Current direction supplied by Ilai

- Optimize Alpha 0.1 for one complete, working play path rather than the previous broad Version 1.0 scope.
- Keep the fork close to upstream Modrinth unless a difference creates real Amberite value.
- Copy and adapt Modrinth Shared Instances instead of maintaining Amberite’s separate synchronization UX.
- Keep Minecraft-first Amberite identity. Modrinth linking remains for Modrinth-owned profile, content, creator, publishing, and moderation functionality.
- Replace Shared Instances’ Modrinth social/account dependencies with Amberite equivalents where required.
- Alpha supports one private group, one Core, and one server.
- Keep the previously decided networking direction. Do not repeatedly reopen it as an unanswered product question.
- Audit visible UX/UI changes and hidden implementation changes. Revert changes that reimplement upstream behavior without meaningful benefit.

Product context is in the [Amberite Notion project](https://app.notion.com/p/39e45eb0db598113ad2de66386847442), `PROJECT.md`, `feature-list.md`, and `TODO.md`.

# Report A: Current implementation audit

Terra inspected the repository to distinguish real implementation from incomplete or disconnected product surfaces.

## What is substantially implemented

### Copal Core

Core is a real Rust/Axum server manager. Its API and services cover setup, installation, lifecycle, console/SSE, historical logs and crashes, files, mods, properties, Java/runtime configuration, backups, RCON-backed player administration, access, and synchronization primitives.

Representative files:

- `apps/core/src/presentation/router.rs`
- `apps/core/src/application/instance_service.rs`
- `apps/core/src/application/instance_status_service.rs`
- `apps/core/src/application/backup_service.rs`
- `apps/core/src/application/fs_service.rs`
- `apps/core/src/application/sync_apply_service.rs`
- `apps/core/src/cli/mod.rs`

The implementation includes process actors, SQLite persistence, server installation, bounded filesystem operations, safe archive handling, pre-restore protection, and a standalone operational CLI.

### Amberite authentication

Minecraft-backed Amberite authentication is implemented rather than mocked:

- `convex/auth.ts` verifies Minecraft identity and creates or restores an Amberite account.
- `apps/app-frontend/src/composables/useAmberiteAuth.ts` manages frontend authentication state.
- `apps/app-frontend/src/adapters/desktop.ts` connects the frontend to native auth.
- `apps/app/src/api/auth.rs` handles native commands and secrets.
- `packages/app-lib/src/state/minecraft_auth.rs` contains modified Minecraft account behavior.

### Social persistence

Convex has real persistence for profiles, friend requests, blocking, groups, membership, and invitations:

- `convex/profiles.ts`
- `convex/friends.ts`
- `convex/friendGroups.ts`
- `convex/groupInvites.ts`

The complete social product is not finished, but the backend is not purely placeholder code.

### Core pairing

Remote pairing has a credible security flow:

- `apps/core/src/application/pairing_service.rs`
- `apps/core/src/presentation/handlers/setup.rs`
- `convex/presence.ts`
- `apps/app-frontend/src/components/core/use-core-onboarding-state.ts`

An unpaired Core registers a short-lived code, the authenticated App claims it, Core verifies a one-time credential, and the cloud finalizes ownership.

### Upstream launcher functionality

The fork retains upstream local instances, discovery, Modrinth installation/update machinery, game launching, public-server joining, and most Shared Instances code.

Important retained locations:

- `apps/app-frontend/src/components/ui/shared-instances/`
- `apps/app-frontend/src/pages/instance/share/`
- `apps/app-frontend/src/pages/instance/components/admonitions/shared-instance-*.vue`
- `apps/app-frontend/src/helpers/worlds.ts`

`helpers/worlds.ts` already contains direct-server launch support. Amberite does not need another launcher implementation.

## Verified breaks in the intended journey

### Parallel synchronization authorities

The fork currently has two incompatible synchronization paths:

- Core stores real `.mrpack` archives through `social_sync_service.rs` and `sync_apply_service.rs`.
- `convex/sync.ts` stores metadata manifests and explicitly says the diff/apply pipeline is deferred.

The synced UI publishes cloud metadata while Library installation downloads from Core. This is not one coherent version/publish/install path.

### Member snapshot permission mismatch

The Core snapshot download handler requires `server:content`, while the member preset grants `client:content`. Limited members therefore cannot use the intended snapshot-download path.

Files:

- `apps/core/src/presentation/handlers/sync.rs`
- `apps/core/src/application/access_service.rs`

### Synced profiles do not Join

`apps/app-frontend/src/pages/instance/synced/SyncedIndex.vue` launches a normal local profile. It does not supply the associated server or invoke the existing join-server path.

### Cloud membership and Core authorization diverge

Accepting a Convex invitation creates cloud membership, while Core authorizes against separate SQLite `core_members` and `instance_members` state. The audit found no complete reconciliation path. A user can appear in the group UI while Core denies them.

### Per-server permission UI is partly simulated

`apps/app-frontend/src/pages/instance/synced/use-synced-role.ts` derives a primary-group preset, does not use the selected server as a real authorization boundary, and supports local “View as” state. This is useful prototype UI, not production access control.

### Networking is incomplete

The current network service attempts UPnP for the Core API port and describes Minecraft exposure as per-instance direct/UPnP. Playit is explicitly unimplemented. The planned firewall, Minecraft mapping, public verification, CGNAT/provider diagnosis, and correction flow are not complete.

File: `apps/core/src/application/network_service.rs`.

### Whitelist automation is disconnected

Convex can calculate eligible whitelist identities, but no complete path applies that state to Core. Core’s current whitelist operations are explicit RCON commands in `apps/core/src/presentation/handlers/players.rs`.

### Presence is narrower than planned

`apps/realtime/src/index.ts` publishes desktop online/offline state and authorization invalidation. It does not implement the old richer presence contract: session duration, catalog modpack identity, current private server, invitations, or Join eligibility.

### Misleading or broken reachable UI

The audit found:

- `/group/mock` and `/group/mock-public` with hard-coded data.
- Home links to mock group routes.
- `showAmberiteAccountModal` hard-coded to `false` in `App.vue`.
- A local share route referencing a missing route member while upstream’s real sharing tree exists elsewhere.
- “Invite to play” copying a Core management URL.
- Core addresses displayed in member-facing surfaces.
- Website changes without a complete Amberite web account/social journey.

## Report conclusion

The repository has substantial component-level work, especially Core, but not one working Alpha journey. The fastest path is to treat Amberite as a narrow self-hosted-server layer on the current Modrinth App rather than preserving parallel launcher, sharing, profile, and social systems.

# Report B: Modrinth Shared Instances and account research

Sol researched the July/August 2026 Modrinth releases, public documentation, source code, account APIs, social implementation, pricing, terms, and licensing.

## Feature release

- Shared Instances shipped in App 0.16.0 on 2026-07-24.
- In-App profiles, profile editing, blocking, social settings, and expanded sharing management followed in 0.17.1 on 2026-07-28.
- The latest public release inspected was 0.17.10 on 2026-08-16.

## Shared Instances UX

The official flow is:

1. An owner opens an instance’s Sharing tab.
2. They invite by Modrinth username or create a share link.
3. The recipient reviews and accepts or denies the installation.
4. The preview identifies game, loader, Modrinth content, and external files.
5. Acceptance installs a normal local instance.
6. The owner explicitly publishes later changes.
7. Recipients review and install required updates before playing.
8. Owner-managed content is read-only; recipients can layer personal content on top.
9. Owners manage members and invite links.
10. Shares can be reported or quarantined.

The model is versioned and explicit, closer to Git than continuous folder synchronization.

## Service architecture

The feature is not peer-to-peer. The official client authenticates to `shared-instances.modrinth.com` with a Modrinth bearer session.

A version contains or references:

- Minecraft version
- Loader and loader version
- Modrinth version IDs
- Optional linked modpack version
- External-file descriptors
- Selected configuration files
- Instance name and icon

Published Modrinth files resolve by ID. Other selected files and config bundles upload to the centralized service.

Source:

- [Service client](https://github.com/modrinth/code/blob/14b2943dbe2e875965091a2e2888353b9e0b8dde/packages/app-lib/src/api/instance/shared/client.rs)
- [Publishing](https://github.com/modrinth/code/blob/14b2943dbe2e875965091a2e2888353b9e0b8dde/packages/app-lib/src/api/instance/shared/publish.rs)
- [Installation](https://github.com/modrinth/code/blob/14b2943dbe2e875965091a2e2888353b9e0b8dde/packages/app-lib/src/api/instance/shared/install.rs)
- [Invitations](https://github.com/modrinth/code/blob/14b2943dbe2e875965091a2e2888353b9e0b8dde/packages/app-lib/src/api/instance/shared/invites.rs)

## Observed limits

These are current implementation details, not service guarantees:

- 50 users per shared instance.
- Invite links default to seven days and ten uses.
- The client rejects expiry beyond seven days.
- Configuration sharing is restricted to the instance `config` directory.
- Up to 4,096 config entries.
- 16 MiB maximum per uncompressed config file.
- 128 MiB maximum total uncompressed config data.
- Icons capped at 4 MiB and normalized to 512×512.
- Imported `.mrpack` links must be unlinked before sharing.
- Update diffs represent file additions/removals, project version changes, modpack links/updates, Minecraft changes, loader changes, and config changes.
- The client distinguishes deletion, revocation, and quarantine.

The service’s source, storage policy, retention, bandwidth quota, abuse limits, uptime commitment, and pricing contract are not public.

## Accounts and social

Modrinth profiles are Modrinth account/creator profiles, not Minecraft identities. They contain username, avatar, short bio, badges/role, account date, aggregate downloads, projects, collections, and organizations.

The source-visible friend system supports requests, acceptance, removal/cancellation, pending relationships, blocking, and disabling incoming requests. Blocking also prevents Shared Instances and Hosting invitations.

Presence shows online/offline, current local instance name, and last update time. It expires after roughly 60 seconds without renewal. It does not represent Amberite group access, Minecraft UUID ownership, server authorization, or Join readiness.

Official presence requires restricted `SESSION_ACCESS`, which normal OAuth apps cannot request.

## OAuth findings

Public OAuth can link a Modrinth identity, but it cannot replace Minecraft verification.

Observed constraints:

- Token exchange requires a client secret.
- PKCE was not documented in the examined flow.
- No refresh token is returned.
- Access tokens expire after roughly 14 days.
- Desktop must not embed the client secret; an Amberite backend must exchange codes.
- Modrinth identity does not prove Minecraft ownership or provide the UUID needed for launch and whitelist enforcement.

Source-visible v3 friend/block routes appear to use ordinary user scopes but were not found in the stable public API reference. Do not treat them as a durable external contract without confirmation.

## Fork service risk

Open-source client code does not guarantee access to Modrinth’s hosted Shared Instances backend. Public documentation does not establish third-party fork eligibility, OAuth compatibility, quotas, availability, pricing, or a stable API contract.

This does not invalidate Ilai’s decision to copy the feature. It means the implementation must identify the backend/auth boundary. Replacing the friend UI alone does not prove the official service will accept Amberite sessions.

## Pricing, license, and branding

At research time:

- Shared Instances and social features were not listed as Modrinth Plus benefits.
- Plus was listed at $4.99/month.
- The client checked authentication and blacklist state rather than a subscription entitlement.
- The App frontend, shell, and native app library are GPLv3-only.
- Shared UI/API-client packages are LGPLv3.
- Labrinth is AGPLv3-only.
- Modrinth’s copying rules require forks to remove protected branding assets.
- Open-source licensing does not grant irrevocable access to Modrinth services or trademarks.

Primary sources:

- [Shared Instances announcement](https://modrinth.com/news/article/shared-instances/)
- [0.16.0 changelog](https://modrinth.com/news/changelog/app/0.16.0)
- [0.17.1 changelog](https://modrinth.com/news/changelog/app/0.17.1)
- [OAuth guide](https://docs.modrinth.com/guide/oauth/)
- [API documentation](https://docs.modrinth.com/api/)
- [Modrinth Plus](https://modrinth.com/plus)
- [Terms](https://modrinth.com/legal/terms)
- [Privacy](https://modrinth.com/legal/privacy)
- [Copying guidance](https://github.com/modrinth/code/blob/14b2943dbe2e875965091a2e2888353b9e0b8dde/COPYING.md)
- [OAuth scopes](https://github.com/modrinth/code/blob/14b2943dbe2e875965091a2e2888353b9e0b8dde/apps/labrinth/src/models/v3/pats.rs)
- [Friend routes](https://github.com/modrinth/code/blob/14b2943dbe2e875965091a2e2888353b9e0b8dde/apps/labrinth/src/routes/v3/friends.rs)
- [Block routes](https://github.com/modrinth/code/blob/14b2943dbe2e875965091a2e2888353b9e0b8dde/apps/labrinth/src/routes/v3/blocked_users.rs)

# Report C: Git and subsystem divergence

Terra compared Amberite against both the historical fork point and current upstream. A single direct diff is misleading because upstream also advanced.

## Baselines and scale

- Last shared commit: `feb3c3ee31ed5b7107987066afebe971b6c89677`, dated 2026-08-07.
- Amberite was 82 commits ahead of that baseline.
- Amberite-authored committed divergence: **801 files, +127,766 / -16,910**.
- Upstream independently advanced 80 commits by 2026-08-15.
- Upstream’s advance: **837 files, +35,004 / -19,301**.
- **65 changed-path overlaps**, concentrated in `packages/ui` and `apps/app-frontend`.
- Dirty tree at audit time: ten files, approximately **+409 / -107**.

A direct current-upstream-to-HEAD diff reported about 1,572 files because it mixed Amberite work with unmerged upstream work.

## Subsystem findings

### Core

- 193 changed paths and +28,595 lines.
- Excluding runtime artifacts: 149 substantive paths and roughly +22,144 lines.
- Core is the strongest differentiator and should not be broadly reverted.

### `@amberite/amberite-api`

- 55 paths and about +9,934 lines.
- Useful typed contracts and operations coexist with older compatibility, relay, endpoint-policy, and generalized pipeline infrastructure.
- Keep real contracts/callers; remove speculative layers only after tracing usage.

### Native App and `app-lib`

- `apps/app`: 13 paths, approximately +1,374 / -74.
- `packages/app-lib`: nine historical Amberite paths, approximately +403 / -26.
- Auth, keyring, pairing, and Core integration are valuable.
- `app-lib` modifications increase recurring upstream conflict and should be minimized where extension points exist.

### Convex and realtime

- Convex contains identity, profiles, friends, groups, invitations, Core projection, messaging, sync metadata, and auth.
- Realtime added about 11,656 lines, but roughly 10,936 were generated Worker declarations.
- Identity/pairing are important; broad social, messaging, custom sync, and presence need Alpha justification.

### Desktop frontend

- 182 changed paths, approximately **+35,339 / -2,052**.
- Major areas: 49 UI-component paths, 51 instance-page paths, 25 Core components, and a heavily rewritten root shell.
- The main maintenance burden is frontend divergence rather than Core.

### Repository artifacts

The audit found roughly 228 MiB of tracked local/generated state under areas including:

- `apps/core/.copal/`
- `apps/core/logs/`
- `apps/core/target-access-tests/`

This included jars, compiled dependencies, SQLite state, logs, and an environment file. Removing current files would not remove historical blobs or exposed secrets.

# Report D: UX/UI and hidden-behavior audit

## Valuable Amberite differences

- Minecraft-backed Amberite sign-in and recovery.
- Core onboarding, pairing, health, and status.
- Core server entries in Library without mutating local instances.
- One truthful Core server-management surface.
- Amberite membership/access required by the private server.
- Shared-instance/server association and automatic Join.

## Duplicated or low-value divergence

- Root-shell pull/minimize interaction and custom route transitions.
- Browse freeze frames, ghost cards, and staging machinery.
- Rewritten ordinary profile pages.
- Custom synced-profile UI duplicating Shared Instances.
- Duplicate server routes.
- Copied upstream components differing only by one data source or action.
- Global backend rewiring hidden beneath otherwise upstream-looking UI.

## Regressions and misleading behavior

- Library refresh can silently remove local server profiles before replacing them with Core entries.
- “View as role” changes local UI state rather than authorization.
- Core management addresses leak into member-facing surfaces.
- Mock and development routes are reachable.
- Core installation copy claims work the implementation does not perform.
- `/hosting/manage/:id` and `/instance/:id` represent the same server concept.

## Low-value changes called out by the audit

The audit used Ilai’s phrase “stupid changes” to mean changes with little product value and significant divergence:

1. Tracking jars, build outputs, databases, logs, and local environment files.
2. Replacing normal navigation with a roughly 1,300-line pull/minimize interaction.
3. Rebuilding Browse transitions with freeze frames and ghost cards.
4. Adding unused shimmer and generic motion changes.
5. Shipping mock profile/group actions and fake Core state on Home.
6. Exposing developer reset and setup/dashboard controls.
7. Claiming to install Copal while only checking whether it already runs.
8. Copying a management URL as an invitation.
9. Showing Core routes in member-facing UI.
10. Maintaining duplicate management routes.
11. Simulating authorization with local role state.
12. Copying large upstream components instead of adding a seam.
13. Putting a one-consumer Tauri string classifier in a platform-neutral package.
14. Adding future-only profile fields and comma-separated IDs to normal settings.
15. Icon/import-only divergence on a conflict-heavy fork.
16. Building generalized communication infrastructure before multiple transports require it.

# Report E: Reduction recommendation

## Keep

- Copal Core and its working server-management capabilities.
- Minecraft-backed Amberite identity and durable sessions.
- Native keyring/session handling.
- Pairing and private Core route/health state.
- Minimal typed Core contracts/client.
- One Core destination and one server-management route.
- Minimum Amberite membership, whitelist, and revocation needed for the private server.
- Modrinth Shared Instances UX and implementation patterns, adapted rather than replaced.

## Restore upstream, then reapply a small Amberite seam

- `apps/app-frontend/src/App.vue`
- Library data model and behavior
- Browse/discovery rendering
- Shared hosting layouts
- Route organization

The reapplied patch should contain only real Amberite behavior: session/Core integration, Core rows, a Core backend adapter, one server route, shared-instance association, and Join.

## Restore upstream directly

- Ordinary Home behavior unless a small Amberite section uses complete real data.
- Ordinary Modrinth profile pages.
- Shared Instances presentation instead of custom synced-profile UI.
- Normal local-instance behavior.
- Generic transitions, cards, icons, and discovery UI.
- Website modifications if the Website is outside Alpha.

## Park or remove from Alpha navigation

- Mock routes and development controls.
- Custom snapshot/synced-profile product surfaces.
- UI-only role simulation.
- Multiple groups and multiple servers.
- Broad role matrices.
- Rich profile/privacy work.
- General presence, messages, activity history, and notification center.
- Browser Core dashboard and Website parity.
- Tasks, macros, schedules, automatic backup UI, and unused future APIs.
- Manual Core URL sharing.
- Speculative transport/pipeline abstractions.
- Generated/runtime repository artifacts.

Parking means removing a feature from reachable Alpha UX and release requirements, not leaving a visible unfinished page.

# Proposed Alpha boundary from the combined reports

This is the combined research recommendation, not a replacement for Ilai’s final decision.

1. Host authenticates through Minecraft-first Amberite identity.
2. Host installs or pairs one Core.
3. Host creates or selects the one server.
4. One local client instance is associated with that server and uses the copied Shared Instances flow.
5. Amberite verifies server/client compatibility and networking readiness.
6. Host explicitly publishes a numbered client version.
7. An Amberite friend/member receives and accepts the share.
8. Acceptance creates required access and applies the verified Minecraft UUID to the whitelist.
9. The recipient receives Install, Update, or Join according to real state.
10. Join uses the existing Modrinth-derived launcher and supplies the server automatically.
11. Later publishes create required reviewed updates.
12. Revocation removes future shared-version and Minecraft access.

## Proposed authority split

The older plan declared Core canonical for client snapshots. The Modrinth release creates a simpler possible split:

- Shared-instance versions own client-required mods and selected client configuration.
- Core owns the world, process, runtime, backups, server-only content/configuration, and operational state.
- One shared instance maps to one Core server.
- Publishing validates and stages/applies the server-compatible subset for Core.
- Do not keep Shared Instances and the old Amberite snapshot system as parallel authorities.

This is the main architectural decision surfaced by the research that is genuinely new relative to the older Notion plan.

# Important risks and incomplete verification

- No audit ran the complete owner/member path in production conditions.
- Third-party/fork access to Modrinth’s Shared Instances backend is undocumented.
- Replacing Modrinth social identity does not automatically solve Shared Instances service authentication.
- The dedicated Sol architecture-audit agent failed because its input exceeded the model context window. No report was fabricated. Internal transport recommendations therefore need direct call-graph verification.
- Deployed Convex data, Core SQLite state, keyring formats, and localStorage compatibility were not fully inventoried.
- Apparently unused API routes may have scripts or browser consumers.
- Generated realtime declarations may be required by the current build.
- Deleting tracked artifacts does not clean them from Git history.
- Shared Instances extension points should be rechecked after merging current upstream.

# Suggested reduction order

1. Preserve the current state before broad restoration.
2. Separate auth correctness work from cosmetic/import-only changes.
3. Remove tracked runtime/generated artifacts and investigate secret history.
4. Integrate current upstream separately from product reduction.
5. Remove mock routes, debug controls, misleading copy, and management-URL invitations.
6. Restore low-coupling visual churn.
7. Restore upstream shell/routes and reapply the minimal Amberite integration.
8. Restore Library and add Core entries through a typed adapter without mutating local instances.
9. Restore Shared Instances UI and remove the custom synced-profile surface.
10. Trace Core and `amberite-api` routes before parking unused APIs.
11. Reconcile cloud membership and Core authorization.
12. Implement the one shared-instance/one-server association, whitelist sync, and automatic Join.
13. Verify with separate owner/member Minecraft identities, one update, and one revocation.

The expensive work preserved here is the repository-state audit, broken-path analysis, Modrinth service/account research, fork statistics, UX/UI divergence audit, reduction classification, and technical risks. Product definitions and implementation details remain in their actual sources rather than being copied into this report.
