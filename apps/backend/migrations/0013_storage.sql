-- Storage (docs/internals/pool.md, "Where a share lives" and "Kept versions and usage").

-- The bytes of a version's objects that the version kept before it did not have, so a share's
-- usage is its oldest kept version plus what each later one added. NULL counts the whole version.
ALTER TABLE shared_versions ADD COLUMN added INTEGER;

-- How many ready versions an account's shares keep, files included. No row keeps five; a NULL
-- `kept_versions` keeps them all.
CREATE TABLE sharing_settings (
  user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  kept_versions INTEGER
);
