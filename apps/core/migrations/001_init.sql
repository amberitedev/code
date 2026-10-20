-- Core's schema. Until `core-rewrite` merges into `main` this file may be edited freely; from the
-- first release on, shipped migrations are never edited again.
-- Times are unix seconds. Ids are UUID strings unless noted.

-- This Core. One row.
CREATE TABLE core (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    -- The audience of every ticket the backend signs for this Core.
    core_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    -- Set at pairing. The owner has every role on every server and is never in `access`.
    owner_user_id TEXT,
    -- Core's own key for its calls to the backend (`amc_...`), issued at pairing.
    backend_key TEXT,
    -- The backend's ticket keys as it last published them (`api::backend::PublicKeys` JSON), and
    -- when they were fetched. Kept so tickets verify while the backend is down.
    ticket_keys TEXT,
    ticket_keys_at INTEGER
);

-- The pairing code this Core is showing, shared by every copal process on this data folder. One row.
CREATE TABLE pairing (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    code TEXT NOT NULL,
    -- The backend key that works once the code is approved.
    key TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    poll_interval INTEGER NOT NULL
);

CREATE TABLE servers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    -- Null until content is installed: the App creates a server first, then installs into it.
    -- Starting a server with either null is refused.
    game_version TEXT,
    loader TEXT CHECK (loader IN ('vanilla', 'fabric', 'quilt', 'forge', 'neoforge', 'paper')),
    loader_version TEXT,
    port INTEGER NOT NULL UNIQUE,
    memory_mb INTEGER NOT NULL,
    -- Advanced override; null means the Java Core manages for this version.
    java_path TEXT,
    -- The managed Java major the owner chose; null means the one the Minecraft version needs.
    java_version INTEGER,
    -- The owner accepted the Minecraft EULA for this server, so Core writes eula=true. Step 8
    -- replaces it with one acceptance per Core.
    eula_accepted INTEGER NOT NULL DEFAULT 0
);

-- Every time a server ran. The live process identity (pid, start time, boot id) is in
-- `runs/<server>/`; this is the history.
CREATE TABLE runs (
    -- The run id the shim records.
    id TEXT PRIMARY KEY,
    server_id TEXT NOT NULL REFERENCES servers (id) ON DELETE CASCADE,
    started_at INTEGER NOT NULL,
    ended_at INTEGER,
    -- Null while running.
    outcome TEXT CHECK (outcome IN ('stopped', 'crashed', 'killed')),
    exit_code INTEGER,
    -- The last console lines of a crashed run.
    last_lines TEXT
);
CREATE INDEX runs_server ON runs (server_id, started_at);

-- Names and avatars as the last ticket for each user carried them, for access lists and the
-- activity log.
CREATE TABLE users (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    avatar_url TEXT,
    updated_at INTEGER NOT NULL
);

-- Who may do what on each server. The only list that decides anything. Invite means granted.
CREATE TABLE access (
    server_id TEXT NOT NULL REFERENCES servers (id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('viewer', 'editor')),
    added_at INTEGER NOT NULL,
    PRIMARY KEY (server_id, user_id)
);
CREATE INDEX access_user ON access (user_id);

-- One per device per Core. Only a hash of the token is stored.
CREATE TABLE sessions (
    -- Public id, for listing and revoking. Not the token.
    id TEXT PRIMARY KEY,
    -- SHA-256 of the token.
    token_hash BLOB NOT NULL UNIQUE,
    user_id TEXT NOT NULL,
    device_name TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    -- Written at most once an hour. A session ends 30 days after this.
    last_used_at INTEGER NOT NULL,
    -- One year after creation, whatever the use.
    expires_at INTEGER NOT NULL
);
CREATE INDEX sessions_user ON sessions (user_id);

-- One-time ids of tickets already used. Kept until the ticket would have expired anyway.
CREATE TABLE used_tickets (
    jti TEXT PRIMARY KEY,
    expires_at INTEGER NOT NULL
);

CREATE TABLE activity (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    at INTEGER NOT NULL,
    -- Who did it. Null for the local CLI, which is the owner.
    user_id TEXT,
    -- Null for actions on the Core itself. No foreign key: a server's history, including its
    -- deletion, outlives the server.
    server_id TEXT,
    action TEXT NOT NULL,
    -- JSON with the details of the action.
    detail TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX activity_server ON activity (server_id, at);

-- Calls Core still owes the backend, retried with backoff until it answers.
CREATE TABLE backend_owed (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- JSON of `api::backend::OwedCall`.
    call TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at INTEGER NOT NULL
);

CREATE TABLE backups (
    id TEXT PRIMARY KEY,
    server_id TEXT NOT NULL REFERENCES servers (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    -- Pre-update backups have their own retention, so they do not evict scheduled ones.
    kind TEXT NOT NULL CHECK (kind IN ('manual', 'scheduled', 'pre_update')),
    state TEXT NOT NULL CHECK (state IN ('queued', 'running', 'done', 'failed')),
    size_bytes INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    error TEXT
);
CREATE INDEX backups_server ON backups (server_id, created_at);

-- Content is the files on disk; this records where each tracked file came from.
CREATE TABLE content_files (
    server_id TEXT NOT NULL REFERENCES servers (id) ON DELETE CASCADE,
    -- Relative to the server folder, with forward slashes: `mods/a.jar`, `config/b.toml`.
    path TEXT NOT NULL,
    origin TEXT NOT NULL CHECK (origin IN ('pack', 'added')),
    -- SHA-1 of the file as Core placed it. A different hash on disk means the owner edited it.
    sha1 TEXT NOT NULL,
    project_id TEXT,
    version_id TEXT,
    -- A pack update found this file edited and left it alone: the new pack `changed` it or
    -- `removed` it. Cleared when the App says keep or replace.
    pending TEXT CHECK (pending IN ('changed', 'removed')),
    PRIMARY KEY (server_id, path)
);

-- The pack a server follows. Unlinking deletes the row and keeps the files.
CREATE TABLE packs (
    server_id TEXT PRIMARY KEY REFERENCES servers (id) ON DELETE CASCADE,
    -- The Modrinth project and version, or null for a pack file the App uploaded.
    project_id TEXT,
    version_id TEXT,
    -- As the pack's index names them.
    name TEXT NOT NULL,
    version_number TEXT NOT NULL,
    summary TEXT,
    -- SHA-1 of the .mrpack, kept in `meta/packs/`.
    sha1 TEXT NOT NULL,
    -- The App instance the pack came from, when the App shares one.
    instance_id TEXT,
    instance_name TEXT,
    -- Why the last install, update, or repair failed. Cleared by the next one that works, or by
    -- unlinking.
    error TEXT
);
