-- Core's schema. Nothing is released, so this one file is edited freely and a local database is
-- reset when it changes; from the first release on, shipped migrations are never edited again.
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
    ticket_keys_at INTEGER,
    -- The release channel the updater follows: 'stable', 'beta', or 'alpha'.
    update_channel TEXT NOT NULL DEFAULT 'stable' CHECK (update_channel IN ('stable', 'beta', 'alpha')),
    -- Install updates as they come out. Off: only when the owner asks.
    auto_update INTEGER NOT NULL DEFAULT 1,
    -- When the owner accepted the Minecraft EULA, once for every server. Null: not yet, and no
    -- server starts.
    eula_accepted_at INTEGER,
    -- Storage this Core offers the shared file pool (`store`). On by default.
    storage_enabled INTEGER NOT NULL DEFAULT 1,
    -- Bytes the store may hold. Null: a tenth of the disk's free space at start, between 10 and
    -- 20 GiB.
    storage_cap INTEGER,
    -- Bytes the store may serve a day. 50 GiB.
    storage_egress_cap INTEGER NOT NULL DEFAULT 53687091200
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
    -- The least stable Modrinth version type the server's addons and their dependencies are
    -- picked from (a `VersionType`).
    update_channel TEXT NOT NULL DEFAULT 'release'
        CHECK (update_channel IN ('release', 'beta', 'alpha')),
    -- The share of a server that follows no instance, which Core writes (`api::share`). Null
    -- until the server is shared.
    share_id TEXT
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
    created_at INTEGER NOT NULL
);
CREATE INDEX backups_server ON backups (server_id, created_at);

-- Every create and restore of a backup, queued and run one at a time.
CREATE TABLE backup_operations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    backup_id TEXT NOT NULL REFERENCES backups (id) ON DELETE CASCADE,
    server_id TEXT NOT NULL REFERENCES servers (id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('create', 'restore')),
    state TEXT NOT NULL CHECK (state IN ('queued', 'running', 'done', 'failed', 'cancelled')),
    -- Who asked. Null for Core itself (schedules, updates) and the local CLI.
    user_id TEXT,
    queued_at INTEGER NOT NULL,
    started_at INTEGER,
    ended_at INTEGER,
    error TEXT,
    -- The App showed the result and was told to stop.
    acked INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX backup_operations_state ON backup_operations (state, id);

-- A server's backup schedule, in the App's slot model. Without a row: daily, keeping the last 7.
CREATE TABLE backup_schedules (
    server_id TEXT PRIMARY KEY REFERENCES servers (id) ON DELETE CASCADE,
    -- Always an interval, even with scheduled backups off.
    interval_hours INTEGER NOT NULL,
    -- The automatic slots; 0 turns scheduled backups off.
    keep INTEGER NOT NULL,
    -- When the schedule last came due with nothing new to back up, so the next due time Core
    -- reports is one it keeps.
    skipped_at INTEGER
);

-- Content is the files on disk. These four tables record where each tracked file came from, in
-- app-lib's shape (`instance_links`, `instance_content_sets`, `instance_files`,
-- `instance_content_entries`) with `server` for `instance`. Copal's differences are marked
-- "Copal:". The rows are read and written by `state::servers::adapters::sqlite::content_rows`.

