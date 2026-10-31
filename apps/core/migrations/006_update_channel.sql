-- The least stable Modrinth version type a server's addons and their dependencies are picked
-- from (`content::modrinth::ReleaseChannel`).
ALTER TABLE servers ADD COLUMN update_channel TEXT NOT NULL DEFAULT 'release'
    CHECK (update_channel IN ('release', 'beta', 'alpha'));
