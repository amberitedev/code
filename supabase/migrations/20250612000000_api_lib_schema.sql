-- Migration: api-lib schema — core_registrations, core_messages, core_group_members, minecraft_identities
-- Drops any old tables and recreates them with RLS.

-- 1. minecraft_identities (maps Minecraft UUID to Supabase auth user)
CREATE TABLE IF NOT EXISTS minecraft_identities (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    minecraft_uuid text NOT NULL UNIQUE,
    minecraft_name text NOT NULL,
    supabase_user_id uuid NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    created_at timestamptz DEFAULT now(),
    updated_at timestamptz DEFAULT now()
);

ALTER TABLE minecraft_identities ENABLE ROW LEVEL SECURITY;
CREATE POLICY "minecraft_identities_deny_all" ON minecraft_identities FOR ALL USING (false);

-- 2. core_registrations (one row per Core instance)
CREATE TABLE IF NOT EXISTS core_registrations (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_user_id uuid NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    machine_user_id uuid UNIQUE REFERENCES auth.users(id) ON DELETE SET NULL,
    direct_url text,
    last_seen timestamptz,
    created_at timestamptz DEFAULT now()
);

ALTER TABLE core_registrations ENABLE ROW LEVEL SECURITY;

-- Owners can read their own registrations
CREATE POLICY "owners_read_own_registrations" ON core_registrations
    FOR SELECT USING (owner_user_id = auth.uid());

-- Machine accounts can read/update their own registration
CREATE POLICY "machine_read_own_registration" ON core_registrations
    FOR SELECT USING (machine_user_id = auth.uid());
CREATE POLICY "machine_update_own_registration" ON core_registrations
    FOR UPDATE USING (machine_user_id = auth.uid());

-- Members can read registrations for Cores they belong to
CREATE POLICY "members_read_registrations" ON core_registrations
    FOR SELECT USING (
        EXISTS (
            SELECT 1 FROM core_group_members
            WHERE core_id = core_registrations.id AND user_id = auth.uid()
        )
    );

-- 3. core_messages (relay + push notifications)
CREATE TABLE IF NOT EXISTS core_messages (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    core_id uuid NOT NULL REFERENCES core_registrations(id) ON DELETE CASCADE,
    sender_id uuid NOT NULL,
    direction text NOT NULL CHECK (direction IN ('client-to-core', 'core-to-client')),
    payload jsonb NOT NULL DEFAULT '{}',
    received_at timestamptz,
    completed_at timestamptz,
    result jsonb,
    created_at timestamptz DEFAULT now(),
    ttl timestamptz NOT NULL DEFAULT (now() + interval '5 minutes')
);

CREATE INDEX IF NOT EXISTS idx_core_messages_core_id_created ON core_messages(core_id, created_at);
CREATE INDEX IF NOT EXISTS idx_core_messages_core_id_direction ON core_messages(core_id, direction);

ALTER TABLE core_messages ENABLE ROW LEVEL SECURITY;

-- Users can insert messages only to Cores they own or are members of
CREATE POLICY "users_insert_to_own_cores" ON core_messages
    FOR INSERT WITH CHECK (
        sender_id = auth.uid()
        AND (
            EXISTS (
                SELECT 1 FROM core_registrations
                WHERE id = core_messages.core_id AND owner_user_id = auth.uid()
            )
            OR EXISTS (
                SELECT 1 FROM core_group_members
                WHERE core_id = core_messages.core_id AND user_id = auth.uid()
            )
        )
    );

-- Users can read messages they sent or that are addressed to Cores they belong to
CREATE POLICY "users_read_own_messages" ON core_messages
    FOR SELECT USING (
        sender_id = auth.uid()
        OR EXISTS (
            SELECT 1 FROM core_registrations
            WHERE id = core_messages.core_id AND owner_user_id = auth.uid()
        )
        OR EXISTS (
            SELECT 1 FROM core_group_members
            WHERE core_id = core_messages.core_id AND user_id = auth.uid()
        )
    );

-- Users can update messages they sent (for acks on client-to-core where they are sender)
CREATE POLICY "users_update_own_messages" ON core_messages
    FOR UPDATE USING (sender_id = auth.uid());

-- Machine account can read/write only rows for its own core_id
CREATE POLICY "machine_access_own_core_messages" ON core_messages
    FOR ALL USING (
        EXISTS (
            SELECT 1 FROM core_registrations
            WHERE id = core_messages.core_id AND machine_user_id = auth.uid()
        )
    );

-- 4. core_group_members
CREATE TABLE IF NOT EXISTS core_group_members (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    core_id uuid NOT NULL REFERENCES core_registrations(id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    role text NOT NULL CHECK (role IN ('viewer', 'member', 'mod', 'admin')),
    created_at timestamptz DEFAULT now(),
    UNIQUE (core_id, user_id)
);

ALTER TABLE core_group_members ENABLE ROW LEVEL SECURITY;

-- Owners can manage members for their Cores
CREATE POLICY "owners_manage_members" ON core_group_members
    FOR ALL USING (
        EXISTS (
            SELECT 1 FROM core_registrations
            WHERE id = core_group_members.core_id AND owner_user_id = auth.uid()
        )
    );

-- Users can view their own memberships
CREATE POLICY "users_view_own_memberships" ON core_group_members
    FOR SELECT USING (user_id = auth.uid());

-- Members can view other members of Cores they belong to
CREATE POLICY "members_view_group" ON core_group_members
    FOR SELECT USING (
        EXISTS (
            SELECT 1 FROM core_group_members AS cg
            WHERE cg.core_id = core_group_members.core_id AND cg.user_id = auth.uid()
        )
    );
