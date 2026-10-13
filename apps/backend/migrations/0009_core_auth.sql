-- Core sign-in. A Core asks for a code and the key it will use; its owner approves the code in the
-- App, and from then on the Core is listed under that account. Replaces pairing, where the App had
-- to reach the Core during setup.

DROP TABLE core_pairing_codes;
DROP TABLE core_pairing_attempts;

-- A sign-in waiting for approval. The Core already holds the key; only its hash is kept, and the
-- key does nothing until approval turns this row into a `cores` row.
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
