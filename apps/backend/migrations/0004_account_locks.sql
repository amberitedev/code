-- Port of Labrinth account locking. Lock details are visible only to staff.
CREATE TABLE user_locks (
  user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  locked_by TEXT NOT NULL REFERENCES users(id),
  reason TEXT NOT NULL,
  created TEXT NOT NULL
);
