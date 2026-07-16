# Amberite — Product Feature Specification

This is the canonical specification of what Amberite must let people do. It describes product behavior, not engineering tasks, code structure, or current implementation status. `PROJECT.md` explains the product at a high level; this file defines its complete feature surface.

Each identified line is a distinct capability or rule. The identifiers are stable references for planning and discussion, not task numbers or completion markers. Every planned feature belongs to exactly one release class: Version 1.0, Version 1.1, or Planned Future. Future features have no promised version or order.

## How to read this specification

A feature exists only when its complete user flow works through the intended surface. Backend support, a partial screen, mock data, disabled controls, or a happy-path demo do not make a feature complete. Every feature inherits the experience and authorization requirements below unless its section defines stricter behavior.

Version 1.0 and Version 1.1 sections are release specifications. Their identified features can be copied into the matching `TODO.md` without product decisions being made later. Planned Future sections record intended capabilities but are not requirements for either release. Product exclusions are not planned work.

### Product boundaries

Amberite is four connected products: the Desktop App, Copal Core, the cloud platform, and the Website. Modrinth remains the catalog and creator platform. Version 1.0 focuses on private friend groups and self-hosted servers; it does not attempt to become a hosting provider, public community platform, or replacement content ecosystem.

- **PB-01** — Amberite lets a private friend group self-host and play modded Minecraft without manually distributing mods, configs, addresses, or updates.
- **PB-02** — The Desktop App is the primary product for playing, managing groups, controlling Core, and administering servers.
- **PB-03** — Copal Core owns Minecraft servers, snapshots, files, backups, processes, connectivity checks, and management authorization.
- **PB-04** — The cloud platform owns Amberite identities, friends, groups, invitations, permissions, Core routing, notifications, user presence, and private Core heartbeats.
- **PB-05** — The Website explains and distributes Amberite and provides account, profile, friend, and group social features.
- **PB-06** — Modrinth remains the content catalog and creator platform (Amberite does not operate a competing mod catalog).
- **PB-07** — Publishing, moderation, creator management, and other Modrinth-owned actions open the correct Modrinth destination.
- **PB-08** — The 1.0 product supports a Windows Desktop App and Windows or Ubuntu Copal Core.
- **PB-09** — One group and one Core are the same ownership boundary in 1.0 (the UI presents a group while the platform enforces a Core boundary).
- **PB-10** — Private friend groups are the only group type in 1.0 (public communities and open servers come later).

### Completion standard for every feature

These are cross-product rules rather than standalone deliverables. They define what “implemented” means everywhere in the specification.

- **UX-01** — Every page uses real data and real actions rather than mock, placeholder, or disconnected controls.
- **UX-02** — Every action shows an immediate pending state and prevents accidental duplicate submission.
- **UX-03** — Every page has intentional loading, empty, offline, unauthorized, unavailable, success, and failure states where applicable.
- **UX-04** — Every failure says what failed, why it failed when known, and what the user can do next.
- **UX-05** — Every destructive action states what will be deleted and what will remain recoverable.
- **UX-06** — Every permission-restricted action is blocked in both the interface and the receiving system.
- **UX-07** — Limited members see a client-like experience, with useful context visible or disabled and irrelevant management screens removed.
- **UX-08** — Disabled controls explain the required access instead of appearing broken.
- **UX-09** — Navigation never leads to an unfinished, blank, misleading, or unsupported screen.
- **UX-10** — Data updates without requiring a full application reload.
- **UX-11** — Long-running work can be left and revisited without losing its status.
- **UX-12** — Sensitive Core routing, private activity, tokens, and account details never appear to unauthorized viewers.
- **UX-13** — The same action has consistent meaning, naming, and consequences wherever it appears.
- **UX-14** — A user never needs repository knowledge, a terminal, manual file distribution, or a copied server address for a normal product flow.

## Version 1.0 — Complete private-group play

Version 1.0 is the first complete product. A new Owner can create an Amberite identity, establish a private group, install or pair Core, create and manage a modded server, invite friends, publish the required client state, and play together. Admins receive a complete server manager; Limited members receive a launcher-like Update-and-Play experience.

### Account, social, and cloud platform

The cloud platform connects people and authorizes access without hosting their Minecraft servers. It owns identity, social relationships, private-group membership, durable notifications, user presence, Core routing, and private health records.

#### Identity and authentication

Minecraft identity is the trust anchor for Amberite. One verified account creates the person’s Amberite identity; additional verified accounts are launch identities owned by that same person.

- **ID-01** — Sign in to Amberite with a verified Minecraft account.
- **ID-02** — Automatically create or recover the Amberite identity attached to the verified Minecraft UUID.
- **ID-03** — Restore a valid Amberite session when the Desktop App reopens.
- **ID-04** — Explain expired or revoked sessions and return the user to sign-in safely.
- **ID-05** — Sign out of the current Amberite identity without deleting local Minecraft installations.
- **ID-06** — Sign in as another person only by signing out of the current Amberite identity.
- **ID-07** — Preserve the verified Minecraft handle as the identity name.
- **ID-08** — Allow an editable display name in addition to the verified Minecraft handle.
- **ID-09** — Show both names consistently anywhere identity ambiguity matters.
- **ID-10** — Add and verify additional Minecraft accounts as play identities owned by the same person.
- **ID-11** — Select a verified play identity before launching Minecraft.
- **ID-12** — Keep friends, groups, permissions, and notifications attached to the primary Amberite identity when switching play identities.
- **ID-13** — Remove a secondary play identity without deleting the Amberite identity.
- **ID-14** — Prevent one Minecraft account from being claimed by multiple Amberite identities.
- **ID-15** — Let the user review active Amberite sessions or signed-in devices.
- **ID-16** — Let the user revoke another active session.
- **ID-17** — Delete the Amberite account through a confirmed, understandable flow.
- **ID-18** — Explain how account deletion affects profiles, friendships, groups, play identities, and owned Core data.

#### Account settings and Modrinth connection

Account settings collect every person-level control in one place: profile, privacy, linked accounts, play accounts, notifications, blocks, sessions, sign-out, and deletion. Linking Modrinth is optional and does not become a second Amberite identity.

- **ID-19** — Provide one account-settings surface for profile, privacy, linked accounts, play accounts, notifications, blocked users, sessions, sign-out, and account deletion.

- **MR-01** — Browse, install, and update public Modrinth content without requiring a linked Modrinth account.
- **MR-02** — Link a Modrinth account through a real authorization flow.
- **MR-03** — Show the linked Modrinth identity and connection state in account settings.
- **MR-04** — Unlink Modrinth without breaking the Amberite identity, groups, servers, or installed content.
- **MR-05** — Open the linked Modrinth profile from Amberite.
- **MR-06** — Redirect project publishing and creator-management actions to Modrinth.
- **MR-07** — Explain when an action requires Modrinth linking and keep unrelated Amberite features usable.

#### User profiles and privacy

Profiles use a fixed Amberite layout so identity and actions remain predictable. Privacy is section-based and relationship-aware rather than one global public/private switch.

- **UP-01** — View a user profile using a shared, fixed Amberite profile layout.
- **UP-02** — Edit profile avatar, banner, display name, bio, and visible content.
- **UP-03** — Show the verified Minecraft handle separately from the editable display name.
- **UP-04** — Configure profile visibility by section and relationship audience.
- **UP-05** — Show only minimal identity information when a profile is private.
- **UP-06** — Let profile owners preview what public users, friends, and group members can see.
- **UP-07** — Make blocking override profile, presence, invitation, and relationship visibility.
- **UP-08** — Hide private group membership from viewers who are not allowed to see it.
- **UP-09** — Show relationship actions appropriate to the viewer (friend request, remove, block, invite, or no action).
- **UP-10** — Keep Favorite Modpacks and Recently Played history out of 1.0 (planned for the future social-profile expansion).

#### Presence, activity, and play invitations

Presence is compact rich activity similar to Discord presence inside the Modrinth-style App. It helps friends play together without turning Amberite into a general social network. Presence never doubles as Core or server health.

