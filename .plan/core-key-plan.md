# Core Key Implementation Plan

## Overview

Two separate authentication mechanisms:

1. **One-time owner claim code** — User pastes it to claim ownership of a Core
2. **Per-core secret key** — Core uses it to prove identity to Supabase (heartbeats)

---

## 1. ONE-TIME OWNER CLAIM CODE

### What
A code shown by Core on first run. User pastes it on amberite.dev to claim ownership.

### Flow
```
1. Core first run (no owner assigned)
2. Core generates random code (e.g., "modstone_ABC123XYZ")
3. Core displays code in console/Panel
4. User goes to amberite.dev, pastes code
5. Supabase verifies code, links user account to Core as owner
6. Code is invalidated (one-time use)
```

### Database Changes (Supabase)

**Table: `owner_claim_codes`**
```sql
CREATE TABLE owner_claim_codes (
    code TEXT PRIMARY KEY,
    core_id UUID NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    used_at TIMESTAMPTZ,
    user_id UUID REFERENCES auth.users
);
```

**Table: `cores` (modify)**
```sql
-- Add owner_claim_code column (temporary, until claimed)
ALTER TABLE cores ADD COLUMN owner_claim_code TEXT UNIQUE;

-- Remove it after claim (or keep for reference, doesn't matter)
```

### Core Changes

**Generate code on first run:**
- Check if `cores.owner_id` is NULL
- If NULL, generate random 16-24 character code
- Store code in `cores.owner_claim_code`
- Display code in console/Panel

**No code storage after claim:**
- Once owner claims, set `owner_claim_code = NULL`
- Core doesn't need to keep it

### Supabase Edge Function

**Function: `claim_core_owner`**
```
Input: { code: string, user_id: uuid }
Steps:
1. Find row in `owner_claim_codes` where code = input.code AND used_at IS NULL
2. If not found → return error "Invalid or used code"
3. Update row: set used_at = NOW(), user_id = input.user_id
4. Update `cores` table: set owner_id = input.user_id, owner_claim_code = NULL
5. Return success
```

### Frontend Changes (amberite.dev)

**Page: `/claim-core`**
```
1. User pastes code
2. Send POST to Supabase Edge Function
3. On success: redirect to dashboard
4. On error: show message (invalid code, already used, etc.)
```

---

## 2. PER-CORE SECRET KEY

### What
Core's secret identity. Used for heartbeats to prove "I am this specific Core."

### Flow
```
1. Core generates random 32-byte secret key
2. Core sends key to Supabase Edge Function once (registration)
3. Supabase hashes key (Argon2/BLAKE3), stores ONLY hash
4. Core keeps original key in local SQLite
5. Every heartbeat: Core sends original key
6. Supabase hashes received key, compares to stored hash
7. If match → Core is legit, update is_online=true
```

### Why Hash
If Supabase DB leaks, attackers get useless hashes. They can't impersonate Core without original key.

### Database Changes (Supabase)

**Table: `cores` (modify)**
```sql
-- Add secret_hash column
ALTER TABLE cores ADD COLUMN secret_hash TEXT NOT NULL DEFAULT '';
```

### Core Changes

**Generate and store key:**
```rust
// On first run, generate key
let secret_key = generate_random_bytes(32); // 32 random bytes
let secret_hash = hash_with_argon2(&secret_key); // or BLAKE3

// Store in local SQLite
sqlx::query("INSERT INTO core_secret (id, key, hash) VALUES (1, ?, ?)")
    .bind(&secret_key)
    .bind(&secret_hash)
    .execute(&pool).await?;
```

**Every heartbeat:**
```rust
// Read original key from local storage
let secret_key = get_local_secret_key();

// Send to Supabase Edge Function
supabase_heartbeat(core_id, secret_key);
```

### Supabase Edge Function

**Function: `core_register`** (called once, on first setup)
```
Input: { core_id: uuid, secret_key: string }
Steps:
1. Hash the secret_key (Argon2/BLAKE3)
2. Update cores table: set secret_hash = hashed_key
3. Return success
```

**Function: `core_heartbeat`** (called periodically)
```
Input: { core_id: uuid, secret_key: string }
Steps:
1. Get stored secret_hash from cores table
2. Hash received secret_key
3. Compare: hashed_key == stored_hash
4. If match:
   - Update cores.is_online = true
   - Update cores.last_seen = NOW()
   - Update cores.player_count, game_version if provided
   - Return success
5. If no match:
   - Return 401 Unauthorized
```

### Core→Supabase Heartbeat Flow

**Every N seconds (e.g., 30s):**
```rust
// Read secret key from local storage
let secret_key = get_local_secret_key();

// POST to Supabase Edge Function
fetch!(
    "https://your-supabase.functions.supabase.co/core_heartbeat",
    {
        method: "POST",
        body: {
            core_id: CORE_ID,
            secret_key: secret_key
        }
    }
);
```

---

## Implementation Order

### Phase 1: One-Time Owner Code
1. Add `owner_claim_code` column to `cores` table in Supabase
2. Create `owner_claim_codes` table (optional, or just use `cores.owner_claim_code`)
3. Create `claim_core_owner` Edge Function
4. Core: generate and display code on first run
5. Frontend (amberite.dev): paste code page

### Phase 2: Core Secret Key
1. Add `secret_hash` column to `cores` table in Supabase
2. Create `core_register` Edge Function (one-time registration)
3. Create `core_heartbeat` Edge Function (periodic authentication)
4. Core: generate secret key on first run, store locally
5. Core: send heartbeat with key every N seconds

---

## Security Notes

### One-Time Owner Code
- Code should be 16-24 characters (enough entropy, easy to type)
- Code should expire after 24 hours if not claimed
- Code should be single-use only
- Core should show code clearly (console output, Panel UI)

### Core Secret Key
- Key should be 32+ bytes of cryptographically random data
- Key should be stored encrypted in Core's local storage (optional, but recommended)
- Supabase should NEVER store the raw key, only the hash
- Heartbeat should use HTTPS (or at least validate certificate in production)

---

## Migration Plan

### SQL Migration (Supabase)
```sql
-- Add owner_claim_code column
ALTER TABLE cores ADD COLUMN owner_claim_code TEXT UNIQUE;

-- Add secret_hash column
ALTER TABLE cores ADD COLUMN secret_hash TEXT NOT NULL DEFAULT '';

-- Optional: Create separate table for claim codes
CREATE TABLE owner_claim_codes (
    code TEXT PRIMARY KEY,
    core_id UUID NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    used_at TIMESTAMPTZ,
    user_id UUID REFERENCES auth.users
);
```

### Core Migration
- No database migration needed
- Core just generates keys on first run if they don't exist

---

## Testing Checklist

### One-Time Owner Code
- [ ] Core generates code on first run
- [ ] Code is displayed in console/Panel
- [ ] Pasting code links user to Core
- [ ] Code becomes invalid after use
- [ ] Code expires after 24 hours (if implemented)
- [ ] Error handling for invalid/used codes

### Core Secret Key
- [ ] Core generates secret key on first run
- [ ] Core stores key locally
- [ ] Heartbeat with correct key succeeds
- [ ] Heartbeat with wrong key fails (401)
- [ ] Core stays marked online while heartbeating
- [ ] Core marked offline after missed heartbeats

---

*Last updated: 2026-04-14*
