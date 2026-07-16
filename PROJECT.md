# Amberite — Product Definition

## What Amberite is

Amberite is an open-source platform for self-hosting modded Minecraft with a private friend group.
The product removes the repeated technical work between choosing a modpack and playing together. The person hosting the server gets complete control without operating everything through terminals and folders. Everyone else receives the correct client, updates, access, and connection automatically.
The central promise is simple: an Owner or Admin manages the server and publishes what the group should use; a Limited member clicks Update and then Play.
Amberite is not a hosting company. The Minecraft server runs on a computer controlled by the group. Amberite supplies the management, synchronization, identity, permissions, and play experience around that self-hosted machine.
Amberite is also not a replacement for Modrinth. Modrinth remains the source for discovering projects, versions, dependencies, modpacks, and public-server listings. Amberite turns that content into a private self-hosted play experience.

## Who the product is for

Amberite is designed for small private groups where one person is willing to host and one or more people want to play without managing technical details.
The Owner controls the group, its Core, connectivity, membership, and permanent decisions.
Admins understand modpacks or server management and can operate server instances, content, settings, files, snapshots, players, and backups.
Limited members primarily install, update, launch, and join. Their experience should feel close to using an ordinary Minecraft launcher profile.
The product is not designed around large public communities in 1.0. Public groups, open joining, and large community permission systems can be added later without complicating the first private-group experience.

## The four products

### Desktop App

The Desktop App is the main Amberite product.
It combines the familiar Modrinth launcher experience with Amberite accounts, friends, groups, synchronized servers, Core setup, server management, notifications, and one-click joining.
Players browse Modrinth content, manage local client instances, receive group servers, install required updates, select a verified Minecraft account, and launch the game.
Owners and Admins use the same App to install or pair a Core, create servers, control processes, manage content, publish snapshots, inspect logs, manage players, edit files and settings, repair problems, and handle backups.
The App should never present an unfinished management screen as a working feature. Backend capability only becomes a product feature when the complete interface, permissions, state handling, and recovery path work.
The 1.0 Desktop App supports Windows.

### Copal Core

Copal Core is the self-hosted server manager.
It runs on the machine that hosts the group’s Minecraft servers. It owns server processes, server files, settings, content, logs, backups, synchronization snapshots, health checks, and authorized management requests.
Core continues running Minecraft servers even when the Desktop App closes or the cloud platform is temporarily unavailable.
Core can be installed automatically on the same Windows computer as the App, installed separately on another Windows computer, or installed on Ubuntu.
A separately installed Core shows a short pairing code. The Owner enters the Core address and code in the App, after which the Core belongs to that Owner’s group.
Removing the App, unpairing Core, or deleting a cloud group must not silently delete Minecraft server data. Program removal and permanent data deletion are separate decisions.

### Cloud platform

The cloud platform connects people without hosting their Minecraft servers.
It stores Amberite identities, verified play accounts, friends, blocks, groups, invitations, roles, notifications, user presence, Core routing, and private Core heartbeat state.
One group and one Core are the same ownership boundary in 1.0. The interface presents a social friend group while the platform treats membership as access to that group’s Core.
Core connection details are private. They are returned only to authenticated users who currently have the required group or server access.
Core health comes from private Core heartbeats. User online and play activity comes from the user presence system. These are separate signals and must not be confused.
The cloud platform does not proxy ordinary Minecraft gameplay, snapshots, files, or backups in 1.0.

### Website

The Website explains Amberite, provides downloads and documentation, publishes releases and service status, and supports the social account surface.
Users can sign in, manage their account and privacy, edit profiles, manage friends, view groups, and handle invitations through the Website.
The 1.0 Website does not manage Core servers, consoles, files, settings, snapshots, or backups. Those capabilities remain in the Desktop App until a later web-management update.

## Identity and accounts

Minecraft identity is the only Amberite sign-in method.
One verified Minecraft account anchors one Amberite identity. The verified Minecraft handle remains visible, while the user may also choose an editable display name.
A person may attach additional verified Minecraft accounts as play identities. They choose which one launches the game without changing their friends, groups, profile, permissions, or notifications.
Another person using the same computer signs out and uses their own Amberite identity. Play-account switching is not shared-person account switching.
Verified ownership of every play identity is stored by the platform so Core can grant the correct whitelist access and remove it when authorization changes.
A Modrinth account may be linked, viewed, and unlinked. Linking is optional for ordinary Amberite use.
Modrinth-owned actions such as publishing, creator management, and moderation open Modrinth. Amberite does not recreate those systems.

## Profiles, privacy, and presence

