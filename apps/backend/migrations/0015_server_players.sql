-- The players of a Core server: the people the server is shared with (docs/internals/plan.md,
-- "Shares, members, and players"). They read the share the server follows without being its
-- members. The Core's owner writes these from the App's Play tab.

-- A server that has players, and the share it follows: its instance's share, or its own
-- server-first share.
CREATE TABLE shared_servers (
  core_id TEXT NOT NULL REFERENCES cores(id) ON DELETE CASCADE,
  server_id TEXT NOT NULL,
  instance_id TEXT NOT NULL REFERENCES shared_instances(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  -- The Minecraft address players join, once the Core knows one.
  address TEXT,
  PRIMARY KEY (core_id, server_id)
);
CREATE INDEX shared_servers_instance ON shared_servers(instance_id);

-- Pending until the player accepts the invite, like a member.
CREATE TABLE server_players (
  core_id TEXT NOT NULL,
  server_id TEXT NOT NULL,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  joined_at TEXT,
  join_type TEXT NOT NULL CHECK(join_type IN ('invite','link')),
  notified INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (core_id, server_id, user_id),
  FOREIGN KEY (core_id, server_id) REFERENCES shared_servers(core_id, server_id) ON DELETE CASCADE
);
CREATE INDEX server_players_user ON server_players(user_id);

-- A link that makes whoever opens it a player of this server instead of a member of the share.
ALTER TABLE shared_links ADD COLUMN core_id TEXT;
ALTER TABLE shared_links ADD COLUMN server_id TEXT;
