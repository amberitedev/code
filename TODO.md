# Amberite TODO

This is the simplified checklist of work remaining for Version 1.0. `feature-list.md` contains the canonical feature definitions.

Only the user may mark an item complete. Agents must not check off, strike through, remove, or otherwise complete an item unless the user explicitly says it is complete.

## Product-wide

- [ ] **Product boundaries** (`PB-01`–`PB-10`)
  Deliver the complete private-group product across the Desktop App, Copal Core, cloud platform, and Website. Keep Modrinth as the content and creator platform, keep Minecraft servers self-hosted, support the defined Windows/Ubuntu platforms, and do not expand 1.0 into public communities or a replacement catalog.

- [ ] **Completion standard** (`UX-01`–`UX-14`)
  Replace mock data, disconnected controls, unfinished routes, and happy-path-only screens with real end-to-end behavior. Every flow needs correct loading, empty, offline, unauthorized, unavailable, success, failure, destructive-action, permission, recovery, and long-running-operation states without requiring repository knowledge or terminal work.

## Accounts, social, and cloud

- [ ] **Identity and authentication** (`ID-01`–`ID-18`)
  Provide Minecraft-backed Amberite sign-in, automatic identity creation or recovery, secure session restoration, and clear expired-session handling. Support verified handles, editable display names, additional verified play accounts, launch-account selection, active-session revocation, sign-out, identity uniqueness, and safe account deletion with complete consequence explanations.

- [ ] **Account settings and Modrinth connection** (`ID-19`, `MR-01`–`MR-07`)
  Provide one complete settings surface for profile, privacy, linked accounts, play accounts, notifications, blocked users, active sessions, sign-out, and account deletion. Modrinth linking must use real authorization, remain optional for normal Amberite use, show its connection state, support unlinking, and redirect Modrinth-owned actions correctly.

- [ ] **User profiles and privacy** (`UP-01`–`UP-10`)
  Provide a consistent fixed profile layout with avatar, banner, display name, verified Minecraft handle, bio, relationship actions, and permitted activity. Add section-level relationship-aware visibility, private-profile behavior, privacy previews, protected group membership, and blocking rules that override profile, presence, invitation, and activity visibility.

- [ ] **Presence, activity, and play invitations** (`PR-01`–`PR-15`)
  Show online state, current session duration, permitted modpack activity, and server activity only when the viewer may know about that server. Clean up stale presence, respect privacy and blocks, expose Join only to eligible viewers, and deliver actionable play invitations through the normal install, update, access, and Join flow.

- [ ] **Friends and blocking** (`FR-01`–`FR-14`)
  Support finding users, sending requests, viewing incoming and outgoing requests, accepting, declining, cancelling, removing friends, blocking, and unblocking. Prevent duplicate, self-directed, blocked, and already-resolved requests, apply relationship changes immediately, and deliver durable notifications without leaking protected profile or activity information.

- [ ] **Groups and group profiles** (`GR-01`–`GR-16`)
  Support private-group creation, permanent 1.0 Owner assignment, one associated Core, and fixed group profiles containing permitted identity, membership, server, and Core information. Include Owner-only editing, member leaving, safe Core unpairing, safe group deletion, recoverable Copal data, and clear explanations of what each destructive action affects.

- [ ] **Group invitations and membership** (`IN-01`–`IN-12`)
  Support pre-approved Owner invitations, member-proposed invitations that require Owner approval before notifying the recipient, and reusable invite links that create approval requests instead of immediate membership. Cover approval, rejection, acceptance, decline, expiry, duplication, blocking, disabling, rotation, revocation, and understandable states for every participant.

- [ ] **Roles and per-server access** (`AC-01`–`AC-17`)
  Enforce the fixed Owner, Admin, and Limited model across cloud data, Core actions, navigation, pages, and controls. Copy group membership into new servers, add later group members as Limited, keep Owner present, support per-server promotion, demotion, and removal, and revoke discovery, snapshots, joining, notifications, and whitelist access immediately.

- [ ] **Notifications and activity inbox** (`NT-01`–`NT-13`)
  Provide one durable notification center for friend requests, group invitations, invitation proposals, approvals, role changes, access changes, snapshot updates, server failures, crashes, Core problems, and compatibility requirements. Keep actionable items until handled, open the exact relevant screen, group repeated failures, and respect notification preferences without hiding mandatory security information.

## Desktop onboarding

- [ ] **First-run onboarding** (`ON-01`–`ON-10`)
  Guide new users through Amberite’s group/Core model, Minecraft sign-in, and the correct setup path for a local Core, remote Core, or membership without owning a Core. Show real progress, handle elevation and pairing failures, resume interrupted setup, make skipped Core setup available later, and end with one useful next action.

## Copal Core