- **PR-01** — Show whether a friend is online or offline.
- **PR-02** — Show the duration of the current play session.
- **PR-03** — Show the current modpack when the player permits it.
- **PR-04** — Show the current server only when the viewer is allowed to know about that server.
- **PR-05** — Show a Join action only when the viewer has server access and the server can be joined.
- **PR-06** — Reduce restricted activity to a generic state instead of leaking a private group or server.
- **PR-07** — Stop or expire stale activity when the App or game disappears unexpectedly.
- **PR-08** — Keep user activity separate from Core and Minecraft server health.
- **PR-09** — Allow the user to hide current activity through privacy settings.
- **PR-10** — Do not store or expose a public gameplay history in 1.0.
- **PR-11** — Let a player invite an eligible friend to the server they are currently playing.
- **PR-12** — Deliver a play invitation as a durable, actionable notification rather than only a temporary popup.
- **PR-13** — Let the recipient accept, decline, or dismiss a play invitation.
- **PR-14** — Accepting a play invitation opens the normal install, update, account-selection, and Join flow; it never bypasses server access.
- **PR-15** — Prevent play invitations from revealing a private group or server to a friend who lacks permission to know about it.

#### Friends and blocking

Social functionality is utility-focused: find people, establish relationships, see permitted activity, and reach shared play. Blocking is authoritative across profiles, requests, invitations, and presence.

- **FR-01** — Find another user by their Amberite or verified Minecraft identity.
- **FR-02** — Send a friend request from search or a profile.
- **FR-03** — View incoming and outgoing friend requests separately.
- **FR-04** — Accept or decline an incoming friend request.
- **FR-05** — Cancel an outgoing friend request.
- **FR-06** — Prevent duplicate, self-directed, blocked, or already-resolved requests.
- **FR-07** — View the complete friends list with presence and current activity.
- **FR-08** — Open a friend’s profile from the friends list.
- **FR-09** — Remove an existing friend with clear confirmation.
- **FR-10** — Block a user from their profile or relationship settings.
- **FR-11** — View and unblock blocked users.
- **FR-12** — Prevent blocked users from sending requests, invitations, or viewing protected activity.
- **FR-13** — Receive a durable notification for a new or resolved friend request.
- **FR-14** — Keep direct messages, group chat, and social feeds out of 1.0.

#### Groups and group profiles

A 1.0 group is a private friend group and the social presentation of one Core ownership boundary. It is expected to be small but has no artificial fixed member cap.

- **GR-01** — Create a private friend group.
- **GR-02** — Establish the creator as the permanent Owner for 1.0.
- **GR-03** — Associate exactly one Core with the group.
- **GR-04** — View a group profile using the shared fixed profile layout.
- **GR-05** — Let the Owner edit the group name, avatar, banner, description, and visibility.
- **GR-06** — Show group members and their group roles.
- **GR-07** — Show the group’s Core and server availability without exposing routing details.
- **GR-08** — Show group servers only to members allowed to know about them.
- **GR-09** — Let non-owner members leave the group.
- **GR-10** — Prevent the Owner from leaving without deleting or unpairing the group.
- **GR-11** — Let the Owner unpair the Core while preserving recoverable Copal data.
- **GR-12** — Let the Owner delete the Amberite group without silently deleting Copal files.
- **GR-13** — Explain whether the group, cloud records, Core pairing, servers, or local files will remain before deletion.
- **GR-14** — Keep ownership transfer out of 1.0.
- **GR-15** — Keep public discovery, open joining, and public community groups out of 1.0.
- **GR-16** — Do not impose a fixed product-level member cap on private groups.

#### Group invitations and membership

Invitation approval happens before a recipient is bothered. Owner invitations are already approved; member-proposed invitations and invite-link requests require the Owner’s decision.

- **IN-01** — Let the Owner send a pre-approved direct group invitation.
- **IN-02** — Let any group member propose inviting another user.
- **IN-03** — Require Owner approval before the proposed recipient is notified.
- **IN-04** — Let the Owner approve or reject a proposed invitation.
- **IN-05** — Let the recipient accept or decline an approved invitation.
- **IN-06** — Create a reusable group invite link.
- **IN-07** — Enable, disable, rotate, and revoke the invite link.
- **IN-08** — Convert invite-link use into an Owner approval request instead of immediate membership.
- **IN-09** — Let the Owner approve or reject an invite-link join request.
- **IN-10** — Prevent expired, disabled, rotated, blocked, duplicate, or unauthorized invitation use.
- **IN-11** — Show incoming invitations and approval requests in the relevant group and notification interfaces.
- **IN-12** — Explain every invitation state to the proposer, Owner, and recipient without exposing private information.

#### Roles and per-server access

Amberite uses fixed roles at two levels. The group role governs group behavior and seeds new servers; each server then keeps its own copied access list so a person can be Admin, Limited, or have no access on that server.

- **AC-01** — Support exactly three fixed roles: Owner, Admin, and Limited.
- **AC-02** — Give the Owner control of Core pairing, connectivity, membership, group deletion, and server ownership.
- **AC-03** — Let Admins create and manage server instances, content, settings, snapshots, files, players, and backups.
- **AC-04** — Limit Limited members to discovering assigned servers, installing updates, launching, joining, and safe read-only information.
- **AC-05** — Use the group role as the default for non-server group actions.
- **AC-06** — Maintain a separate member-role list for every server.
- **AC-07** — Copy current group members and group roles into a newly created server.
- **AC-08** — Add a person who joins later to every existing server as Limited.
- **AC-09** — Let authorized managers promote a server member to Admin.
- **AC-10** — Let authorized managers demote a server member to Limited.
- **AC-11** — Let authorized managers remove a member’s access to a specific server without removing them from the group.
- **AC-12** — Keep the group Owner present on every server.
- **AC-13** — Recalculate visible navigation, pages, controls, status, and actions when access changes.
- **AC-14** — Remove access immediately from cloud discovery, Core requests, snapshots, server joining, and notifications.
- **AC-15** — Never rely on hidden or disabled UI as the actual permission check.
- **AC-16** — Keep custom roles and custom permission matrices out of 1.0.
- **AC-17** — Decide exact screen-level visibility while designing each surface, while preserving the fixed role model and the Limited member’s client-like experience.

#### Notifications and activity inbox

Important social, access, synchronization, and operational events persist on the account across devices. Notifications are an inbox with actions and destinations, not an ephemeral toast history.

- **NT-01** — Provide one durable notification center in the Desktop App.
- **NT-02** — Notify users about friend requests and their resolution.
- **NT-03** — Notify users about group invitations, proposed invitations, and join approvals.
- **NT-04** — Notify users when their group or server role changes.
- **NT-05** — Notify members when a required snapshot update is published.
- **NT-06** — Notify relevant managers about important server failures and crashes.
- **NT-07** — Notify relevant users when Core becomes unavailable or incompatible.
- **NT-08** — Keep actionable notifications until handled or dismissed.
- **NT-09** — Open the exact relevant screen or action from a notification.
- **NT-10** — Avoid repeatedly notifying users about the same unresolved event.
- **NT-11** — Mark notifications read individually or together.
- **NT-12** — Apply account notification preferences without hiding mandatory security or access information.
- **NT-13** — Notify a user about an eligible play invitation and let the notification enter the normal Join flow.

### Desktop App onboarding

The Desktop App is the primary Amberite surface. It combines the existing Modrinth-derived launcher with Amberite identity, private groups, synchronized servers, Core setup, and complete server management.

#### First-run onboarding

Onboarding adapts to three people: an Owner installing Core locally, an Owner pairing an existing remote Core, and a member who only needs to join somebody else’s group.

- **ON-01** — Explain Amberite’s group, Core, and seamless-play model before setup.
- **ON-02** — Complete Minecraft sign-in and Amberite identity setup.
- **ON-03** — Offer automatic local Core installation on the current Windows computer.
- **ON-04** — Offer pairing to an existing Core on another computer.
- **ON-05** — Allow setup to continue without a Core when the user is joining someone else’s group.
- **ON-06** — Show local installation download, configuration, startup, and pairing progress.
- **ON-07** — Recover cleanly when installation, elevation, startup, pairing, or connectivity fails.
- **ON-08** — Resume an interrupted onboarding flow without losing completed account work.
- **ON-09** — End onboarding on a useful Home state with one clear next action.
- **ON-10** — Make Core setup available later from the App when it was skipped.

