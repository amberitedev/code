CREATE TABLE shared_instances (
  id TEXT PRIMARY KEY,
  owner_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  icon TEXT,
  quarantine INTEGER NOT NULL DEFAULT 0,
  icon_data TEXT,
  icon_type TEXT,
  created TEXT NOT NULL
);
CREATE TABLE shared_members (
  instance_id TEXT NOT NULL REFERENCES shared_instances(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  joined_at TEXT,
  join_type TEXT NOT NULL CHECK(join_type IN ('owner','invite','link')),
  last_played TEXT,
  notified INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY(instance_id,user_id)
);
CREATE INDEX shared_members_user ON shared_members(user_id);
CREATE TABLE shared_versions (
  instance_id TEXT NOT NULL REFERENCES shared_instances(id) ON DELETE CASCADE,
  version INTEGER NOT NULL,
  manifest TEXT NOT NULL,
  ready INTEGER NOT NULL DEFAULT 0,
  pinned INTEGER NOT NULL DEFAULT 0,
  created TEXT NOT NULL,
  PRIMARY KEY(instance_id,version)
);
CREATE TABLE shared_blobs (
  id TEXT PRIMARY KEY,
  instance_id TEXT NOT NULL REFERENCES shared_instances(id) ON DELETE CASCADE,
  sha256 TEXT NOT NULL,
  size INTEGER NOT NULL,
  created TEXT NOT NULL,
  UNIQUE(instance_id,sha256)
);
CREATE TABLE shared_files (
  id TEXT PRIMARY KEY,
  instance_id TEXT NOT NULL,
  version INTEGER NOT NULL,
  file_name TEXT NOT NULL,
  file_type TEXT NOT NULL,
  blob_id TEXT REFERENCES shared_blobs(id),
  FOREIGN KEY(instance_id,version) REFERENCES shared_versions(instance_id,version) ON DELETE CASCADE,
  UNIQUE(instance_id,version,file_type,file_name)
);
CREATE TABLE shared_links (
  id TEXT PRIMARY KEY,
  instance_id TEXT NOT NULL REFERENCES shared_instances(id) ON DELETE CASCADE,
  expiration TEXT NOT NULL,
  max_uses INTEGER NOT NULL,
  uses INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE shared_downloads (
  token TEXT PRIMARY KEY,
  file_id TEXT NOT NULL REFERENCES shared_files(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  expires INTEGER NOT NULL
);
CREATE INDEX shared_downloads_expiry ON shared_downloads(expires);
CREATE TABLE storage_nodes (
  id TEXT PRIMARY KEY,
  last_seen INTEGER NOT NULL
);
CREATE TABLE shared_replicas (
  blob_id TEXT NOT NULL REFERENCES shared_blobs(id) ON DELETE CASCADE,
  node_id TEXT NOT NULL,
  verified INTEGER NOT NULL,
  PRIMARY KEY(blob_id,node_id)
);
CREATE TABLE sharing_blacklist (
  user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE
);
