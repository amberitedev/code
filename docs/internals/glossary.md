# Glossary

> For current planning terminology and superseded decisions, read
> [planning memory](planning-memory.md#language-and-boundaries). Code references below describe
> existing implementations, not necessarily the intended backend architecture.

This is a living glossary for Amberite. It explains the terms used throughout the product and
codebase. For now, each term points to the code that currently defines or implements it. As the
architecture settles, those sources can become more detailed technical reports.

## Instances

### Instance / Profile

An instance, also called a profile in the Modrinth code, is one copy of Minecraft with its own mods,
content, and settings. Each instance is unique. It can be created from a modpack, imported from a
`.mrpack`, or shared with friends. A `.mrpack` contains the modpack metadata and files that cannot be
downloaded automatically.

Current model: [`Instance`](../../packages/app-lib/src/state/instances/model/instance.rs)

### Creator / Owner

The person who currently owns an instance, usually its creator. Ownership can be transferred to
another person.

Current model: [`sharedClients`](../../convex/schema.ts)

### Shared Instance

An instance that its owner has shared with other people. The owner can publish changes, and the
other people can review and install those updates.

Each person may add personal content without changing the shared setup. They can also unlink or
clone their copy to make it independent.

Current implementation: [shared instance client](../../packages/app-lib/src/api/instance/shared/client.rs)
and [`sharedClients`](../../convex/schema.ts)

### Personal Content

Client-side additions or changes that are not part of the original shared instance.

Current implementation: [instance content](../../packages/app-lib/src/state/instances/content.rs)

### Environment

Metadata describing where a project can be installed, such as the client, server, or both.

Current model: [Modrinth project types](../../packages/api-client/src/modules/labrinth/types.ts)

## Worlds and servers

### World

A Minecraft save belonging to an instance.

Current model: [`World`](../../packages/app-lib/src/api/worlds.rs)

### Server

A world hosted on someone's Core. A server is separate from the instance and describes the hosted
world and its metadata, not the instance's client content.

Only someone who owns the instance or has access to the shared instance can host its server.

Current model: [Core instance](../../apps/core/src/domain/instance.rs)

## Social

### Friend

Another Amberite user connected through an accepted friendship.

Current model: [`friendships`](../../convex/schema.ts)

### Friend Group

A list of friends who regularly play together. It is a quality-of-life social feature and is not
tied to a Core or instance.

Friend groups are not modeled in the code yet.

## Hosting

### Core / Copal

Core, also called Copal, is Amberite's self-hosted Minecraft server manager. It is the process and
CLI tool installed on a computer. After a user signs in and connects it to their account, Amberite
can use that computer to host servers.

Current implementation: [Core](../../apps/core) and [`coreList`](../../convex/schema.ts)

### Node

One computer running [Core / Copal](#core--copal) and connected to an Amberite account. A user may
connect several nodes so Amberite has several computers available for hosting servers. Each server
runs on one node, and Amberite chooses the node automatically unless the user needs manual control.

The account-level collection of nodes does not have a final name yet. The current code still models
each paired Core separately. The node model will be introduced when multi-computer Core management
is implemented.

Current model: [`coreList`](../../convex/schema.ts)