### Copal Core

Copal is the self-hosted authority for Minecraft processes, server data, snapshots, files, backups, health, and management requests. The normal local path is automated by the App; standalone distribution supports a dedicated Windows or Ubuntu host.

#### Installation and pairing

Installing the program, pairing it to a group, unpairing it, and permanently deleting its data are separate operations. None may silently imply another.

- **CO-01** — Download and install Copal locally from inside the Windows Desktop App.
- **CO-02** — Configure Copal’s data location and required operating-system access during installation.
- **CO-03** — Start Copal automatically after a successful local installation.
- **CO-04** — Pair a locally installed Copal without requiring the user to copy an address or code.
- **CO-05** — Provide a Windows installer for a separate or remote Core machine.
- **CO-06** — Provide an Ubuntu installation path for a separate or remote Core machine.
- **CO-07** — Display a short pairing code when an unpaired remote Core starts.
- **CO-08** — Pair a remote Core by entering its address and pairing code in the Desktop App.
- **CO-09** — Explain wrong, expired, already-used, unreachable, or unauthorized pairing attempts.
- **CO-10** — Prevent one Core from being silently claimed by another group.
- **CO-11** — Show the paired Core identity, owning group, version, compatibility, and last heartbeat.
- **CO-12** — Reconnect to a known Core after an address or network change.
- **CO-13** — Reset broken connection credentials through an explicit recovery flow.
- **CO-14** — Unpair Core without deleting its server directory.
- **CO-15** — Repair the App’s local Copal installation without overwriting server data.
- **CO-16** — Uninstall the Copal program separately from permanent data deletion.
- **CO-17** — Keep container and Docker distribution out of 1.0.

#### Copal CLI

The CLI is a complete basic control surface for standalone installation and recovery. It is not expected to replace the App’s server-management UI.

- **CLI-01** — Install and perform initial Copal configuration.
- **CLI-02** — Start Copal.
- **CLI-03** — Stop Copal safely.
- **CLI-04** — Restart Copal.
- **CLI-05** — Show Copal service and process status.
- **CLI-06** — Show the current pairing code when pairing is available.
- **CLI-07** — Reset existing App connections and pairing state safely.
- **CLI-08** — Read recent Copal logs.
- **CLI-09** — Show the installed Copal version and compatibility information.
- **CLI-10** — Check for and apply a compatible Copal update.
- **CLI-11** — Check installation, configuration, storage, runtime, and network health.
- **CLI-12** — Repair recoverable Copal installation problems.
- **CLI-13** — Uninstall Copal without deleting server data by default.
- **CLI-14** — Require separate explicit confirmation for permanent data deletion.
- **CLI-15** — Return understandable success and failure output suitable for both people and automation.

#### Security, routing, and private health

The cloud platform stores private Core routing information and heartbeat state. Knowing an address never grants authority: every management request still requires authentication and action-level authorization on Core.

- **SE-01** — Register each paired Core with the cloud platform for its owning group.
- **SE-02** — Store the Core connection details privately in the cloud platform.
- **SE-03** — Return Core connection details only to authenticated users currently authorized for that Core or server.
- **SE-04** — Require Core to authenticate every remote management request.
- **SE-05** — Require Core to authorize the user against the requested group, server, and action.
- **SE-06** — Prevent a valid Amberite account without group access from reading or controlling Core.
- **SE-07** — Keep Core connection details out of public profiles, presence, group previews, and public website data.
- **SE-08** — Let Core send a minimal private heartbeat to the cloud platform.
- **SE-09** — Show authorized members the last known Core state without showing its address.
- **SE-10** — Distinguish online, offline, stale, incompatible, unreachable, and locally blocked states.
- **SE-11** — Keep server health separate from user presence.
- **SE-12** — Allow authorized Apps to run direct diagnostics when a heartbeat is stale or a connection fails.
- **SE-13** — Expire or revoke access after group removal, server removal, token revocation, or account deletion.
- **SE-14** — Accept that authorized direct players can discover the Minecraft server IP (address secrecy is not an authentication mechanism).
- **SE-15** — Keep management endpoints protected even when the Minecraft game endpoint is reachable.
- **SE-16** — Include the state of managed Minecraft servers in private Core health reporting without exposing private server details publicly.

#### Automatic networking and diagnostics

Networking is a guided product feature, not a documentation-only prerequisite. Amberite performs every safe automatic step it can, verifies the result, and names the failing layer when direct access cannot be established.

- **NW-01** — Detect when App and Core are on the same computer.
- **NW-02** — Detect when App and Core can communicate over the same local network.
- **NW-03** — Prefer the best reachable local route when available.
- **NW-04** — Request operating-system elevation only when firewall changes require it.
- **NW-05** — Add or repair the required Windows firewall rules.
- **NW-06** — Explain unsupported or blocked firewall automation without claiming success.
- **NW-07** — Attempt compatible automatic router port mapping.
- **NW-08** — Verify that the mapped Minecraft endpoint is externally reachable.
- **NW-09** — Verify that the authorized Core management path is reachable.
- **NW-10** — Identify whether failure comes from Core, local firewall, router mapping, carrier-grade NAT, provider blocking, address changes, or server configuration.
- **NW-11** — Give a specific correction path for the detected networking layer.
- **NW-12** — Recheck connectivity after the user applies a correction.
- **NW-13** — Remove obsolete automatically-created mappings when a server or Core no longer uses them.
- **NW-14** — Never describe a random hostname as hiding the owner’s IP.
- **NW-15** — Keep managed public-TCP relay fallback out of 1.0.
- **NW-16** — Keep AI networking diagnosis out of 1.0 (deterministic diagnosis is required).

### Desktop App home, library, and discovery

After onboarding, the App centers on playing, maintaining local instances, reaching group servers, and browsing Modrinth content. Management surfaces appear only when the selected server and role make them useful.

#### Home dashboard

Home is the player’s current-action dashboard. It prioritizes playing, updates, invitations, approvals, and failures over marketing or generic discovery.

- **HM-01** — Make Home the default play dashboard rather than a marketing screen.
- **HM-02** — Show the most relevant Continue Playing action.
- **HM-03** — Show installed instances that require an update before play.
- **HM-04** — Show groups with unavailable, stale, or incompatible Cores.
- **HM-05** — Show assigned Minecraft servers and their current state.
- **HM-06** — Show friends who are online and their permitted current activity.
- **HM-07** — Show pending friend requests, invitations, and Owner approvals.
- **HM-08** — Show recent important server, snapshot, and access events.
- **HM-09** — Let every Home item open the exact place where the user can act.
- **HM-10** — Personalize Home by role without exposing inaccessible controls or servers.

#### Library and local instances

The Library combines launcher instances and Amberite server entries while clearly communicating their different ownership, synchronization, and access models.

- **LB-01** — Show local client instances and synchronized server instances in one understandable library.
- **LB-02** — Distinguish ordinary client, custom client, synchronized server, and saved public-server entries.
- **LB-03** — Filter the library by instance type and availability.
- **LB-04** — Search the library by instance, modpack, group, or server name.
- **LB-05** — Sort instances by recent use, name, update state, or installation state.
- **LB-06** — Show installed version, game version, loader, update state, running state, and access state on each entry.
- **LB-07** — Open the correct client-like or management-oriented detail page based on instance type and role.
- **LB-08** — Create a normal local client instance.
- **LB-09** — Install a local client instance from a Modrinth modpack.
- **LB-10** — Import an existing compatible local client instance.
- **LB-11** — Duplicate a local instance without changing the original.
- **LB-12** — Rename and customize a local instance.
- **LB-13** — Repair a damaged local instance.
- **LB-14** — Remove a local instance with a clear data warning.
- **LB-15** — Keep the Worlds tab for ordinary local client instances.
- **LB-16** — Remove the unfinished top-level Worlds destination.
- **LB-17** — Omit the Worlds tab from server and synchronized-server views in 1.0.

