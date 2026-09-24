# Accounts and sharing: goals and decisions

Updated 2026-09-24. This defines the intended result and agreed boundaries. Implementation structure
and task order are left to the implementing agent.

## Goal

Recreate Modrinth's existing account, friends, and private instance-sharing experience on our own
backends. Public projects, mods, versions, and downloads continue to come from Modrinth.

This milestone works locally with real persistence and storage. Neither participant needs a Minecraft
server or a paired Core to share. Minecraft hosting is the next milestone. Amberite-specific UI and UX
come after recreating the existing functionality.

## Client baseline

Ilai chose restoration of `apps/app-frontend` and `apps/app` to upstream behavior on a new branch.
Keep development-environment support and the existing API-client integration boundary. Set aside
other client changes, including ad cleanup, for later restoration. Preserve existing work, including
uncommitted changes.

After restoration, leave the client UI and UX unchanged wherever possible. The replacement backends
must satisfy the client. Missing backend functionality is not a reason to redesign or remove a screen.
Custom Minecraft-first accounts and friend-code UX are not the baseline for this milestone.

The September 24 audit found upstream `10b1d7002` closely matches our restoration at `387a96f58`.
That is a candidate compatible baseline. Updating to today's upstream is separate work involving
shared dependencies, not simply replacing the two app directories.

## Compatibility

The existing typed API client remains the app-facing boundary. It can route requests, coordinate
transfers, and adapt responses while presenting the operations the UI expects. Keeping this boundary
does not mean keeping its old Convex-specific implementation.

Native Rust code also performs sharing requests. Necessary transport and session changes are part
of integration, subject to repository edit permissions. Changing HTTP base URLs alone may not suffice.

Reuse or port relevant public Labrinth account/social code, preserving attribution and licenses.
Do not rebuild public content hosting. Recreate private sharing from public client contracts and
authorized official-app observations. The research mock is not the replacement service or proof of
how Modrinth's private backend behaves.

Account compatibility means matching functionality, not importing Modrinth's users or credentials.
Provider configuration may require separate credentials. A dev login does not prove a real login works.

## Backend responsibilities

The central backend owns accounts, sessions, friends, and related social state. The sharing backend
owns shared-instance metadata, membership, versions, storage locations, and coordination. These are
logical responsibilities, not necessarily separate deployments.

One backend application with separate modules and a shared relational database is the current
recommendation, not a finalized deployment decision. It avoids unnecessary calls between services
that need the same users.

The storage component holds shared bytes independently of Minecraft management. Core/Copal remains
the user-hosted server-management product. We do not host users' Cores.

Clients can prepare content and transfer directly to authorized storage endpoints. The backend keeps
authority over access, versions, and placement. Account session secrets do not go to unrelated storage
contributors. The exact split is an engineering choice intended to reduce central cost without
changing the user experience.

## Storage decisions

- Reference public Modrinth content rather than re-uploading every mod JAR. Preserve the content types
  supported by the existing sharing workflow.
- Start with two network copies and simple placement/repair. A complete verified copy can serve
  downloads while the second copy is created.
- Owner clients retain recovery snapshots. Lost remote content is restored automatically when a
  client holding recovery data becomes available. A mutable installed instance is not a historical backup.
- Encrypt contributed content. Compression and reuse of unchanged content should reduce storage
  and traffic without changing client-facing payload semantics. Specific techniques are delegated.
- The agreed retention direction is five recent online versions plus pinned versions, with local
  history governed by user settings. Local retention defaults remain open. New history/pinning UI
  is outside this milestone.
- Repair is event-driven and bounded. Healthy data should not constantly move for small placement
  improvements. Requested installs and updates take priority over background contribution work.

The NAS is eventual overflow/availability storage, not a permanent backup archive. Its deployment,
contributor onboarding, and fleet tuning are later work. Local storage processes can exercise basic
sharing, replica fallback, and recovery now.

## What working means

The main acceptance workflow uses two isolated app users against our real local services:

**Add friend → share an existing instance → accept and install → push changes → install the update.**

The resulting content and version match what was shared. Existing invitation, review, update, and
error behavior remains intact.

Account switching and app restart retain the correct user-specific state. Unauthorized users cannot
access private shares. Backend and storage restarts preserve data.

After upload completes, the recipient can install/update with the owner's app closed. Interrupted
transfers can be retried without treating partial or incorrect content as successful. Replica fallback
and owner-snapshot recovery can be demonstrated using disposable local data.

These are the main acceptance workflows, not substitutes for the other inherited account/social
operations. Missing functionality and unverified reference behavior must be identified.

The existing isolated dev environment supports owner/recipient testing. Official Modrinth may be
needed for reference checks. Ilai accepted testing two accounts on one computer; installation isolation
and the reported installer-issued sharing credentials still need verification if those checks are needed.

## Open decision: backend runtime

Cloudflare Workers + D1 is the current candidate. Retaining relevant Rust/Postgres code on a service
such as Cloud Run is an alternative. The choice depends on actual code portability and operating cost.
The implementing agent should recommend a choice to Ilai before committing to a large port.

The cost goal is free or inexpensive operation for roughly 1,000 to 5,000 users. Those figures express
intended scale, not measured capacity. Small usage-based costs are preferable to a required subscription.
Development should use the selected runtime locally; production deployment comes later.

## Deferred work

Minecraft hosting integration, server pairing and Worlds UX, private-Core settings, contribution
incentives, tunnels, firewall/router setup, production deployment, direct client-to-client capacity
fallback, advanced placement/AI, erasure coding, PRs/branches, and expanded KubeJS/FancyMenu content
handling are outside this milestone. Ad cleanup and other Amberite UI changes return separately.

Use focused subagents where useful. The lead owns integration and unresolved decisions. Task breakdown,
file ownership, and implementation order belong to execution, not this product plan.

The fuller history and later ideas remain in [planning-memory.md](planning-memory.md). They provide
context, not additional requirements for this milestone.
