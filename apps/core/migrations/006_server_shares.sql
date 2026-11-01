-- The share of a server that follows no instance, which Core writes (`api::share`). Null until
-- the server is shared.
ALTER TABLE servers ADD COLUMN share_id TEXT;

-- Side toggles of a server's addons, by the enabled path (`content::records::SideToggle`). No row
-- is on for both sides.
CREATE TABLE side_toggles (
    server_id TEXT NOT NULL REFERENCES servers (id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    -- Not run on the server: the file is renamed `.disabled`, and players still get it.
    disabled_server INTEGER NOT NULL DEFAULT 0,
    -- Run on the server, left out of the share.
    disabled_player INTEGER NOT NULL DEFAULT 0,
    -- Someone set the sides by hand.
    unlocked INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (server_id, path)
);
