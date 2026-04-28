# infrastructure/auth/

JWT validation for Supabase RS256 tokens.

## Files

| File | Key types |
|------|-----------|
| `jwks.rs` | `JwksCache`, `Claims`, `AuthError` |

## How it works

1. `JwksCache` fetches the JWKS document from `AppState.config.jwks_url` (Supabase's `/.well-known/jwks.json`).
2. Keys are cached in memory and refreshed on 401 from downstream.
3. `Claims` is extracted from the validated JWT payload.
4. `AuthUser` extractor (in `presentation/extractors.rs`) calls `JwksCache::validate()` and injects `Claims` into handlers.

## `Claims`

```rust
pub struct Claims {
    pub sub:  String,           // Supabase user UUID
    pub role: Option<String>,   // "authenticated" etc.
    pub exp:  u64,
}
```

## `AuthError`

```
Fetch(reqwest::Error)
InvalidToken(jsonwebtoken::errors::Error)
NotPaired                  // server hasn't completed /setup yet
```

## Rules

- JWKS URL comes from `core_config` DB table, set during `/setup`.
- Do not hardcode any Supabase project URL here.
- `JwksCache` is `Arc`-shared across all requests via `AppState`.
