-- A linked server's source (Amberite instance, Modrinth pack, or uploaded .mrpack) and the
-- durable state of applying it. One row per linked server; no row means unlinked.
CREATE TABLE server_sources (
    instance_id        TEXT PRIMARY KEY REFERENCES instances(id) ON DELETE CASCADE,
    -- CoreServerSource JSON.
    source_json        TEXT NOT NULL,
    -- Instance sources only: sharing backend base URL and the read-only server token.
    sharing_url        TEXT,
    server_token       TEXT,
    desired_version    TEXT NOT NULL,
    installed_version  TEXT,
    -- Version whose files are downloaded and verified in Core's staging directory.
    staged_version     TEXT,
    state              TEXT NOT NULL,
    error              TEXT,
    -- Stop a running server, apply, and start it again.
    restart_requested  INTEGER NOT NULL DEFAULT 0,
    updated_at         TEXT NOT NULL
);

-- Content the installed source version provides, with the hash last applied. Client-only items
-- are recorded for listing but are not on disk. Any other file on the server is server content.
CREATE TABLE server_source_files (
    instance_id  TEXT NOT NULL REFERENCES server_sources(instance_id) ON DELETE CASCADE,
    -- Path relative to the instance data dir, e.g. `mods/a.jar` or `world/datapacks/b.zip`.
    path         TEXT NOT NULL,
    kind         TEXT NOT NULL,
    sha1         TEXT NOT NULL,
    client_only  INTEGER NOT NULL,
    project_id   TEXT,
    version_id   TEXT,
    PRIMARY KEY (instance_id, path)
);
