-- The backup engine (step 5). Kept out of 001, which installs had already applied.

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

-- A server's backup schedule. Without a row: daily, keeping the last 7.
CREATE TABLE backup_schedules (
    server_id TEXT PRIMARY KEY REFERENCES servers (id) ON DELETE CASCADE,
    -- Zero turns scheduled backups off.
    interval_hours INTEGER NOT NULL,
    keep INTEGER NOT NULL
);
