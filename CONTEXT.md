# Amberite instances and servers

Everything a player sets up is an instance, and every instance lives in one library. Hosting an
instance gives it a server, which is part of the instance, not a separate thing beside it. A server
can also stand alone, created on its own.

## Language

### Instances

**Instance**:
A Minecraft setup: its loader, game version, content, and configs. It is the only kind of entry in
the library.
_Avoid_: Profile, pack, client

**Library**:
The one list of every instance a person has, including instances that have a server.
_Avoid_: Servers library, server list

**Owner**:
The person who controls an instance's content and its server.

**Shared instance**:
An instance its owner has given to other people. Each of them has their own copy, which follows the
owner's pushes; they can only propose changes.

**Push**:
The owner publishing their current version of an instance to its server and to the people it is
shared with.
_Avoid_: Sync, instance update

**Duplicate**:
A new instance with the same content as another, with no server and shared with nobody.
_Avoid_: Clone, copy

### Servers

**Server**:
A Minecraft server running on a Core, with its own world, backups, and console. Usually it is the
hosted part of an instance and runs that instance's content; an instance has at most one. A server
can also stand alone, with no instance behind it.
_Avoid_: Linked server, server instance, server-typed instance

**Host**:
To give an instance a server. Removing the server leaves the instance as it was before.
_Avoid_: Create server, convert to server

**Core**:
The server manager an owner runs on a machine they control. Servers run on it.
_Avoid_: Copal (outside code), node, host machine

**Private server**:
A server reached through a shared instance. Only the people the instance is shared with see it, and
it appears in their library as a server.
_Avoid_: Shared server

**Public server**:
A server listed on Modrinth that anyone can find and join. Amberite shows it but does not host it.

## Relationships

- An instance has zero or one server. A second server with the same content means duplicating the
  instance and hosting the duplicate.
- A standalone server has no instance. Its content belongs to the server.
- An instance that has a server appears in the library as a server, for its owner and for everyone
  it is shared with.
- Sharing happens only on instances. Sharing an instance that has a server shares that server.
- Content belongs to the instance, or to a standalone server. The world, backups, and console
  belong to the server.