-- Where the pack a server follows came from (`state::servers::ServerLink`). A server that follows
-- no pack has no row. Written and deleted with its content set.
CREATE TABLE server_links (
    server_id TEXT PRIMARY KEY REFERENCES servers (id) ON DELETE CASCADE,
    -- 'modrinth_modpack', 'imported_modpack' (a file the App uploaded), or 'shared_instance' (a
    -- version of an App instance's share).
    link_kind TEXT NOT NULL
        CHECK (link_kind IN ('modrinth_modpack', 'imported_modpack', 'shared_instance')),
    -- The Modrinth project and version of a 'modrinth_modpack'.
    modrinth_project_id TEXT,
    modrinth_version_id TEXT,
    -- The share of a 'shared_instance'.
    shared_instance_id TEXT,
    -- Copal: the version of the share the pack is, and the App instance it came from. app-lib
    -- keeps an instance's applied share version with its shared-instance attachment.
    shared_instance_version INTEGER,
    instance_id TEXT,
    instance_name TEXT
);

-- The pack a server follows, as a content set. A server has at most one; unlinking deletes it
-- and keeps the files, as files someone added.
CREATE TABLE server_content_sets (
    id TEXT PRIMARY KEY,
    -- Copal: one set per server.
    server_id TEXT NOT NULL UNIQUE REFERENCES servers (id) ON DELETE CASCADE,
    -- As the pack's index names it.
    name TEXT NOT NULL,
    -- 'modrinth_modpack', 'imported_modpack' (a file the App uploaded), or 'shared_instance' (a
    -- version of an App instance's share).
    source_kind TEXT NOT NULL,
    status TEXT NOT NULL,
    game_version TEXT NOT NULL,
    protocol_version INTEGER,
    loader TEXT NOT NULL,
    loader_version TEXT,
    created INTEGER NOT NULL,
    modified INTEGER NOT NULL,
    -- Copal: every column from here on, about the pack file itself. As the pack's index names
    -- them.
    version_number TEXT NOT NULL,
    summary TEXT,
    -- SHA-1 of the .mrpack, kept in `meta/packs/`.
    sha1 TEXT NOT NULL,
    -- Why the last install, update, or repair failed. Cleared by the next one that works, or by
    -- unlinking.
    error TEXT
);

-- Every file Core tracks on a server, by the path it has when enabled.
CREATE TABLE server_files (
    id TEXT PRIMARY KEY,
    server_id TEXT NOT NULL REFERENCES servers (id) ON DELETE CASCADE,
    -- Relative to the server folder, with forward slashes: `mods/a.jar`, `config/b.toml`. A
    -- disabled addon is on disk with `.disabled` added.
    relative_path TEXT NOT NULL,
    file_name TEXT NOT NULL,
    -- As Core last left it; the disk decides.
    enabled INTEGER NOT NULL,
    -- SHA-1 of the file as Core placed it. A different hash on disk means the owner edited it.
    sha1 TEXT NOT NULL,
    -- As Core placed it; 0 for a file it found there rather than placed.
    size INTEGER NOT NULL,
    -- Copal: a file of the pack the server leaves out, client only, which is never on its disk.
    missing INTEGER NOT NULL DEFAULT 0,
    added_at INTEGER NOT NULL,
    modified_at INTEGER NOT NULL,
    UNIQUE (server_id, relative_path)
);

-- Where each tracked file came from, and which sides it is on for.
CREATE TABLE server_content_entries (
    id TEXT PRIMARY KEY,
    server_id TEXT NOT NULL REFERENCES servers (id) ON DELETE CASCADE,
    -- Copal: the pack's set for a file the pack placed, null for one someone added.
    content_set_id TEXT REFERENCES server_content_sets (id) ON DELETE SET NULL,
    -- Copal: every entry is one file's.
    file_id TEXT NOT NULL UNIQUE REFERENCES server_files (id) ON DELETE CASCADE,
    -- A `ContentType` (`mod`, `plugin`, `datapack`, ...). Copal: null for a pack's other files,
    -- such as configs.
    project_type TEXT,
    project_id TEXT,
    version_id TEXT,
    -- 'local' for a file someone on the server added, or the source kind of the pack's set.
    source_kind TEXT NOT NULL,
    -- 'unsupported' server side: the file is renamed `.disabled`, and players still get it.
    server_requirement TEXT NOT NULL DEFAULT 'required',
    -- 'unsupported' client side: the server runs it, and its share leaves it out.
    client_requirement TEXT NOT NULL DEFAULT 'required',
    -- Off for both sides.
    enabled INTEGER NOT NULL DEFAULT 1,
    added_at INTEGER NOT NULL,
    modified_at INTEGER NOT NULL,
    -- Copal: someone set the sides by hand.
    sides_unlocked INTEGER NOT NULL DEFAULT 0,
    -- Copal: a client-only file of the pack that someone turned on for the server, so Core placed
    -- it. Pack updates keep it on the server; turning the server side off makes it client only
    -- again.
    server_override INTEGER NOT NULL DEFAULT 0,
    -- Copal: a pack update found this file edited and left it alone: the new pack `changed` it
    -- or `removed` it. Cleared when the App says keep or replace.
    pending TEXT CHECK (pending IN ('changed', 'removed'))
);
CREATE INDEX server_content_entries_server ON server_content_entries (server_id);

-- A pack pushed while the server ran, applied when it next stops. Only the latest push waits.
-- Not a content set: nothing of it is on the server until it applies.
CREATE TABLE pending_packs (
    server_id TEXT PRIMARY KEY REFERENCES servers (id) ON DELETE CASCADE,
    -- SHA-1 of the .mrpack, kept in `meta/packs/`.
    sha1 TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    instance_name TEXT NOT NULL,
    shared_instance_id TEXT NOT NULL,
    shared_version INTEGER NOT NULL,
    -- Start the server again once the pack is in: the push asked for a restart.
    start_after INTEGER NOT NULL,
    -- Install it even for an older Minecraft than the world's, as the push asked.
    force INTEGER NOT NULL
);

-- When a server restarts by itself. A server without a row has the defaults: crash restart on,
-- nothing scheduled, no smart restarts.
CREATE TABLE restarts (
    server_id TEXT PRIMARY KEY REFERENCES servers (id) ON DELETE CASCADE,
    -- Start the server again after a crash.
    crash INTEGER NOT NULL DEFAULT 1,
    -- JSON of `servers::restarts::Schedule`, null for none.
    schedule TEXT,
    -- Restart once nobody is online and the server has run this many hours. Null: off.
    smart_after_hours INTEGER
);

-- A server's own JVM flags, instead of the tuned ones. No row: the tuned flags.
CREATE TABLE jvm_flags (
    server_id TEXT PRIMARY KEY REFERENCES servers (id) ON DELETE CASCADE,
    -- JSON array of strings.
    flags TEXT NOT NULL
);

-- Port mappings Core asked the router for, so a Core that restarts renews them instead of asking
-- again (a PCP router knows a mapping by its nonce). Keyed by server id, or 'core' for the API.
CREATE TABLE port_mappings (
    key TEXT PRIMARY KEY,
    -- JSON of `network::router::Mapping`.
    mapping TEXT NOT NULL
);