#### Modrinth discovery and content

Amberite embeds the content journey needed to play, while Modrinth remains the source of project data, files, attribution, and creator-owned actions.

- **MD-01** — Search Modrinth mods and modpacks.
- **MD-02** — Filter results by supported game version, loader, content type, and compatibility.
- **MD-03** — View project identity, summary, description, gallery, authors, categories, compatibility, and links.
- **MD-04** — View available project versions and their supported game versions and loaders.
- **MD-05** — Explain required and optional dependencies before installation.
- **MD-06** — Select an existing compatible instance as the installation target.
- **MD-07** — Create a compatible client instance when no suitable target exists.
- **MD-08** — Install a compatible project version with visible progress.
- **MD-09** — Update installed Modrinth content while preserving compatible personal overrides.
- **MD-10** — Prevent incompatible content installation with a specific reason.
- **MD-11** — Open the original Modrinth project page.
- **MD-12** — Preserve visible Modrinth attribution throughout catalog experiences.

#### Public Modrinth servers

Public Modrinth server projects remain separate from private Core-managed servers. Saving or joining one does not create an Amberite group or imply management access.

- **PS-01** — Browse Modrinth public-server projects separately from private Amberite group servers.
- **PS-02** — View a public server’s description, address information, required modpack, compatibility, and current project details.
- **PS-03** — Save or unsave a public server in the user’s library.
- **PS-04** — Install the public server’s required compatible client profile.
- **PS-05** — Update the required profile before joining when necessary.
- **PS-06** — Select the Minecraft play identity used to join.
- **PS-07** — Join a public server without confusing it with an Amberite-managed Core server.
- **PS-08** — Explain unavailable, incompatible, outdated, or unreachable public servers.

### Server management

The App provides a functional manager for servers owned by Core, not only lifecycle buttons over backend routes. Owners and server Admins can create, operate, inspect, modify, repair, back up, and remove a server without using server folders or a terminal.

#### Server creation

Creation is a guided transaction: Amberite validates the source and environment, shows progress, and either produces a coherent server or rolls the attempt back.

- **SC-01** — Let Owner and permitted Admin users begin server creation.
- **SC-02** — Create a blank server by selecting Minecraft version and supported loader.
- **SC-03** — Create a server from a compatible Modrinth modpack.
- **SC-04** — Create a synchronized server from an existing local client profile.
- **SC-05** — Import an existing Minecraft server during server creation.
- **SC-06** — Import a whole-server backup during server creation.
- **SC-07** — Validate storage, runtime, game version, loader, content, and required files before committing creation.
- **SC-08** — Show install, download, extraction, configuration, and startup progress.
- **SC-09** — Roll back a failed creation without leaving a misleading usable server entry.
- **SC-10** — Name and identify the server independently from its installed modpack.
- **SC-11** — Copy current group members and roles into the new server.
- **SC-12** — Establish the group Owner as permanent server Owner.
- **SC-13** — Create the first canonical client snapshot when the creation source supports synchronization.
- **SC-14** — Explain when a server cannot produce a compatible client snapshot.

#### Overview and lifecycle

- **SV-01** — Show whether a server is stopped, starting, running, stopping, crashed, updating, restoring, or unavailable.
- **SV-02** — Show server uptime and current player count when running.
- **SV-03** — Start a stopped server.
- **SV-04** — Stop a running server gracefully.
- **SV-05** — Restart a running server safely.
- **SV-06** — Force-stop a stuck server only after warning about data risk.
- **SV-07** — Prevent conflicting lifecycle actions while another operation is active.
- **SV-08** — Show lifecycle progress without requiring the page to remain open.
- **SV-09** — Explain a crash and link directly to logs and repair actions.
- **SV-10** — Preserve the Minecraft process when the cloud platform or Desktop App is temporarily unavailable.
- **SV-11** — Keep management state understandable when Core is reachable but the Minecraft server is stopped.
- **SV-12** — Rename and edit safe server presentation details.
- **SV-13** — Delete a server through a confirmed flow that distinguishes cloud removal from local data deletion.

#### Console, logs, and failures

- **CL-01** — Stream live server console output to authorized managers.
- **CL-02** — Send a server command from the console.
- **CL-03** — Preserve readable console history across page navigation.
- **CL-04** — Search or filter the visible console output.
- **CL-05** — View Copal lifecycle logs for the selected server.
- **CL-06** — View Minecraft server logs from previous runs.
- **CL-07** — View crash reports and the files related to a failed start.
- **CL-08** — Download relevant logs for manual troubleshooting.
- **CL-09** — Separate normal warnings from errors that prevented startup or play.
- **CL-10** — Link known deterministic failures to the correct setting, file, content, runtime, or networking screen.
- **CL-11** — Keep AI crash and log explanation out of 1.0.

#### Players and Minecraft access

- **PM-01** — Show currently connected players.
- **PM-02** — Show the verified play identity connected to an Amberite member when known.
- **PM-03** — Kick a connected player.
- **PM-04** — Ban and unban a Minecraft identity.
- **PM-05** — Grant and revoke operator status.
- **PM-06** — View and edit the server whitelist.
- **PM-07** — Automatically whitelist verified play identities belonging to authorized Amberite members.
- **PM-08** — Remove automatic whitelist access when Amberite server access is revoked.
- **PM-09** — Preserve explicit bans even when the banned identity belongs to a group member.
- **PM-10** — Explain when Amberite access and Minecraft server access disagree.

#### Files

- **FL-01** — Browse the server filesystem inside the permitted server data boundary.
- **FL-02** — Open and edit supported text configuration files.
- **FL-03** — Upload files with visible progress and conflict handling.
- **FL-04** — Download a file or directory export.
- **FL-05** — Create a directory.
- **FL-06** — Rename or move a file or directory.
- **FL-07** — Delete a file or directory with a clear warning.
- **FL-08** — Prevent path traversal and access outside the server boundary.
- **FL-09** — Warn when editing or replacing a file while the server is running could be unsafe.
- **FL-10** — Refresh changed files without losing the user’s current location.
- **FL-11** — Explain permission, size, lock, conflict, and invalid-file failures.

#### Settings and runtime

- **ST-01** — View and edit supported `server.properties` values through clear controls.
- **ST-02** — Preserve unknown `server.properties` values when saving supported values.
- **ST-03** — Select a compatible installed Java runtime.
- **ST-04** — Detect when the required Java runtime is missing or incompatible.
- **ST-05** — Install or repair a managed Java runtime when supported.
- **ST-06** — Configure memory allocation and safe launch parameters.
- **ST-07** — Show which settings require a restart before taking effect.
- **ST-08** — Validate settings before replacing the last working configuration.
- **ST-09** — Restore the previous valid settings after a failed change.
- **ST-10** — Keep Core-owned settings on Core and available from every authorized App.
- **ST-11** — Keep scheduled tasks, macros, and scripting out of 1.0.

#### Content and modpack management

Content management classifies files before changing them and explains whether a change affects only the server or must also reach synchronized clients.

- **CM-01** — List installed server mods with resolved project identity where available.
- **CM-02** — Distinguish Modrinth content, local files, server-only overrides, and unknown files.
- **CM-03** — Search Modrinth for content compatible with the server’s game version and loader.
- **CM-04** — Install a compatible server mod.
- **CM-05** — Update an installed server mod.
- **CM-06** — Replace a mod with another compatible version.
- **CM-07** — Remove a server mod with dependency and synchronization warnings.
- **CM-08** — Detect required dependencies before changing server content.
- **CM-09** — Prevent client-only content from being installed as required server content.
- **CM-10** — Explain whether a content change affects only the server or must be published to clients.
- **CM-11** — Let an Admin publish compatible client-affecting changes as a new snapshot.
- **CM-12** — Keep member change proposals and voting out of 1.0.
- **CM-13** — Show the installed Modrinth modpack identity and version when the server was created from a known modpack.
- **CM-14** — Detect and present a compatible update for an installed server modpack.
- **CM-15** — Apply or replace a server modpack only after explaining file, configuration, world, and client-snapshot effects.

