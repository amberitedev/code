-- "Visible to friends": the people a linked server's instance is shared with see the server in
-- Worlds and can join it. On unless the owner turns it off.
ALTER TABLE server_sources ADD COLUMN visible_to_friends INTEGER NOT NULL DEFAULT 1;
-- The shared instance behind the linked App instance, as the App reports it. Null while the
-- instance is not shared, which keeps the server to its owner.
ALTER TABLE server_sources ADD COLUMN shared_instance_id TEXT;
