-- Servers a Core shows to the people an instance is shared with. The Core that runs a server
-- writes its rows with its own key. Online state and player counts are not stored: the App pings
-- `address`, the server's Minecraft address, itself.
CREATE TABLE visible_servers (
  core_id TEXT NOT NULL REFERENCES cores(id) ON DELETE CASCADE,
  server_id TEXT NOT NULL,
  name TEXT NOT NULL,
  address TEXT NOT NULL,
  instance_id TEXT NOT NULL REFERENCES shared_instances(id) ON DELETE CASCADE,
  PRIMARY KEY(core_id, server_id)
);
CREATE INDEX visible_servers_instance ON visible_servers(instance_id);