#### Backups and world boundary

Version 1.0 backs up the complete server but does not expose a dedicated server-world manager. Each server has one implicit world; importing an existing server or backup is a creation path, not a post-creation world-slot action.

- **BK-01** — Create a whole-server backup manually.
- **BK-02** — Name a backup and add short change notes.
- **BK-03** — Show creation time, source server, size, and compatibility information.
- **BK-04** — List all available backups stored by Core.
- **BK-05** — Download a backup outside Core.
- **BK-06** — Restore a selected backup to its server.
- **BK-07** — Create a recovery backup of the current state before restoration.
- **BK-08** — Stop or lock the server safely while backup or restore requires it.
- **BK-09** — Show backup and restore progress.
- **BK-10** — Recover safely from interrupted or failed backup restoration.
- **BK-11** — Delete a backup with explicit confirmation.
- **BK-12** — Import a backup as a new server through the server-creation flow.
- **BK-13** — Keep scheduled automatic backups out of 1.0 and prioritize them immediately afterward.
- **BK-14** — Do not support server-world upload, swapping, reset, or multiple world slots in 1.0.

### Synchronization and play

Copal is the canonical source for the client state required by each managed server. Admins publish deliberate versions; members install the current version and may keep compatible personal client-only changes outside it.

#### Snapshot publishing

- **SN-01** — Treat Copal as the canonical source for synchronized client snapshots.
- **SN-02** — Create a snapshot from the server’s client-required mods and configs.
- **SN-03** — Give each snapshot a version, name, creation time, author, and change notes.
- **SN-04** — Show the currently published snapshot for a server.
- **SN-05** — Show previous snapshots and their change notes.
- **SN-06** — Let Owner/Admin inspect changes before publishing a new snapshot.
- **SN-07** — Publish a new snapshot without requiring every member to be online.
- **SN-08** — Notify authorized members that an update is required.
- **SN-09** — Prevent Limited members from publishing or changing the canonical snapshot.
- **SN-10** — Keep server-only files and overrides out of the client snapshot.
- **SN-11** — Preserve enough snapshot state for late-joining and long-offline members to install the current version directly.
- **SN-12** — Explain why a server state cannot become a compatible client snapshot.

#### Member installation and updates

- **SY-01** — Show every assigned synchronized server in the member’s library.
- **SY-02** — Install the current client snapshot when the member first opens or joins the server.
- **SY-03** — Show Update instead of Play when the installed snapshot is outdated.
- **SY-04** — Require the current mandatory snapshot before joining.
- **SY-05** — Display download, verification, extraction, application, and completion progress.
- **SY-06** — Resume or safely restart an interrupted update.
- **SY-07** — Verify the installed snapshot before enabling Join.
- **SY-08** — Preserve local saves and unrelated client instances during synchronization.
- **SY-09** — Explain insufficient disk, missing access, unavailable Core, corrupt download, incompatible runtime, and invalid snapshot failures.
- **SY-10** — Remove access to future snapshots immediately when server access is removed.
- **SY-11** — Let an existing authorized installation remain understandable when the user is offline.
- **SY-12** — Avoid requiring the member to understand file paths, mod loaders, addresses, or manual mod installation.

#### Personal client overrides

- **OV-01** — Let a member add compatible client-only mods to a synchronized instance.
- **OV-02** — Let a member keep personal client configuration overrides.
- **OV-03** — Let a member keep personal resource packs.
- **OV-04** — Let a member keep personal shaders.
- **OV-05** — Preserve safe personal overrides across snapshot updates.
- **OV-06** — Keep personal overrides out of the canonical group snapshot.
- **OV-07** — Detect when a personal override becomes incompatible with a required update.
- **OV-08** — Disable or remove an incompatible override only with a clear explanation and recovery path.
- **OV-09** — Prevent a personal override from silently changing required server-compatible content.
- **OV-10** — Keep runtime keybind/config injection through a companion mod out of 1.0.

#### Server drift

- **DR-01** — Compare current server mods and configs with the published snapshot.
- **DR-02** — Identify known mods using embedded metadata instead of filenames alone.
- **DR-03** — Report added, removed, changed, unresolved, and configuration differences.
- **DR-04** — Warn about drift before starting while still allowing an authorized manager to continue.
- **DR-05** — Revert drifted mods and configs to the current snapshot.
- **DR-06** — Preserve a compatible difference as an explicit server-only override.
- **DR-07** — Publish compatible client-affecting differences as a new snapshot.
- **DR-08** — Ignore the warning for the current start while showing it again later.
- **DR-09** — Explain when a difference cannot be safely classified or published.
- **DR-10** — Limit 1.0 drift management to mods and configs.

#### Launch, Join, and whitelisting

- **JP-01** — Show Play when a local client instance is ready.
- **JP-02** — Show Update when a synchronized instance is behind the required snapshot.
- **JP-03** — Show Install when an assigned synchronized instance is not installed.
- **JP-04** — Show Join when the instance, access, Core, and Minecraft server are ready.
- **JP-05** — Select the verified Minecraft play identity used for launch.
- **JP-06** — Prepare the correct game, loader, content, configs, and personal overrides before launch.
- **JP-07** — Launch Minecraft through the normal Modrinth-derived launcher flow.
- **JP-08** — Supply the correct server connection without manual address entry.
- **JP-09** — Explain when Join is blocked by access, snapshot, runtime, Core, networking, whitelist, or server state.
- **JP-10** — Offer the exact available correction action from the blocked Join state.
- **JP-11** — Join an authorized friend’s current server from their activity when all requirements are satisfied.
- **JP-12** — Keep the user’s selected play identity consistent through automatic whitelisting and launch.

### Cross-product repair and recovery

Repair protects user-owned data first. Deterministic failures may be repaired automatically; unsafe or ambiguous failures stop with a precise manual recovery path.

- **RP-01** — Detect a missing or damaged local client installation.
- **RP-02** — Repair a client instance without deleting personal overrides or saves.
- **RP-03** — Detect missing or damaged managed server runtime files.
- **RP-04** — Repair managed server runtime or loader files without overwriting world data.
- **RP-05** — Detect Copal storage, permission, runtime, database, process, and configuration problems.
- **RP-06** — Offer safe automatic repair when the failure is deterministic.
- **RP-07** — Preserve original data before a repair that changes user-owned files.
- **RP-08** — Show manual recovery instructions when automatic repair is unsafe or impossible.
- **RP-09** — Keep server data recoverable when the App, Copal program, pairing, or group is removed.
- **RP-10** — Keep AI repair recommendations out of 1.0.

### Website

The 1.0 Website is complete for public information and account/social parity. It deliberately has no Core or Minecraft server-management dashboard.

- **WB-01** — Explain what Amberite is and who it is for.
- **WB-02** — Explain the relationship between the Desktop App, Copal Core, private groups, and Modrinth.
- **WB-03** — Provide official Desktop App and Copal downloads for supported platforms.
- **WB-04** — Publish installation, pairing, networking, server, sync, account, and recovery documentation.
- **WB-05** — Publish release notes and supported component compatibility.
- **WB-06** — Publish current platform service status.
- **WB-07** — Sign in with the same Minecraft-backed Amberite identity.
- **WB-08** — View and edit account and privacy settings supported on the web.
- **WB-09** — View and edit the user profile.
- **WB-10** — View other profiles according to privacy and relationship rules.
- **WB-11** — View friends and friend requests.
- **WB-12** — Send, accept, decline, cancel, remove, block, and unblock social relationships.
- **WB-13** — View group profiles and group membership.
- **WB-14** — Receive, accept, decline, approve, and reject group invitations available to the signed-in role.
- **WB-15** — Keep Core/server management, console, files, settings, snapshots, and backups out of the 1.0 Website.

### Releases, compatibility, and updates

The Desktop App, Copal, and Platform/Website release independently but advertise a shared compatibility protocol. Updates preserve running Minecraft servers whenever management components can no longer communicate temporarily.

