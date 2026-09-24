# Product model

> Historical product model. Read [planning memory](planning-memory.md) first for the current
> milestone, later corrections, and decision status. In particular, hosting UX, ownership,
> Core/Node terminology, and the open architecture questions below are not all current decisions.

Amberite is the Modrinth App with tools for sharing instances and hosting Minecraft servers on your
own computers.

This file explains how those parts fit together. It does not decide how they are implemented.

## Instances

An Instance is a configured Minecraft installation. It specifies the Minecraft version, mod loader,
mods, resource packs, data packs, configuration files, and scripts used to play. Modrinth also calls
an Instance a Profile.

An Instance also keeps track of the Worlds and Servers that use that configuration. Usually an
Instance is one modpack with one World, but it can have more than one World or Server.

There are no client Instances or server Instances. Players install the Instance on their computers.
Servers install the configuration needed to run Worlds from that Instance. These are two uses of the
same Instance, not two Instance types.

An Instance can stay local and work like a normal Modrinth profile. Core, Friends, Friend Groups, and
sharing are optional.

## Sharing

The owner of an Instance can share it with Friends. Each Friend gets their own local installation.
When the owner publishes a change, everyone with the Instance can update instead of downloading a
new modpack export.

Sharing an Instance lets someone play it. It does not let them change the shared mods and files.
That requires Contributor permission.

A player can add client-only mods and settings to their own installation. This is Personal Content.
It is not shared and cannot change what other players or Servers need.

A player can clone or unlink an Instance to make an independent copy. This copies the configuration
they already have. It does not copy Worlds they cannot access.

## Versions and updates

Publishing a change creates a new version of the Instance configuration. A player can only join a
Server when both use compatible versions.

Each Server keeps a copy of the Instance configuration on the computer where it runs. When a new
version is published, that copy must update too. A running Minecraft Server must restart before it
can use the new files.

The published Instance version is the official version. A Server uses it but cannot change it.

We have not decided exactly when player installations update or when Servers restart. Amberite must
make version mismatches clear and keep the normal case easy to play.

The Instance owner can publish changes. Contributors can publish too. We may later let other people
propose changes for review, but that system has not been designed.

## Worlds and Servers

A World is a Minecraft save. It contains the blocks, entities, inventories, player progress, and
other data created while playing.

A Server is a World that is running for multiplayer. It belongs to an Instance and uses that
Instance's configuration.

A Server also has files that belong only to that Server. These include `server.properties`, the
whitelist, operators, bans, logs, and backups. They are not part of the Instance configuration.

The person hosting a Server owns that Server and its World. The files are stored on their computer,
so Amberite cannot prevent them from accessing or copying the World. Hosting a Server does not make
them the owner of the Instance.

People who receive an Instance can normally join its Servers. We may later allow individual Servers
to have a smaller player list.

Worlds need stronger backup and recovery than Instance configurations. Every player has a copy of
the configuration, but the Server host may have the only current copy of a World. The rules for
deletion, transfer, recovery, and retention have not been decided.

## Core and Nodes

Core, also called Copal, is the program that runs Minecraft Servers. It can run inside the Amberite
App on a player's computer or on a computer that stays online.

A Node is one computer running Core. A user can connect several Nodes. Amberite treats them as one
set of computers available for hosting, while each Server runs on one Node at a time.

Amberite should pick a Node automatically. Users can still view their Nodes and manually choose one
when needed.

Adding a Node uses a pairing code. The user signs in, copies the code shown by the new Core, and
confirms it from Amberite. Amberite manages the connection after that.

Someone can only host a Server for an Instance already shared with them. They can offer to host it,
or the Instance owner can ask them. Both people must agree.

The Core owner controls their computer and can stop anything running on it. This does not give them
permission to change the Instance.

We do not have a name for a user's complete set of Nodes. We do not need one unless the UI needs to
refer to the set directly.

## Friends and Friend Groups

A Friend is another Amberite user you can share an Instance with or ask to host a Server.

A Friend Group is a saved list of Friends who play together. It makes it easier to share the same
Instance with the same people again.

A Friend Group owns nothing and grants no permissions. It is not connected to Core or Nodes. A user
can join several Groups, and Amberite works without them.

## Ownership and permissions

Different people can own different parts of the same setup:

- The Instance owner controls its shared configuration.
- The Server host owns the Server and its World.
- The Core owner controls the computers running their Nodes.
- Friend Groups own nothing.

The current permission model has three roles:

- Members can install, update, and play the Instance. They can also change their Personal Content.
- Admins can manage Servers.
- Contributors can change and publish the Instance configuration.

The Instance creator starts as its owner and a Contributor. These roles apply to the Instance and
all of its Servers. We do not need separate roles for every Server.

The full permission list, ownership transfer, and proposed changes still need to be designed.

## The Instance screen

The Instance screen is where a user plays and manages an Instance. A regular player may only need
the Play button. The same screen also contains the content, people, history, Worlds, and Servers for
users who want to manage them.

Amberite should not split this into separate client and server Instance screens. The information can
still be separated where needed:

- Content shows whether it is installed on the client, server, or both.
- Personal Content is shown separately from shared content.
- Worlds and Servers are managed together.
- Activity shows changes to the Instance.
- Sharing shows who has the Instance and what they can do.
- Server pages show status, console, logs, backups, properties, and files.

Permissions change which actions a user can perform. They do not change what an Instance is or hide
the basic structure from regular players.

## Rules

- There is one kind of Instance.
- An Instance configuration and a World are different things.
- A Server is a hosted World from an Instance.
- Instances work without Core, sharing, or Friend Groups.
- Sharing does not require a Friend Group.
- Hosting requires access to the Instance and agreement from both people.
- A Server host does not own the Instance.
- Friend Groups own nothing and do not control Core access.
- Personal Content cannot change the files required for multiplayer.
- Players and Servers need compatible Instance versions.
- A Server must restart before using an updated configuration.
- Servers use published Instance versions. They do not create them.

## Open architecture questions

The largest unanswered question is where Amberite stores published Instance versions and how it
sends them to players and Nodes without paying for too much storage or bandwidth.

We still need to decide:

- what files belong to a published version;
- how versions are identified;
- where version metadata and files are stored;
- what Modrinth can continue hosting;
- how players and Nodes download files;
- which backend owns versions, permissions, Server state, and Node state;
- how Nodes update after being offline;
- how publishing and Server restarts work together;
- what Core can recreate and what must be backed up;
- how Amberite chooses between several Nodes;
- what can wait until after the first release.
