ALTER TABLE core_config ADD COLUMN convex_url TEXT;
ALTER TABLE core_config ADD COLUMN auth_jwks_url TEXT;
ALTER TABLE core_config ADD COLUMN auth_audience TEXT NOT NULL DEFAULT 'authenticated';

UPDATE core_config
SET convex_url = supabase_url,
    auth_jwks_url = supabase_url || '/auth/v1/.well-known/jwks.json'
WHERE convex_url IS NULL AND supabase_url IS NOT NULL;

CREATE TABLE IF NOT EXISTS core_relay_messages (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL,
    version INTEGER NOT NULL,
    sender_id TEXT NOT NULL,
    recipient_id TEXT NOT NULL,
    payload TEXT NOT NULL,
    ack TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    received_at TEXT,
    processed_at TEXT,
    result TEXT,
    error TEXT
);

CREATE INDEX IF NOT EXISTS idx_core_relay_recipient_status
ON core_relay_messages (recipient_id, status);
