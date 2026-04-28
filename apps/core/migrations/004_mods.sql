CREATE TABLE IF NOT EXISTS mods (
    id                   TEXT PRIMARY KEY,
    instance_id          TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    filename             TEXT NOT NULL,
    display_name         TEXT,
    modrinth_project_id  TEXT,
    modrinth_version_id  TEXT,
    version_number       TEXT,
    client_side          TEXT,
    server_side          TEXT,
    sha512               TEXT,
    enabled              INTEGER NOT NULL DEFAULT 1,
    installed_at         TEXT NOT NULL,
    UNIQUE(instance_id, filename)
);
