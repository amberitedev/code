-- Server access: who besides the Core's owner may manage each server, and what was done.

-- Accounts this Core has seen, for names in the access list and the activity log.
CREATE TABLE accounts (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL,
    avatar_url TEXT
);

-- An account invited to a server. `role` is `editor` or `viewer`; the Core's owner is owner of
-- every server and has no row. `added_on` is null until the invite is accepted.
CREATE TABLE server_users (
    server_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    role TEXT NOT NULL,
    last_invite_sent TEXT NOT NULL,
    added_on TEXT,
    PRIMARY KEY (server_id, user_id)
);
CREATE INDEX server_users_user ON server_users(user_id);

-- The activity log. `metadata` is the JSON the Access tab reads for that action.
CREATE TABLE action_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    server_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    action TEXT NOT NULL,
    metadata TEXT NOT NULL,
    timestamp TEXT NOT NULL
);
CREATE INDEX action_log_server ON action_log(server_id, timestamp);