- [ ] **Installation and pairing** (`CO-01`–`CO-17`)
  Support automatic Copal download, configuration, startup, and silent local pairing from the Windows App, plus supported remote Windows and Ubuntu installation paths. Handle pairing codes, expired or used codes, ownership conflicts, reconnection, credential reset, repair, unpairing, safe program removal, and separate explicit deletion of persistent server data.

- [ ] **Copal CLI** (`CLI-01`–`CLI-15`)
  Provide complete commands for installation, initial configuration, start, stop, restart, status, pairing-code display, connection reset, logs, version, updates, health checks, repair, and safe uninstall. Output must clearly explain success and failure, work for people and automation, preserve server data by default, and require separate confirmation for permanent deletion.

- [ ] **Security, routing, and private health** (`SE-01`–`SE-16`)
  Store Core routing privately and return it only to authenticated users with current group or server access. Authenticate and authorize every Core action, revoke removed access, keep addresses out of public surfaces, report private heartbeat states, distinguish health from user presence, protect management even when the game port is public, and support direct diagnostics.

- [ ] **Automatic networking and diagnostics** (`NW-01`–`NW-16`)
  Detect same-machine and LAN routes before remote access, request elevation only when required, configure supported firewall rules, attempt router port mapping, and verify external Minecraft and management reachability. Diagnose Core, firewall, router, carrier-grade NAT, provider, address, and server-configuration failures separately, then provide an exact correction and retry path.

## Desktop home, library, and discovery

- [ ] **Home dashboard** (`HM-01`–`HM-10`)
  Make Home the main play dashboard with Continue Playing, required updates, assigned server state, group/Core health, friends online, permitted activity, invitations, approvals, and recent important events. Every card must reflect the viewer’s role, avoid leaking inaccessible servers, and open the exact screen or action needed to continue.

- [ ] **Library and local instances** (`LB-01`–`LB-17`)
  Unify local clients, custom clients, synchronized group servers, and saved public servers while keeping their types and available actions obvious. Add search, filtering, sorting, version/update/running/access states, correct role-based detail pages, client creation, import, duplication, customization, repair, removal, and Worlds visibility only for ordinary local clients.

- [ ] **Modrinth discovery and content** (`MD-01`–`MD-12`)
  Provide Modrinth project search, game-version and loader compatibility filters, project details, galleries, authors, categories, versions, environment rules, and dependency explanations. Support compatible target selection, new-instance creation when needed, progress, installation, updates, personal-override preservation, useful incompatibility errors, original project links, and visible Modrinth attribution.

- [ ] **Public Modrinth servers** (`PS-01`–`PS-08`)
  Keep public Modrinth server projects separate from private Amberite-managed servers. Support browsing, project details, saving, unsaving, required-profile installation, required updates, play-account selection, joining, and clear unavailable, incompatible, outdated, or unreachable states without implying Amberite controls the public server’s Core.

## Server management

- [ ] **Server creation** (`SC-01`–`SC-14`)
  Support creation of blank servers, compatible Modrinth modpack servers, synchronized servers from local client profiles, and imports from existing servers or whole-server backups. Validate storage, Java, game version, loader, content, and source files; show every stage; roll back failures; copy access; assign Owner; and create an initial snapshot when possible.

- [ ] **Overview and lifecycle** (`SV-01`–`SV-13`)
  Show stopped, starting, running, stopping, crashed, updating, restoring, and unavailable states with uptime and player information where available. Support safe start, graceful stop, restart, warned force-stop, progress that survives navigation, conflicting-operation prevention, crash recovery links, presentation changes, and deletion choices that distinguish records from local data.

- [ ] **Console, logs, and failures** (`CL-01`–`CL-11`)
  Provide live console streaming and authorized command submission with readable history that survives page navigation. Add search and filtering, Copal lifecycle logs, previous Minecraft logs, crash reports, downloads, clear warning/error separation, and links from deterministic failures to the correct content, file, runtime, setting, repair, or networking action.

- [ ] **Players and Minecraft access** (`PM-01`–`PM-10`)
  Show connected players and their Amberite identity when known, then support kick, ban, unban, operator, and whitelist management. Automatically whitelist verified play accounts for authorized members, remove that access when authorization is revoked, preserve explicit bans, and explain disagreements between Amberite membership and Minecraft server access.

- [ ] **Files** (`FL-01`–`FL-11`)
  Provide server-bounded filesystem browsing, safe text editing, uploads with progress and conflict handling, downloads, directory export, directory creation, rename, move, and warned deletion. Prevent path escape, warn about unsafe live edits, preserve navigation during refresh, and explain permission, size, lock, conflict, and invalid-file failures precisely.

