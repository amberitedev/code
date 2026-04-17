# Supabase Setup — Modstone

This directory contains the Supabase configuration for Modstone backend.

## Setup Complete ✅

### Migrations Applied (in order)

1. **001_users.sql** - User profiles with friend codes
   - `users` table with auto-generated friend codes
   - `generate_unique_friend_code()` function
   - `handle_new_user()` trigger function (auto-creates profile on signup)
   - `set_updated_at()` trigger function

2. **002_friend_requests.sql** - Friend request system
   - `friend_requests` table (temporary, deleted on accept/decline)
   - Block check trigger to prevent requests to/from blocked users

3. **003_friendships.sql** - Permanent friendships
   - `friendships` table with normalized UUIDs (smaller first)
   - Auto-normalization trigger

4. **004_blocked_users.sql** - User blocking
   - `blocked_users` table
   - Auto-cleanup trigger (removes requests and friendships when blocked)

5. **005_cores.sql** - Core server registry
   - `cores` table for tracking Minecraft servers
   - Connection codes for easy sharing
   - Online status, player count, game version tracking

6. **006_rls_policies.sql** - Row Level Security
   - All tables have RLS enabled
   - Policies for read/write access based on ownership

### Edge Functions

- **accept_friend_request** - Atomically accepts friend requests
  - Deletes the friend_request row
  - Creates the friendship row
  - Called via: `POST /functions/v1/accept_friend_request`

### Environment Variables Needed

Create `.env` files based on these templates:

#### Panel (`.env.example`)
```env
VITE_SUPABASE_URL=https://YOUR_PROJECT_ID.supabase.co
VITE_SUPABASE_ANON_KEY=eyJhbG...
VITE_RELAY_URL=wss://relay.modstone.app
```

#### Core (`.env.example`)
```env
SUPABASE_JWT_SECRET=<from Supabase Dashboard>
CORE_OWNER_ID=<your Supabase user UUID — first run only>
RELAY_URL=wss://relay.modstone.app/core
DATABASE_URL=sqlite://data/core.db
```

#### Relay (`.env.example`)
```env
SUPABASE_URL=https://YOUR_PROJECT_ID.supabase.co
SUPABASE_SERVICE_ROLE_KEY=<service role key — NOT the anon key>
SUPABASE_JWT_SECRET=<same secret as Core>
RELAY_PORT=8080
```

## Next Steps

1. **Enable Realtime** on the `cores` table in Supabase dashboard
   - Go to Database → Replication → Enable Realtime for `cores`

2. **Deploy Relay Server** (separate repository: `modstone-relay`)

3. **Get your credentials** from Supabase Dashboard:
   - Settings → API
   - Copy: Project URL, Anon public key, Service role key, JWT secret

4. **Test the setup**:
   - Create a test user account
   - Verify the user profile was auto-created in `public.users`
   - Check that a friend_code was assigned

## Architecture

```
[Panel]  ←→  [Supabase]   — auth, profiles, friends, core registry, presence
[Panel]  ←→  [Relay]      — all live communication with Core
[Core]   →   [Relay]      — Core connects outward (no port forwarding)
[Core]   →   [Supabase]   — Core pushes status updates
```

## Security Notes

- All tables have RLS enabled
- `handle_new_user()` uses SECURITY DEFINER with fixed search_path
- Edge Function validates JWT before accepting friend requests
- Service role key should only be used in Core and Relay (never in Panel)
