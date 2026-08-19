# Modrinth + Amberite sharing

The short version is that all the pieces already exist. Modrinth has the friend system, Shared Instances, and most of the UI. Amberite has Convex, Core, archives, Minecraft server management, and an early version of sharing. The goal is to connect those pieces into one working system.

When the two versions disagree, Modrinth wins unless Ilai says otherwise. We should copy their behavior and reuse their UI wherever it makes sense, then move the hosted parts onto our own Convex and Core implementation.

A shared instance is the client content. It can exist on its own, with the option to create or attach a Minecraft server from its settings. Core manages those servers.

Sharing has one owner and normal members. Friends make people easier to find, while access comes from the shared instance invitation. Friend groups can come later.

Publishing is explicit. The owner changes their local instance and publishes when they are ready. That creates a new shared version for everyone else to install or update to. Local changes stay local until they are published. People can also launch the last shared version while keeping their own local changes untouched.

When Core is offline, the app says it is offline and operations that need it wait until it comes back.

The app handles what the user sees. Convex keeps the users, friends, invites, and who should have access. Core keeps the actual files, manages Minecraft servers, and applies the access for real.

We are doing this as a series of small worktree PRs into `feat/modrinth-shared-instances`. Before each one starts, we explain exactly what it is doing and settle any real product questions. Once the PR is open, we review and babysit it before moving on to anything that depends on it.