- **RL-01** — Version Desktop App, Copal, and Platform/Web independently.
- **RL-02** — Maintain a shared compatibility protocol across independently released components.
- **RL-03** — Show the installed and available Desktop App version.
- **RL-04** — Prompt for normal Desktop App updates without blocking play.
- **RL-05** — Require a Desktop App update only for security or expired compatibility.
- **RL-06** — Show the installed and available Copal version to authorized managers.
- **RL-07** — Apply compatible Copal updates automatically.
- **RL-08** — Roll back Copal when an automatic update cannot start or remain healthy.
- **RL-09** — Coordinate incompatible updates by updating the App before Copal.
- **RL-10** — Keep an existing Minecraft server running when its management surface is temporarily incompatible.
- **RL-11** — Publish Windows Desktop App releases.
- **RL-12** — Publish Windows Copal installer releases.
- **RL-13** — Publish Ubuntu Copal releases.
- **RL-14** — Publish accurate release notes for the affected component.
- **RL-15** — Use component-specific releases so unrelated products are not republished together.
- **RL-16** — Operate one production environment for 1.0.
- **RL-17** — Keep a permanent staging environment out of scope until the team needs it.

#### Production delivery

Production delivery is part of the product promise: supported artifacts, updates, cloud services, and public pages must actually reach users together with accurate health and compatibility information.

- **DP-01** — Release each product independently without forcing unrelated products to publish again.
- **DP-02** — Turn a Desktop App release into a signed Windows installer and working update offered inside the App.
- **DP-03** — Turn a Copal release into supported Windows and Ubuntu downloads plus compatible update information.
- **DP-04** — Turn a platform release into the production account, social, permission, notification, presence, routing, and heartbeat services.
- **DP-05** — Turn a Website release into the production marketing, download, documentation, account, profile, friend, and group site.
- **DP-06** — Publish release notes and compatibility information with every affected product release.
- **DP-07** — Verify sign-in, social access, Core discovery, downloads, and service health after a production release.
- **DP-08** — Prevent a failed product release from being presented to users as available or compatible.
- **DP-09** — Allow a failed Platform or Website release to return to the last healthy production version.
- **DP-10** — Keep secrets and production-only configuration out of downloadable clients and public documentation.
- **DP-11** — Maintain local development and one production environment until a larger team makes staging worthwhile.

### 1.0 acceptance journeys

These journeys are the release definition, not a duplicate task list. Version 1.0 is not complete unless each journey works end to end through production surfaces.

- **QA-01** — A new Owner can sign in, install local Copal, create a group, create a modpack server, invite a friend, and play without manual file or address work.
- **QA-02** — An Owner can install Copal on another Windows or Ubuntu computer, pair it, and reach a useful diagnosis for every failed networking layer.
- **QA-03** — A friend can accept an invitation, receive Limited access, install the required snapshot, select a verified play account, and join.
- **QA-04** — An Admin can change compatible mods/configs, publish a named snapshot, and require members to update before joining.
- **QA-05** — An Admin can identify drift and choose revert, server-only override, publish, or ignore for the current start.
- **QA-06** — An Owner/Admin can manage lifecycle, console, players, files, settings, content, logs, repairs, and manual backups through completed UI.
- **QA-07** — A Limited member gets a clear client-like view and cannot perform or invoke management actions.
- **QA-08** — Removing server access immediately removes Core discovery, snapshot access, joining, activity detail, and automatic whitelisting.
- **QA-09** — A Core or server can keep running through temporary App, cloud, or management incompatibility.
- **QA-10** — Copal program removal, group deletion, and unpairing do not silently destroy server files.
- **QA-11** — The Website supports the complete promised account and social surface without pretending to manage servers.
- **QA-12** — No reachable route contains mock data, a dead control, an unexplained permission failure, or an unfinished placeholder presented as a feature.

## Version 1.1 — Automatic protection and member proposals

Version 1.1 is a fixed additive release. It requires the complete Version 1.0 product and adds exactly two feature groups: automatic whole-server backup protection and controlled member change proposals. Social-profile expansion, custom permissions, ownership transfer, public groups, web server management, relays, AI, automation, dedicated world management, communication systems, the Companion Mod, and peer-to-peer failover are Planned Future and are not required for 1.1.

Unknown nice-to-haves are not implicit 1.1 requirements. They enter 1.1 only if this section is deliberately amended. Defects are tracked as defects rather than being disguised as features, but Version 1.1 cannot ship with a known release-blocking regression in a Version 1.0 or Version 1.1 flow.

### 1.1 upgrade and stability contract

- **V11-RL-01** — Require every Version 1.0 capability and acceptance journey to remain complete in Version 1.1.
- **V11-RL-02** — Upgrade existing 1.0 users without losing or recreating accounts, friendships, groups, invitations, roles, notifications, or privacy settings.
- **V11-RL-03** — Upgrade existing 1.0 Cores without re-pairing them or losing routing, authorization, server registrations, settings, snapshots, or health state.
- **V11-RL-04** — Preserve existing Minecraft servers, worlds, files, manual backups, client instances, personal overrides, and whitelist access during the 1.1 upgrade.
- **V11-RL-05** — Keep supported 1.0 App and Core versions usable during the normal coordinated 1.1 update window.
- **V11-RL-06** — Explain and safely block an unsupported mixed-version action instead of corrupting state or silently failing.
- **V11-RL-07** — Resolve every known data-loss, authorization-bypass, broken-update, broken-restore, server-lifecycle, or join-flow regression before releasing 1.1.
- **V11-RL-08** — Keep a failed 1.1 App or Core update recoverable through the existing rollback and repair paths.

### 1.1 automatic whole-server backups

Automatic backups extend the complete 1.0 manual-backup lifecycle. They protect each server on a schedule while keeping manual and recovery backups under explicit user control.

- **V11-BK-01** — Provide automatic-backup settings for each Minecraft server.
- **V11-BK-02** — Enable or disable automatic backups independently for each server.
- **V11-BK-03** — Enable a safe default automatic-backup policy for servers created after upgrading to 1.1.
- **V11-BK-04** — Ask the Owner/Admin to review and enable a backup policy for servers that existed before 1.1 rather than silently changing their storage use.
- **V11-BK-05** — Select how frequently an automatic backup should run.
- **V11-BK-06** — Limit retained automatic backups by count.
- **V11-BK-07** — Limit retained automatic backups by age.
- **V11-BK-08** — Limit the storage automatic backups may consume for a server.
- **V11-BK-09** — Show the active policy, next expected run, last successful run, and last failure.
- **V11-BK-10** — Run an automatic-policy backup immediately without changing the schedule.
- **V11-BK-11** — Pause and resume a schedule without deleting its existing backups or settings.
- **V11-BK-12** — Run one missed backup at the next safe opportunity after Core was offline, without creating a backlog of repeated backups.
- **V11-BK-13** — Prevent scheduled backup, manual backup, restore, update, repair, and destructive server operations from overlapping unsafely.
- **V11-BK-14** — Produce a consistent, restorable backup when the server is running and explain any brief interruption required to protect world state.
- **V11-BK-15** — Return the server to its previous save and running state after the backup attempt succeeds or fails.
- **V11-BK-16** — Verify a completed backup before presenting it as restorable.
- **V11-BK-17** — Mark interrupted, incomplete, or corrupt backup attempts as failed rather than listing them as valid restore points.
- **V11-BK-18** — Label manual, scheduled, pre-restore recovery, and other safety backups distinctly in one backup history.
- **V11-BK-19** — Apply retention deletion only to scheduled backups covered by that policy.
- **V11-BK-20** — Never automatically delete a manual or pre-restore recovery backup.
- **V11-BK-21** — Remove the oldest eligible scheduled backups first when enforcing retention.
- **V11-BK-22** — Warn before configured retention is likely to exhaust available Core storage.
- **V11-BK-23** — Pause new scheduled backups and notify managers when safe storage is unavailable rather than deleting protected data or filling the disk.
- **V11-BK-24** — Notify relevant managers when scheduled backups repeatedly fail or remain paused.
- **V11-BK-25** — Download, restore, and explicitly delete a scheduled backup through the same controls and permission rules as a manual backup.
- **V11-BK-26** — Restore a scheduled backup through the existing pre-restore recovery-backup flow.
- **V11-BK-27** — Cancel a pending or running automatic backup without leaving a valid-looking partial backup.
- **V11-BK-28** — Keep 1.1 automatic backups on Core-controlled storage rather than expanding this release into external backup-provider integration.