User and group profiles use a consistent fixed layout rather than a freeform page builder.
User profiles may contain an avatar, banner, display name, verified Minecraft handle, bio, relationship actions, and permitted activity.
Group profiles may contain an avatar, banner, name, description, members, permitted server information, and Core availability.
Profile sections use relationship-aware visibility. A private profile shows only the minimum identity needed for safe social interaction.
Blocking overrides friendship, invitations, presence, activity, and protected profile visibility.
The 1.0 presence system shows online state, current session duration, current modpack, and the current server when the viewer is allowed to know about that server.
A Join action appears only when the viewer has server access and can satisfy the server’s installation and update requirements.
Favorite Modpacks, Recently Played history, and richer profile activity are planned future features with no committed release.
Chat, direct messages, and a general social feed are not required for 1.0. Amberite social features exist to help friends organize and play.

## Friends and groups

Users can find one another, send and resolve friend requests, view permitted presence, remove friends, block users, and invite friends into groups.
Groups are private in 1.0 and are normally small. There is no product need for a restrictive fixed member cap.
The Owner can invite someone directly. That invitation is already approved.
Any group member can propose inviting someone, but the Owner must approve the proposal before the recipient is notified.
Groups also have an invite link that the Owner can enable, disable, rotate, or revoke. Opening the link creates a join request for Owner approval; it never grants immediate open membership.
The Owner alone edits the group profile and controls group deletion or Core unpairing.
Members can leave. The Owner cannot leave in 1.0 because ownership transfer is not yet supported.
Deleting a group removes the Amberite relationship and access state but leaves Copal data recoverable unless it is separately and explicitly deleted.

## Roles and permissions

Amberite 1.0 uses three fixed roles: Owner, Admin, and Limited.
The Owner created and owns the group/Core boundary. The Owner controls Core pairing, networking, membership, permanent group actions, and server ownership.
Admins can create and manage server instances, publish snapshots, change content and settings, use the console, manage players, work with files, repair servers, and manage backups.
Limited members install, update, launch, and join assigned servers. They may see useful status and read-only context but cannot invoke management operations.
The group role is the default for group-level behavior. Every Minecraft server also owns its own copied member list and server roles.
When a server is created, current group members and roles are copied to it. The Owner is always present.
When a person joins the group later, they are added to existing servers as Limited even if their group role is Admin. A manager can then promote them or remove their access for each server.
Removing someone from one server does not remove them from the group. Removing them from the group removes access everywhere.
The UI combines hidden screens, read-only information, and visibly disabled actions according to what helps the user understand the product. Disabled actions explain the required role.
Core and cloud authorization enforce every restriction independently of what the interface displays.
Custom roles, custom permission matrices, and ownership transfer come later.

## Installing and connecting Core

The most common 1.0 setup is automatic local installation from the Desktop App.
The App downloads Copal, configures it, starts it, pairs it, and reports each stage without requiring the user to operate a terminal.
The second setup path installs Copal separately on a Windows or Ubuntu machine and pairs it to the App with an address and short code.
The Copal CLI supports installation and configuration, start, stop, restart, status, pairing codes, connection reset, logs, version, update, repair checks, repair, and safe uninstall.
Core publishes a private heartbeat so authorized group members can see whether it is online, offline, stale, incompatible, or unreachable.
The cloud platform stores the Core details needed to locate it. Only authorized users receive those details and only authenticated, authorized users can use Core management functions.
Authorized direct players may learn the Minecraft server IP. A random domain does not hide an origin IP and is not treated as a security feature.

## Networking

Amberite first detects same-machine and local-network connectivity.
For remote direct access, it can request elevation, configure supported firewall rules, attempt automatic router port mapping, and verify external reachability.
Networking failures are diagnosed by layer. The user should learn whether Core is stopped, a firewall is blocking traffic, router mapping failed, carrier-grade NAT is present, the provider blocks traffic, the address changed, or the Minecraft server is misconfigured.
The result is a specific correction and a way to retry, not a generic connection error.
Version 1.0 does not promise a tunnel fallback. A future restricted managed public-TCP fallback may be added for users who genuinely cannot accept direct connections.
Files, snapshots, and backups should not be sent over a limited future relay path.
AI may later explain difficult networking and crash problems, but deterministic diagnostics must work first.

## Server creation and management

An authorized manager can create a blank server, create from a Modrinth modpack, create a synchronized server from a local client profile, or import an existing server or backup.
Server creation validates the Minecraft version, loader, runtime, content, storage, and source before presenting the server as usable.
Managers can start, stop, restart, and force-stop servers with clear state and progress.
The server manager includes live console access, historical logs, crash information, connected players, whitelist, bans, operators, files, server properties, Java/runtime settings, memory, content, repair, and backups.
File management supports browsing, safe text editing, upload, download, rename, move, and deletion inside the server boundary.
Content management identifies Modrinth projects where possible and separates known content, local files, server-only overrides, and unresolved files.
Manual whole-server backups support naming, notes, listing, download, restore, and deletion. Restore creates a recovery backup of the current state first.
Scheduled automatic backups are a high-priority 1.1 feature.
Each server instance owns one implicit world in 1.0. Dedicated world upload, swapping, resetting, and world-slot management come later.
The existing Worlds experience remains only for ordinary local singleplayer client instances. Server and synchronized-instance views do not show it.
Scheduled server actions, macros, and scripting are not 1.0 features.

