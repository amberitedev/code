CREATE TABLE IF NOT EXISTS backups (
    id          TEXT    PRIMARY KEY,
    instance_id TEXT    NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    name        TEXT    NOT NULL,
    size_bytes  INTEGER NOT NULL DEFAULT 0,
    locked      INTEGER NOT NULL DEFAULT 0,
    trigger     TEXT    NOT NULL DEFAULT 'manual',
    created_at  TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS backup_schedules (
    instance_id  TEXT    PRIMARY KEY REFERENCES instances(id) ON DELETE CASCADE,
    enabled      INTEGER NOT NULL DEFAULT 0,
    cron         TEXT    NOT NULL DEFAULT '0 4 * * *',
    retain_count INTEGER NOT NULL DEFAULT 5
);