### 1.1 member change proposals

Proposals let Limited members request shared mod or configuration changes without granting them direct server-management or snapshot-publishing access. Approval is a controlled handoff to the normal Admin workflow, not a vote and not an automatic live change.

- **V11-CP-01** — Let a Limited member with access to a server begin a change proposal for that server.
- **V11-CP-02** — Propose adding a Modrinth mod compatible with the server’s Minecraft version and loader.
- **V11-CP-03** — Propose updating an installed Modrinth mod to a compatible version.
- **V11-CP-04** — Propose removing an installed mod.
- **V11-CP-05** — Propose a compatible shared configuration change covered by snapshot synchronization.
- **V11-CP-06** — Identify the target server, current snapshot, requested change, proposer, and submission time.
- **V11-CP-07** — Let the proposer include a short reason or note.
- **V11-CP-08** — Validate the proposer’s current server access before submission.
- **V11-CP-09** — Validate game-version, loader, environment, dependency, and known compatibility requirements before submission.
- **V11-CP-10** — Explain an invalid proposal and prevent submission until its required information is valid.
- **V11-CP-11** — Detect an existing equivalent pending proposal instead of creating duplicates.
- **V11-CP-12** — Detect when a proposal conflicts with the current server content or another accepted change.
- **V11-CP-13** — Let the proposer review and submit a draft proposal without receiving file or server-management access.
- **V11-CP-14** — Let the proposer withdraw their own pending proposal.
- **V11-CP-15** — Track proposal states as pending, accepted, rejected, withdrawn, stale, applied, or failed.
- **V11-CP-16** — Let members view the status and decision information for proposals they submitted while they retain access to the server.
- **V11-CP-17** — Provide Owner/Admin users with one review queue for proposals on servers they manage.
- **V11-CP-18** — Notify relevant Owner/Admin users when a new valid proposal is submitted.
- **V11-CP-19** — Show reviewers the requested difference, dependencies, compatibility result, snapshot impact, and proposer note.
- **V11-CP-20** — Let an Owner/Admin accept or reject a pending proposal.
- **V11-CP-21** — Let the reviewer attach a short decision response without creating a general chat thread.
- **V11-CP-22** — Revalidate a proposal when the server or current snapshot changes before a decision is applied.
- **V11-CP-23** — Mark a proposal stale when it can no longer be applied to the current server state without review.
- **V11-CP-24** — Turn acceptance into a staged Owner/Admin change rather than immediately modifying a running server.
- **V11-CP-25** — Let the reviewing Owner/Admin choose the final compatible version or dependency resolution before applying an accepted proposal.
- **V11-CP-26** — Apply an accepted proposal through the normal content/configuration controls and publish client-affecting results through the normal snapshot flow.
- **V11-CP-27** — Mark the proposal applied only after the server change and required snapshot publication succeed.
- **V11-CP-28** — Preserve a failed accepted proposal with its failure reason and a safe retry or rejection action.
- **V11-CP-29** — Notify the proposer when the proposal is accepted, rejected, stale, applied, or fails during application.
- **V11-CP-30** — Prevent a Limited proposer from using proposals to execute commands, edit arbitrary files, change runtime settings, manage players, or bypass content permissions.
- **V11-CP-31** — Stop accepting new proposals from a member immediately when their server access is removed.
- **V11-CP-32** — Retain proposal audit information for managers after the proposer leaves while hiding private server information from the former member.
- **V11-CP-33** — Keep group voting and automatic approval thresholds out of Version 1.1.

### 1.1 acceptance journeys

Version 1.1 is complete only when all Version 1.0 acceptance journeys still pass and every journey below works end to end.

- **V11-QA-01** — Upgrade a populated 1.0 account, group, Core, server, snapshot, client installation, and manual-backup history without re-creation or data loss.
- **V11-QA-02** — Enable a schedule, observe a consistent backup run, enforce its retention policy, and restore the resulting backup safely.
- **V11-QA-03** — Miss scheduled time while Core is offline, reconnect, and create one catch-up backup without a backup storm.
- **V11-QA-04** — Exhaust safe backup storage, preserve protected backups and server data, pause scheduling, and deliver a useful manager notification.
- **V11-QA-05** — Submit a compatible mod proposal as Limited, review it as Admin, apply it, publish the required snapshot, and notify the proposer.
- **V11-QA-06** — Reject or withdraw a proposal without changing the server or snapshot.
- **V11-QA-07** — Change the server after proposal submission, force revalidation, and prevent a stale proposal from being applied silently.
- **V11-QA-08** — Remove the proposer’s server access and immediately prevent new proposals or protected server visibility.
- **V11-QA-09** — Confirm that Limited members cannot use proposals or backup visibility to invoke any Admin-only Core operation.
- **V11-QA-10** — Confirm that 1.1 introduces no known release-blocking regression in a Version 1.0 flow.

## Planned Future — No committed version

Everything below is intended product direction but is not required for Version 1.0 or Version 1.1. The order of these sections is not priority, and the presence of a feature does not promise a release date.

### Richer social profiles and activity

- **FS-01** — Let users select Modrinth modpacks as profile favorites without requiring a linked Modrinth identity.
- **FS-02** — Add, remove, and reorder Favorite Modpacks.
- **FS-03** — Show Favorite Modpacks through the shared fixed profile layout.
- **FS-04** — Control the audience of Favorite Modpacks separately from other profile sections.
- **FS-05** — Record recently played modpacks for the Amberite identity across verified play identities.
- **FS-06** — Show a recently played list with useful modpack and play-time information.
- **FS-07** — Keep private group and server identity hidden from viewers who only have activity-history access.
- **FS-08** — Let users choose the audience for recent play history.
- **FS-09** — Let users clear individual history entries or their complete stored history.
- **FS-10** — Let users control how long activity history is retained.
- **FS-11** — Expand current activity with richer modpack information while preserving the 1.0 server-access checks.
- **FS-12** — Keep social profile expansion inside the fixed profile system rather than creating a general social feed.

### Custom roles and ownership

- **FRL-01** — Create custom group roles beneath Owner.
- **FRL-02** — Name, describe, order, and visually identify custom roles.
- **FRL-03** — Configure explicit permissions for group membership and social actions.
- **FRL-04** — Configure explicit permissions for each server-management capability.
- **FRL-05** — Assign multiple members to a custom role.
- **FRL-06** — Override a member’s role for one server without changing their group role.
- **FRL-07** — Preview the effective permissions and visible UI for a role.
- **FRL-08** — Prevent any role configuration from removing the Owner’s recovery authority.
- **FRL-09** — Transfer group/Core ownership to an eligible verified member through confirmation by both people.
- **FRL-10** — Transfer server ownership, routing authority, and recovery responsibility without losing data or member access.

### Core portability and additional platforms

- **FCP-01** — Prepare a paired Core for migration without invalidating its recoverable server data.
- **FCP-02** — Move Core configuration, server registrations, permissions, snapshots, backups, and server files to another machine.
- **FCP-03** — Verify migrated data before making the destination Core authoritative.
- **FCP-04** — Roll back to the source Core when migration verification fails.
- **FCP-05** — Update cloud routing after a successful migration without requiring members to rejoin the group.
- **FCP-06** — Provide a supported container deployment for Copal.
- **FCP-07** — Support additional Linux distributions through maintained installation and update paths.
- **FCP-08** — Evaluate and add Desktop App platforms beyond Windows without reducing the Windows experience.
- **FCP-09** — Show platform-specific capability differences before installation.
- **FCP-10** — Keep one portable server-data model across supported Copal platforms.

### Public communities and open servers

