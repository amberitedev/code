# Amberite glossary

This glossary is for product decisions, code, issues, pull requests, and agent communication. Prefer these terms over near-synonyms so the App, Core, cloud platform, and documentation describe the same system.

## People and roles

- **developer**: Ilai or another contributor directing repository work. This is different from an Amberite user.
- **user**: A person with an Amberite identity.
- **player**: A user acting through one verified Minecraft play account.
- **member**: A user who belongs to a group or has access to a particular server.
- **Owner**: The permanent Version 1.0 owner of one group/Core boundary. Controls pairing, membership, networking, and destructive group decisions.
- **Admin**: A manager who can operate assigned Minecraft servers, content, files, settings, players, snapshots, repair, and backups.
- **Limited**: A member whose normal path is install, update, launch, and join. Limited members do not perform server-management operations.
- **manager**: Product-language shorthand for an Owner or Admin when both can perform an operation.

## Products and services

- **Amberite**: The complete open-source product around self-hosted modded Minecraft: Desktop App, Copal Core, cloud platform, and website.
- **Desktop App** or **App**: The Windows launcher and management application. It is the primary Amberite product and the Version 1.0 Core-management surface.
- **Copal Core**, **Copal**, or **Core**: The self-hosted Rust server manager. Prefer **Copal** for the binary or implementation and **Core** for its role in the product.
- **cloud platform**: Durable identity, profile, friend, block, group, invitation, role, notification, pairing, routing, and authorization state, currently implemented with Convex.
- **realtime**: The Cloudflare Worker and Durable Object that hold short-lived desktop online presence. Realtime is not the durable social database and does not own Core health.
- **website**: The public product, download, documentation, release, status, account, profile, friend, and group surface. It does not manage Core servers in Version 1.0.
- **Modrinth**: The upstream platform and source for discovering projects, versions, dependencies, modpacks, and public-server listings. Amberite does not recreate Modrinth publishing or moderation.

## Identity and social state

- **Amberite identity**: The durable user identity anchored by one verified Minecraft account.
- **play account**: A verified Minecraft account a user can select for launching and joining. Switching play accounts does not switch the Amberite user.
- **Modrinth account link**: An optional connection to a Modrinth account. It is not the Amberite sign-in method.
- **group**: A private friend group associated with one Core ownership boundary in Version 1.0.
- **group role**: The Owner, Admin, or Limited default role held at group level.
- **server role**: The copied and independently adjustable role for one server. A later-joining group member starts as Limited on existing servers.
- **invitation**: A pre-approved offer sent by the Owner.
- **invitation proposal**: A member’s request for the Owner to invite someone. The recipient is not notified until approval.
- **invite link**: A revocable link that creates an Owner-approved join request. It never grants immediate membership.
- **presence**: A user’s short-lived online and permitted play activity.
- **Core health**: The private heartbeat-derived availability of Core. It is separate from user presence.

## Minecraft and synchronization

- **server** or **server instance**: One Minecraft server owned, stored, and run by Core.
- **client instance**: An ordinary local Minecraft installation in the App with no required group synchronization.
- **synchronized server**: A group server that publishes the client content its members must install.
- **synchronized instance**: The member’s local client installation associated with a synchronized server.
- **snapshot**: A named, versioned publication of client-required mods and configuration from Core.
- **current snapshot**: The one snapshot a new or returning member installs directly. Old snapshots are history, not an update chain the member must replay.
- **personal override**: Compatible client-only mods, personal configuration, resource packs, or shaders owned by one member and excluded from the shared snapshot.
- **server-only override**: Server content or configuration intentionally excluded from the client snapshot.
- **drift**: A difference between the running server’s relevant mods/configuration and the published snapshot.
- **public server**: A Modrinth-listed server that Amberite may help install and join but does not manage through the group’s Core.
- **backup**: A recoverable whole-server archive owned by Core.
- **world**: The one implicit world owned by a server instance in Version 1.0. Dedicated world-slot management is later work.

## Connectivity and authorization

- **pairing**: The one-time process that associates a Core with its Owner and group.
- **Core route**: Private connection information used by authorized clients to locate Core.
- **management port**: The authenticated Core API surface. It remains protected even if the Minecraft gameplay port is public.
- **Minecraft port**: The network endpoint players use to join the game server.
- **direct connectivity**: Same-machine, LAN, or verified public access without proxying gameplay through Amberite’s cloud platform.
- **authorization**: The server-side decision that a current identity and role may perform an operation. Hidden or disabled UI is not authorization.
- **revocation**: Removal of access that must take effect across discovery, routing, snapshots, joining, notifications, and whitelist state.

## Repository and development

- **primary checkout**: The normal long-lived Git checkout used as the local reference and, when appropriate, the cloud development deployment.
- **worktree**: A task-specific Git checkout with its own branch, environment, and runtime state.
- **upstream**: The `modrinth/code` Git remote and reference implementation.
- **fork delta**: Amberite’s intentional difference from upstream Modrinth code.
- **`.data/`**: Ignored state local to one checkout, including Convex, Core, App, and dev-runner data.
- **baseline**: An idempotent seeded development scenario such as `accounts` or `group`.
- **full stack**: The worktree-local Convex, Core, and Desktop App processes started together by `pnpm dev`.
- **contract**: A typed boundary shared between a producer and consumer, including validation and error behavior.
- **vertical slice**: One complete user path across every necessary UI, native, contract, backend, authorization, persistence, and recovery boundary.
