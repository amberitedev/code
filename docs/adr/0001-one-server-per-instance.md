# One library, one server per instance

Servers used to live in their own library, linked to instances, with many servers allowed per
instance. That made one pack appear in several places and left unclear which row was the real one.
We now keep one library of instances: a server is the hosted part of an instance, an instance has
at most one, and an instance with a server is shown as a server. A second server with the same
content is a duplicated instance.

## Considered Options

- **Two libraries** (instances and servers): clearer split, but the same pack lives in both or jumps
  between them, and search, sort, and groups exist twice. The split is kept as a library grouping
  instead.
- **Instance becomes a server**: the owner's copy is still a client they play, and a friend's copy
  never runs a server, so "becomes" is false on every machine but the Core.
- **Server as a sibling entity**: brings back two entries per pack inside one library.
- **Many servers per instance**: Play and the library row would have to ask which server.