- **FPG-01** — Create a public or discoverable group separately from the private-group model.
- **FPG-02** — Publish a community profile with rules, description, staff, and visible servers.
- **FPG-03** — Discover and search public Amberite communities.
- **FPG-04** — Request membership or join according to the community’s configured entry policy.
- **FPG-05** — Support public, approval-required, invite-only, and closed membership states.
- **FPG-06** — Give community staff moderation capabilities separate from Minecraft server administration.
- **FPG-07** — Ban or remove a user from a public community without affecting unrelated Amberite relationships.
- **FPG-08** — Publish an Amberite-managed server for eligible public discovery.
- **FPG-09** — Show public server requirements before installation or joining.
- **FPG-10** — Protect private Core management and routing details even when a Minecraft server is public.
- **FPG-11** — Scale membership, notifications, and presence without applying private-group assumptions.
- **FPG-12** — Keep private friend groups usable without requiring public-community behavior.

### Website server management

- **FWD-01** — View owned and joined Cores and servers through the Website.
- **FWD-02** — View private Core heartbeat, compatibility, networking, and server state according to permissions.
- **FWD-03** — Start, stop, and restart a server from the Website.
- **FWD-04** — Use the live console and historical logs from the Website.
- **FWD-05** — Manage players, whitelist, bans, and operators from the Website.
- **FWD-06** — Browse and edit permitted server files from the Website.
- **FWD-07** — Manage server settings, runtime, content, and snapshots from the Website.
- **FWD-08** — Create, download, restore, and delete backups from the Website.
- **FWD-09** — Match Desktop permission-aware hiding, read-only states, and disabled explanations.
- **FWD-10** — Avoid exposing Core credentials or unrestricted direct management access to the browser.

### Restricted managed networking fallback

- **FNR-01** — Offer managed public-TCP fallback only after direct networking diagnostics confirm that direct access is not practical.
- **FNR-02** — Explain relay limits, privacy, availability, and usage before enabling it.
- **FNR-03** — Let the Owner explicitly enable or disable fallback for an eligible server.
- **FNR-04** — Route Minecraft gameplay for authorized members through the fallback when required.
- **FNR-05** — Keep normal authentication, server access, and whitelist rules active through the fallback.
- **FNR-06** — Show fallback health and whether a player is using direct or relayed connectivity.
- **FNR-07** — Return to direct connectivity automatically when it becomes healthy.
- **FNR-08** — Keep snapshots, server files, and backups off a limited relay path.
- **FNR-09** — Avoid promising a specific provider until a suitable public raw-TCP service is selected.
- **FNR-10** — Explain that only proxying or relaying hides the origin address; a random hostname does not.

### AI-assisted troubleshooting

- **FAI-01** — Explain a Minecraft crash using the relevant logs and known server/client context.
- **FAI-02** — Explain a failed server start using runtime, loader, content, and configuration evidence.
- **FAI-03** — Explain difficult networking failures after deterministic diagnostics have completed.
- **FAI-04** — Distinguish evidence, likely causes, uncertainty, and suggested actions.
- **FAI-05** — Ask before sending logs or private diagnostic material to an external AI service.
- **FAI-06** — Remove tokens, addresses, and unrelated personal data from diagnostic context where possible.
- **FAI-07** — Let the user inspect what diagnostic information will be shared.
- **FAI-08** — Link recommendations to existing repair or settings actions when safe.
- **FAI-09** — Never perform destructive repair or configuration changes solely from an AI response.
- **FAI-10** — Keep deterministic diagnostics and manual recovery usable when AI is unavailable.

### Server scheduling, macros, and automation

- **FAT-01** — Schedule server start, stop, and restart actions.
- **FAT-02** — Schedule server commands and maintenance announcements.
- **FAT-03** — Show upcoming scheduled actions to affected managers and players.
- **FAT-04** — Prevent conflicting scheduled and manual operations from running unsafely.
- **FAT-05** — Pause, resume, edit, and delete a schedule.
- **FAT-06** — Create reusable server-management macros from permitted actions.
- **FAT-07** — Restrict macro capabilities to the permissions of their owner and execution context.
- **FAT-08** — Record automation runs, output, failures, and the initiating schedule or user.
- **FAT-09** — Disable repeatedly failing automation and notify managers.
- **FAT-10** — Prevent automation from escaping its server boundary or bypassing Core authorization.

### Dedicated server-world management

- **FWM-01** — Show the current server world as a managed object rather than an implicit filesystem detail.
- **FWM-02** — Upload or import a world into an existing server.
- **FWM-03** — Validate an imported world before making it active.
- **FWM-04** — Keep multiple world slots or archives for one server.
- **FWM-05** — Switch the active world through a controlled stopped-server flow.
- **FWM-06** — Create a safety backup before switching, replacing, resetting, or deleting a world.
- **FWM-07** — Reset a server world while preserving the previous world for recovery.
- **FWM-08** — Download or archive a world independently from a whole-server backup.
- **FWM-09** — Explain modpack, game-version, dimension, and server-setting compatibility risks.
- **FWM-10** — Restore the previous active world when a switch or import fails.

### Communication, announcements, and voting

- **FCM-01** — Publish a durable group announcement to members.
- **FCM-02** — Target an announcement to a group or a specific server audience.
- **FCM-03** — Provide group chat if real use demonstrates a need beyond announcements and notifications.
- **FCM-04** — Provide direct messages if they can be moderated and blocked consistently with existing privacy rules.
- **FCM-05** — Convert an eligible pending change proposal into a member vote.
- **FCM-06** — Let the Owner choose which proposals may be voted on.
- **FCM-07** — Define eligible voters and a visible approval threshold before voting begins.
- **FCM-08** — Prevent vote changes, expired membership, or duplicate identities from corrupting the result.
- **FCM-09** — Treat a successful vote as an Owner/Admin review signal rather than unrestricted server access.
- **FCM-10** — Keep communication features focused on organizing Minecraft play rather than building a general social network.

### Companion Mod

- **FMO-01** — Apply supported personal settings at runtime when filesystem replacement is unsafe or ineffective.
- **FMO-02** — Apply personal keybind preferences after the game loads.
- **FMO-03** — Preserve required server or modpack settings over conflicting personal preferences.
- **FMO-04** — Separate required, recommended, and personal runtime preferences.
- **FMO-05** — Restore or save supported personal changes when leaving a server.
- **FMO-06** — Explain which preferences require the Companion Mod and which work without it.
- **FMO-07** — Keep the game playable when the Companion Mod is absent unless a specific required capability depends on it.
- **FMO-08** — Match the installed Companion Mod version to the game, loader, and Amberite snapshot requirements.

### Peer-to-peer failover

- **FP2P-01** — Let an eligible group member opt in to caching encrypted recoverable server state.
- **FP2P-02** — Show cache freshness, storage use, and whether it can currently host a recovery session.
- **FP2P-03** — Detect that the authoritative Owner Core is unavailable before considering failover.
- **FP2P-04** — Elect exactly one temporary host and prevent two writable hosts from starting simultaneously.
- **FP2P-05** — Require appropriate group authority before a member can host a temporary server.
- **FP2P-06** — Tell players that a temporary host may contain older world state before they join.
- **FP2P-07** — Keep the Owner’s Core canonical when it returns unless an explicit safe recovery decision changes authority.
- **FP2P-08** — Detect divergent world state and prevent automatic destructive merging.
- **FP2P-09** — Preserve both conflicting states until an authorized person chooses a recovery path.
- **FP2P-10** — Return players and routing to the authoritative Core only after world state and host authority are safe.

## Product exclusions and non-goals

These statements protect the product boundary. They are not unfinished features.

- **NO-01** — Amberite does not build its own mod catalog, creator dashboard, or moderation platform.
- **NO-02** — Amberite does not promise that a hostname hides a self-hosted user’s IP.
- **NO-03** — Amberite does not expose private Core routing or server activity on public profiles.
- **NO-04** — Amberite does not treat backend-only behavior as a finished product feature.
- **NO-05** — Amberite does not make the Companion Mod, peer-to-peer failover, or a general social network a prerequisite for 1.0.
- **NO-06** — Amberite does not add small infrastructure conveniences ahead of broken core user flows.