## Snapshots and synchronized play

Copal is the canonical source for synchronized client snapshots.
An Owner or Admin changes the server’s mods or configs and publishes a named, versioned snapshot with change notes.
The snapshot contains what clients require, while server-only files and personal member overrides remain separate.
An assigned member sees the synchronized server in their library. The first use installs the current client snapshot automatically.
When a new required snapshot exists, Update replaces Play. The member must update successfully before joining.
Late-joining and long-offline members can install the current snapshot directly without replaying old changes.
Members may keep compatible client-only mods, personal configs, resource packs, and shaders. These survive snapshot updates and do not alter the group snapshot.
If a personal override becomes incompatible, Amberite explains the conflict and gives a recovery path instead of silently breaking the instance.
Core compares the server’s mods and configs with the published snapshot and identifies known mods through embedded metadata rather than filenames alone.
Drift warns but does not automatically prevent startup. A manager may revert to the snapshot, preserve the difference as a server-only override, publish compatible changes as a new snapshot, or ignore the warning for the current start.
Member change proposals arrive in 1.1. Voting comes later if proposals prove useful.

## Home, library, discovery, and joining

Home is a play dashboard showing Continue Playing, required updates, Core health, server state, friends online, invitations, approvals, and recent important activity.
The Library combines ordinary client instances and synchronized group servers while clearly distinguishing their types and available actions.
Users can search, filter, and sort the Library and see installation, version, update, running, synchronization, and access states.
Modrinth discovery supports searching projects, checking compatibility and dependencies, selecting a version, installing into a compatible instance, and updating installed content.
Modrinth public-server projects remain separate from private Amberite group servers. Users can browse them, save them, install or update their required profile, select a play account, and join.
Launching a synchronized server prepares the required game, loader, content, configs, and personal overrides, then supplies the correct server connection automatically.
Join explains exactly what is missing when it is unavailable, including access, snapshot, runtime, Core, networking, whitelist, or server state.

## Notifications

The notification center is durable across sessions and devices.
It contains friend requests, group invitations, proposed invitations, approval requests, role and access changes, snapshot updates, important server events, crashes, Core failures, and compatibility requirements.
Actionable notifications remain until handled or dismissed and open the exact relevant screen.
Repeated reports of the same unresolved condition are grouped rather than creating noise.

## Releases and product compatibility

The Desktop App, Copal Core, and Platform/Web have independent versions joined by a shared compatibility protocol.
Normal Desktop App updates are prompted and optional. Security or expired compatibility may require an update.
Compatible Copal updates install automatically and roll back if the new version cannot become healthy.
An incompatible update is coordinated App first and Copal second. Existing Minecraft servers continue running if management is temporarily unavailable.
Releases are component-specific so changing one product does not force every product to publish a new version.
Amberite begins with local development and one production environment. A permanent staging environment can be added when the project team and release needs justify it.

## Roadmap after 1.0

Version 1.1 is a fixed additive release containing automatic whole-server backup scheduling, retention and recovery, plus controlled member mod or configuration proposals. It also requires a safe upgrade from the complete 1.0 product without data loss or release-blocking regressions.
Planned future work has no committed version. It includes richer social profiles and play history, custom roles, ownership transfer, portable Cores, containers, more operating systems, public communities, web server management, restricted relay fallback, AI troubleshooting, server automation, dedicated world management, voting, chat, and other social features.
The Companion Mod is intentionally very low priority. It is only needed for runtime behavior that cannot be handled safely through ordinary files.
Peer-to-peer failover is also a distant feature. It requires safe host authority, conflict handling, and split-brain prevention before it can be trusted with a world.

## Definition of 1.0

Amberite 1.0 is complete when a new Owner can sign in, install or pair a Core, create a group and modded server, invite a friend, publish the required client state, and play together without manually distributing files or addresses.
It is complete when an Admin can operate the entire server through finished interfaces rather than backend-only capabilities or placeholder screens.
It is complete when a Limited member receives a simple client-like experience and cannot see or use actions outside their access.
It is complete when account settings, profiles, friends, groups, joining, leaving, invitations, approvals, permissions, presence, notifications, Core health, networking, servers, synchronization, backups, Website social features, releases, and recovery behave as one connected product.
It is not complete merely because individual backend systems can perform those actions in isolation.
