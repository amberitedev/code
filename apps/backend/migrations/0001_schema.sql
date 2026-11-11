-- Account/social behavior ported from apps/labrinth under AGPL-3.0-only.
-- Statements are split on semicolons by tests/worker.mjs, so none may appear in a comment or string.
CREATE TABLE users (
  id TEXT PRIMARY KEY, username TEXT NOT NULL COLLATE NOCASE UNIQUE,
  email TEXT NOT NULL COLLATE NOCASE UNIQUE, password_hash TEXT,
  email_verified INTEGER NOT NULL DEFAULT 0, totp_secret TEXT,
  avatar_url TEXT, bio TEXT, created TEXT NOT NULL,
  allow_friend_requests INTEGER NOT NULL DEFAULT 1, is_dev INTEGER NOT NULL DEFAULT 0,
  role TEXT NOT NULL DEFAULT 'developer', preferences TEXT NOT NULL DEFAULT '{}'
);
CREATE TABLE sessions (
  id TEXT PRIMARY KEY, token_hash TEXT NOT NULL UNIQUE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  created TEXT NOT NULL, last_login TEXT NOT NULL, expires TEXT NOT NULL,
  refresh_expires TEXT NOT NULL, user_agent TEXT NOT NULL, ip TEXT NOT NULL
);
CREATE INDEX sessions_user ON sessions(user_id);
CREATE TABLE auth_flows (
  id TEXT PRIMARY KEY, kind TEXT NOT NULL,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  payload TEXT, expires TEXT NOT NULL
);
CREATE TABLE backup_codes (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  code_hash TEXT NOT NULL, PRIMARY KEY(user_id, code_hash)
);
CREATE TABLE used_totp (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  counter INTEGER NOT NULL, PRIMARY KEY(user_id, counter)
);
CREATE TABLE friends (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  friend_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  accepted INTEGER NOT NULL DEFAULT 0, created TEXT NOT NULL,
  PRIMARY KEY(user_id, friend_id), CHECK(user_id != friend_id)
);
CREATE UNIQUE INDEX friends_pair ON friends(min(user_id,friend_id), max(user_id,friend_id));
CREATE INDEX friends_recipient ON friends(friend_id);
CREATE TABLE blocks (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  blocked_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  PRIMARY KEY(user_id,blocked_id), CHECK(user_id != blocked_id)
);
CREATE TABLE notifications (
  id TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  body TEXT NOT NULL, created TEXT NOT NULL, read INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX notifications_user ON notifications(user_id,created);
CREATE TABLE email_outbox (
  id TEXT PRIMARY KEY, recipient TEXT NOT NULL, kind TEXT NOT NULL,
  flow TEXT NOT NULL, created TEXT NOT NULL
);
CREATE TABLE auth_attempts (key TEXT PRIMARY KEY, count INTEGER NOT NULL, expires INTEGER NOT NULL);
CREATE TABLE avatars (
  user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  content_type TEXT NOT NULL, data TEXT NOT NULL, revision TEXT NOT NULL
);
-- Lock details are visible only to staff.
CREATE TABLE user_locks (
  user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  locked_by TEXT NOT NULL REFERENCES users(id),
  reason TEXT NOT NULL,
  created TEXT NOT NULL
);

-- A paired Core. `key_hash` is the SHA-256 of the key the Core uses for its own calls.
-- `cert_fingerprint` is the SHA-256 of the certificate it serves its API with, the only one the
-- App trusts for it. `address` is on its own network, `public_address` on the internet once the
-- Core has proven it can be reached there. `version` is reported each time it starts.
CREATE TABLE cores (
  id TEXT PRIMARY KEY,
  owner_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  address TEXT NOT NULL,
  key_hash TEXT NOT NULL UNIQUE,
  created TEXT NOT NULL,
  last_seen TEXT NOT NULL,
  cert_fingerprint TEXT,
  public_address TEXT,
  version TEXT
);
CREATE INDEX cores_owner ON cores(owner_id);

-- A Core sign-in waiting for its owner's approval. Only the key's hash is kept, and the key does
-- nothing until approval turns this row into a `cores` row.
CREATE TABLE core_auth_codes (
  code TEXT PRIMARY KEY,
  core_id TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  address TEXT NOT NULL,
  cert_fingerprint TEXT,
  key_hash TEXT NOT NULL UNIQUE,
  expires TEXT NOT NULL
);

-- Counted events since `window_start`: codes an account tried (`approve:<user>`), code requests
-- per Core (`code:<core>`) and per caller address (`ip:<address>`). One UPSERT counts and reads.
CREATE TABLE core_auth_limits (
  scope TEXT PRIMARY KEY,
  count INTEGER NOT NULL,
  window_start TEXT NOT NULL
);

-- Accounts that may reach a Core they do not own, so the App can be told where it is. The Core
-- adds an account when it invites them and removes it when they have no server left there.
CREATE TABLE core_access (
  core_id TEXT NOT NULL REFERENCES cores(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  created TEXT NOT NULL,
  PRIMARY KEY (core_id, user_id)
);
CREATE INDEX core_access_user ON core_access(user_id);

-- Each Core's storage as it last reported it. `service` is whether it runs as a system service.
CREATE TABLE core_storage (
  core_id TEXT PRIMARY KEY REFERENCES cores(id) ON DELETE CASCADE,
  enabled INTEGER NOT NULL,
  service INTEGER NOT NULL,
  cap INTEGER NOT NULL,
  used INTEGER NOT NULL,
  free INTEGER NOT NULL,
  egress_cap INTEGER NOT NULL,
  egress_today INTEGER NOT NULL,
  reported TEXT NOT NULL
);

-- What each Core's store reported it holds, one row per version hash.
CREATE TABLE core_holdings (
  core_id TEXT NOT NULL REFERENCES cores(id) ON DELETE CASCADE,
  version_hash TEXT NOT NULL,
  bytes INTEGER NOT NULL,
  confirmed TEXT NOT NULL,
  PRIMARY KEY(core_id, version_hash)
);
CREATE INDEX core_holdings_version_hash ON core_holdings(version_hash);

-- Sharing metadata. Each version's manifest is in R2 under `manifests/<instance>/<version>`, and
-- the files themselves live on Cores.
-- `icon` is the URL of an uploaded image, kept in R2 under the share. `icon_config` is an icon
-- editor icon's settings, which clients draw themselves. `home` and `replica` are where the
-- share's files live, picked at its first push with files. `installed_once` is set the first time
-- someone other than the owner joins and never cleared. `core_id` and `server_id` are set only on
-- a server-first share, written by that Core with its key and owned by the Core's owner.
CREATE TABLE shared_instances (
  id TEXT PRIMARY KEY,
  owner_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  icon TEXT,
  icon_config TEXT,
  quarantine INTEGER NOT NULL DEFAULT 0,
  installed_once INTEGER NOT NULL DEFAULT 0,
  home TEXT REFERENCES cores(id) ON DELETE SET NULL,
  replica TEXT REFERENCES cores(id) ON DELETE SET NULL,
  core_id TEXT,
  server_id TEXT,
  created TEXT NOT NULL
);
CREATE UNIQUE INDEX shared_instances_server ON shared_instances(core_id, server_id)
  WHERE core_id IS NOT NULL;
CREATE TABLE shared_instance_users (
  instance_id TEXT NOT NULL REFERENCES shared_instances(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  joined_at TEXT,
  join_type TEXT NOT NULL CHECK(join_type IN ('owner','invite','link')),
  last_played TEXT,
  notified INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY(instance_id,user_id)
);
CREATE INDEX shared_instance_users_user ON shared_instance_users(user_id);

-- `version_hash` is the hash of the version's sorted object hashes, which tickets and holdings
-- name. `bytes` is the sum of its distinct object sizes, and `added` the bytes of objects the
-- previous kept version did not have (NULL counts the whole version). A version is ready once its
-- home Core has confirmed it holds `version_hash`, and a version with no files is ready at once.
CREATE TABLE shared_instance_versions (
  instance_id TEXT NOT NULL REFERENCES shared_instances(id) ON DELETE CASCADE,
  version INTEGER NOT NULL,
  version_hash TEXT NOT NULL,
  bytes INTEGER NOT NULL,
  added INTEGER,
  ready INTEGER NOT NULL DEFAULT 0,
  created TEXT NOT NULL,
  PRIMARY KEY(instance_id, version)
);
CREATE INDEX shared_instance_versions_version_hash ON shared_instance_versions(version_hash);

-- A link with `core_id` and `server_id` makes whoever opens it a player of that server instead of
-- a member of the share.
CREATE TABLE shared_instance_invites (
  id TEXT PRIMARY KEY,
  instance_id TEXT NOT NULL REFERENCES shared_instances(id) ON DELETE CASCADE,
  expiration TEXT NOT NULL,
  max_uses INTEGER NOT NULL,
  uses INTEGER NOT NULL DEFAULT 0,
  core_id TEXT,
  server_id TEXT
);

-- A member's report that a version's bytes were wrong: `object` when a Core served bytes that did
-- not hash to their name, `content` when they decrypted to the wrong file. Shown to the owner.
CREATE TABLE shared_instance_reports (
  instance_id TEXT NOT NULL,
  version INTEGER NOT NULL,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  core_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK(kind IN ('object','content')),
  created TEXT NOT NULL,
  PRIMARY KEY(instance_id, version, user_id, kind),
  FOREIGN KEY(instance_id, version) REFERENCES shared_instance_versions(instance_id, version) ON DELETE CASCADE
);
CREATE TABLE sharing_blacklist (
  user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE
);

-- How many ready versions an account's shares keep, files included. No row keeps five, and a NULL
-- `kept_versions` keeps them all.
CREATE TABLE sharing_settings (
  user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  kept_versions INTEGER
);

-- Servers a Core shows to the people an instance is shared with, written with that Core's key.
-- Online state and player counts are not stored: the App pings `address`, the server's Minecraft
-- address, itself.
CREATE TABLE visible_servers (
  core_id TEXT NOT NULL REFERENCES cores(id) ON DELETE CASCADE,
  server_id TEXT NOT NULL,
  name TEXT NOT NULL,
  address TEXT NOT NULL,
  instance_id TEXT NOT NULL REFERENCES shared_instances(id) ON DELETE CASCADE,
  PRIMARY KEY(core_id, server_id)
);
CREATE INDEX visible_servers_instance ON visible_servers(instance_id);

-- A Core server that has players, and the share it follows: its instance's share or its own
-- server-first share. Players join `port` at the Core's last reported address, or first at
-- `outside`, the `ip:port` the Core's router granted it on the internet.
CREATE TABLE hosted_servers (
  core_id TEXT NOT NULL REFERENCES cores(id) ON DELETE CASCADE,
  server_id TEXT NOT NULL,
  instance_id TEXT NOT NULL REFERENCES shared_instances(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  port INTEGER NOT NULL,
  outside TEXT,
  PRIMARY KEY (core_id, server_id)
);
CREATE INDEX hosted_servers_instance ON hosted_servers(instance_id);

-- The people a server is shared with. They read the share it follows without being its members,
-- and are pending until they accept the invite, like a member.
CREATE TABLE hosted_server_players (
  core_id TEXT NOT NULL,
  server_id TEXT NOT NULL,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  joined_at TEXT,
  join_type TEXT NOT NULL CHECK(join_type IN ('invite','link')),
  notified INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (core_id, server_id, user_id),
  FOREIGN KEY (core_id, server_id) REFERENCES hosted_servers(core_id, server_id) ON DELETE CASCADE
);
CREATE INDEX hosted_server_players_user ON hosted_server_players(user_id);
