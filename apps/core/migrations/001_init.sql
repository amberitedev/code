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

-- A server's content, loader and Minecraft version are its engine instance's, which has the
-- server's id.
CREATE TABLE servers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    port INTEGER NOT NULL UNIQUE,
    memory_mb INTEGER NOT NULL,
    -- Advanced override; null means the Java Core manages for this version.
    java_path TEXT,
    -- The managed Java major the owner chose; null means the one the Minecraft version needs.
    java_version INTEGER,
    -- The icon editor settings (JSON `IconConfig`) the server's icon was made from, and the
    -- SHA-256 of the game icon they made. A game icon with another hash was replaced some other
    -- way, so the settings no longer describe it.
    icon_config TEXT,
    icon_hash TEXT
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