- [ ] **Settings and runtime** (`ST-01`–`ST-11`)
  Provide clear controls for supported server properties while preserving unknown values, plus compatible Java selection, missing-runtime detection, managed runtime installation or repair, memory, and safe launch settings. Mark restart requirements, validate changes before replacing working settings, recover from invalid changes, and keep Core as the settings source of truth.

- [ ] **Content and modpack management** (`CM-01`–`CM-15`)
  Identify installed Modrinth content where possible and distinguish catalog projects, local files, server-only overrides, and unresolved files. Support compatible search, dependencies, add, update, replace, remove, full modpack upgrades, rollback, server/client impact explanations, client-only rejection, drift awareness, and snapshot publication for client-affecting changes.

- [ ] **Backups and world boundary** (`BK-01`–`BK-14`)
  Provide manual whole-server backups with names, notes, creation time, size, source, listing, download, restore, deletion, and import-as-new-server. Create a recovery backup before restore, coordinate server safety, show progress, recover from interruption, preserve data, and keep 1.0 to one implicit server world without dedicated world switching or reset tools.

## Synchronization and play

- [ ] **Snapshot publishing** (`SN-01`–`SN-12`)
  Treat Copal as the canonical snapshot source and let Owner/Admin publish the client-required mods and configs as named, versioned snapshots with author and change notes. Provide preview, current and previous history, member notifications, late-member installation, server-only exclusion, Limited-member restrictions, and clear explanations when server state cannot become a compatible client snapshot.

- [ ] **Member installation and updates** (`SY-01`–`SY-12`)
  Show every assigned synchronized server in the member’s library, install its current snapshot on first use, and replace Play with Update whenever the required snapshot changes. Display every transfer and application stage, resume safely, verify before Join, preserve unrelated local data, enforce removed access, support offline clarity, and explain every common failure.

- [ ] **Personal client overrides** (`OV-01`–`OV-10`)
  Allow compatible client-only mods, personal configs, resource packs, and shaders on synchronized instances without modifying the canonical group snapshot. Preserve them across required updates, detect newly incompatible overrides, explain what conflicts, provide safe disable or removal recovery, and prevent personal files from silently replacing required shared server-compatible content.

- [ ] **Server drift** (`DR-01`–`DR-10`)
  Compare server mods and configs with the published snapshot using embedded metadata rather than filenames alone. Report added, removed, changed, unresolved, and configuration differences; warn without forcing shutdown; and let managers revert, preserve a server-only override, publish compatible changes, ignore once, or handle an unclassifiable difference safely.

- [ ] **Launch, Join, and whitelisting** (`JP-01`–`JP-12`)
  Present Install, Update, Play, or Join according to installation, snapshot, access, Core, and server state. Prepare the selected verified play account, game, loader, content, configs, and overrides; supply the address automatically; support joining through friend activity or invitations; and provide the exact correction when access, runtime, networking, or whitelist blocks play.

## Recovery, Website, and delivery

- [ ] **Cross-product repair and recovery** (`RP-01`–`RP-10`)
  Detect missing or damaged client installations, server runtime or loader files, and Copal storage, permission, runtime, process, or configuration state. Repair deterministic problems without deleting saves, overrides, worlds, or server data; preserve originals before risky changes; and provide precise manual recovery when automatic repair is unsafe or impossible.

- [ ] **Website** (`WB-01`–`WB-15`)
  Provide the public product explanation, Desktop and Copal downloads, installation and recovery documentation, release notes, compatibility information, and service status. Add Minecraft-backed sign-in plus account, privacy, profiles, friends, blocking, groups, and invitation parity while keeping Core, console, files, settings, snapshots, and backup management out of the 1.0 Website.

- [ ] **Releases, compatibility, and updates** (`RL-01`–`RL-17`)
  Version the Desktop App, Copal, and Platform/Web independently while enforcing one shared compatibility protocol. Provide optional normal App updates, required security or expired-compatibility updates, automatic compatible Copal updates with rollback, coordinated incompatible updates, continued Minecraft server operation, component-specific artifacts, and accurate release notes.

- [ ] **Production delivery** (`DP-01`–`DP-11`)
  Turn component releases into signed Windows App installers, Windows and Ubuntu Copal downloads, working in-App updates, production cloud services, and the public Website. Verify sign-in, social access, Core discovery, downloads, compatibility, and health; prevent failed releases from appearing available; support rollback; and keep secrets out of downloadable clients and documentation.

- [ ] **Release acceptance journeys** (`QA-01`–`QA-12`)
  Prove the complete Owner flow from sign-in through Core setup, group creation, server creation, invitation, synchronization, and play. Prove remote networking, Admin management and drift, Limited-member restrictions, immediate access revocation, running-server resilience, recoverable data, Website social behavior, and the absence of mock data, dead controls, and unfinished routes.
