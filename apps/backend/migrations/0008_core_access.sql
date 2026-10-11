-- Server access. A Core's owner invites other accounts to manage its servers.

-- Accounts that may reach a Core they do not own, so the App can be told where it is. The Core
-- adds an account when it invites them and removes it when they have no server left there.
-- Roles and which servers stay on the Core.
CREATE TABLE core_access (
  core_id TEXT NOT NULL REFERENCES cores(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  created TEXT NOT NULL,
  PRIMARY KEY (core_id, user_id)
);
CREATE INDEX core_access_user ON core_access(user_id);

-- The Core's own version, reported each time it starts.
ALTER TABLE cores ADD COLUMN version TEXT;
