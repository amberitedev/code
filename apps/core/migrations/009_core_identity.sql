CREATE TABLE IF NOT EXISTS core_identity (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    core_id TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL
);

ALTER TABLE core_config ADD COLUMN core_id TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_core_config_core_id
ON core_config (core_id)
WHERE core_id IS NOT NULL;
