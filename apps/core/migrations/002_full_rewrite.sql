-- Migration 002: Full Amberite Core Rewrite Schema

-- Java version registry
CREATE TABLE IF NOT EXISTS java_installations (
    version   INTEGER PRIMARY KEY,
    path      TEXT    NOT NULL
);

-- Server instances
CREATE TABLE IF NOT EXISTS instances (
    id              TEXT    PRIMARY KEY,
    name            TEXT    NOT NULL,
    game_version    TEXT    NOT NULL,
    loader          TEXT    NOT NULL,
    loader_version  TEXT,
    port            INTEGER NOT NULL,
    memory_min      INTEGER NOT NULL DEFAULT 512,
    memory_max      INTEGER NOT NULL DEFAULT 4096,
    java_version    INTEGER REFERENCES java_installations(version),
    status          TEXT    NOT NULL DEFAULT 'offline',
    data_dir        TEXT    NOT NULL,
    created_at      TEXT    NOT NULL,
    updated_at      TEXT    NOT NULL
);

-- Modpack manifest (one per instance)
CREATE TABLE IF NOT EXISTS modpack_manifests (
    id                    TEXT PRIMARY KEY,
    instance_id           TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    pack_name             TEXT NOT NULL,
    pack_version          TEXT NOT NULL,
    game_version          TEXT NOT NULL,
    loader                TEXT NOT NULL,
    loader_version        TEXT,
    modrinth_project_id   TEXT,
    modrinth_version_id   TEXT,
    installed_at          TEXT NOT NULL,
    UNIQUE(instance_id)
);

-- Core pairing (single row enforced)
CREATE TABLE IF NOT EXISTS core_config (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    supabase_url    TEXT NOT NULL,
    owner_user_id   TEXT NOT NULL,
    paired_at       TEXT NOT NULL
);
