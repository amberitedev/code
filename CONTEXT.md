# Amberite instance and server content

An instance defines a Minecraft setup. Linked servers inherit that setup while keeping their own
worlds and server-specific state.

## Language

**Instance**:
A Minecraft setup containing its installation choices and content. It can be local, shared with
other people, and used by linked servers.

**Linked server**:
A Minecraft server whose base installation and content come from an instance or modpack.

**Unlinked server**:
A Minecraft server with independently managed installation and content, disconnected from source updates.

**Instance update**:
A version of an instance's setup pushed by someone allowed to change it. Linked servers receive
its server content, and shared player installations receive its client content.

**Server override**:
A content exception belonging to one server rather than its source instance or modpack.

**Public server**:
A linked server visible and joinable by everyone who has access to its shared instance. Public here
does not mean listed publicly on the internet.

**Private server**:
A linked server visible only to the people selected for it, within the instance's access rules.
