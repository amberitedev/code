-- Account/social behavior ported from apps/labrinth under AGPL-3.0-only.
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
