-- Sign-in waiting for approval in the App. `copal auth` and a running Core share this row, so both
-- show the same code instead of replacing each other's. See `application::auth_service`.
CREATE TABLE IF NOT EXISTS core_auth_pending (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    code TEXT NOT NULL,
    key TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    poll_interval INTEGER NOT NULL
);
