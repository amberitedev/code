# Future Ideas

A scratchpad for features we want later but are not building right now.

## Auto-generated client-side performance pack (for server-only instances)

For a server-only instance, generate a matching client-side modpack focused on
performance/optimization so players connecting to the server get a good,
preconfigured client out of the box.

- Input: the server's loader + game version (and ideally its mod list).
- Output: a client-side pack containing performance/optimization mods
  (e.g. rendering, FPS, memory, connection-quality mods) compatible with the
  server, excluding server-only mods.
- Goal: one click to produce a "recommended client" for anyone joining the
  server, so the player doesn't have to assemble an optimized client manually.
- Open questions:
  - Where the mod selection list/heuristics live (curated list vs. tag-based
    query against the Modrinth API).
  - How this ties into the synced profile (auto-attach the generated client
    side to a synced profile created from a server instance).
  - Version/loader compatibility resolution and update flow.

Status: deferred. Not in active development.
