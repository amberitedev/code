-- Shared files live in a pool of Cores (docs/internals/pool.md). The backend keeps each version's
-- manifest in R2 under `manifests/<instance>/<version>` and only these rows in D1. The per-file
-- rows and R2 file objects of the earlier design go; local databases start their versions over.
DROP TABLE IF EXISTS shared_files;
DROP TABLE IF EXISTS shared_versions;

-- Where a share's files live: picked at its first push with files and kept for every version.
ALTER TABLE shared_instances ADD COLUMN home TEXT REFERENCES cores(id) ON DELETE SET NULL;
ALTER TABLE shared_instances ADD COLUMN replica TEXT REFERENCES cores(id) ON DELETE SET NULL;

-- `root` is R, the hash of the version's sorted object hashes, which tickets and holdings name.
-- `bytes` is the sum of the version's distinct object sizes. A version is ready once its home
-- Core has confirmed it holds `root`; a version with no files is ready at once.
CREATE TABLE shared_versions (
  instance_id TEXT NOT NULL REFERENCES shared_instances(id) ON DELETE CASCADE,
  version INTEGER NOT NULL,
  root TEXT NOT NULL,
  bytes INTEGER NOT NULL,
  ready INTEGER NOT NULL DEFAULT 0,
  created TEXT NOT NULL,
  PRIMARY KEY(instance_id, version)
);
CREATE INDEX shared_versions_root ON shared_versions(root);

-- What each Core's store reported it holds, one row per version root.
CREATE TABLE core_holdings (
  core_id TEXT NOT NULL REFERENCES cores(id) ON DELETE CASCADE,
  root TEXT NOT NULL,
  bytes INTEGER NOT NULL,
  confirmed TEXT NOT NULL,
  PRIMARY KEY(core_id, root)
);
CREATE INDEX core_holdings_root ON core_holdings(root);

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

-- A member's report that a version's bytes were wrong: `object` when a Core served bytes that did
-- not hash to their name, `content` when they decrypted to the wrong file. Shown to the owner.
CREATE TABLE shared_reports (
  instance_id TEXT NOT NULL,
  version INTEGER NOT NULL,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  core_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK(kind IN ('object','content')),
  created TEXT NOT NULL,
  PRIMARY KEY(instance_id, version, user_id, kind),
  FOREIGN KEY(instance_id, version) REFERENCES shared_versions(instance_id, version) ON DELETE CASCADE
);
