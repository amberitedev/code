-- The share version a pack from an App instance is (`content::shared`).
ALTER TABLE packs ADD COLUMN shared_instance_id TEXT;
ALTER TABLE packs ADD COLUMN shared_version INTEGER;

-- A pack pushed while the server ran, applied when it next stops. Only the latest push waits.
CREATE TABLE pending_packs (
    server_id TEXT PRIMARY KEY REFERENCES servers (id) ON DELETE CASCADE,
    -- SHA-1 of the .mrpack, kept in `meta/packs/`.
    sha1 TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    instance_name TEXT NOT NULL,
    shared_instance_id TEXT NOT NULL,
    shared_version INTEGER NOT NULL,
    -- Start the server again once the pack is in: the push asked for a restart.
    start_after INTEGER NOT NULL
);
