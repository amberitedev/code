ALTER TABLE profiles
ADD COLUMN profile_type TEXT NOT NULL DEFAULT 'client'
CHECK (profile_type IN ('client', 'server', 'synced'));
