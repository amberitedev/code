-- Shared server installations: a deduplicated install of a given
-- (game_version, loader, loader_version). Multiple instances reference the same
-- installation via instances.installation_id to avoid downloading the server
-- JAR + loader libraries more than once.
CREATE TABLE IF NOT EXISTS server_installations (
    id             TEXT PRIMARY KEY,
    game_version   TEXT NOT NULL,
    loader         TEXT NOT NULL,
    loader_version TEXT,
    status         TEXT NOT NULL DEFAULT 'installing',
    error          TEXT,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);

-- Nullable back-reference. NULL means a legacy instance whose files live in its
-- own data_dir (pre-shared-installation behaviour). No FK constraint: SQLite FK
-- enforcement is off by default and ADD COLUMN cannot add a FK with validation.
ALTER TABLE instances ADD COLUMN installation_id TEXT;
