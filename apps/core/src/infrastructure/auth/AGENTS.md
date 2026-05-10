# infrastructure/auth/

Supabase RS256 JWT validation via cached JWKS.

## Files

| File | Key types |
|------|-----------|
| `jwks.rs` | `JwksCache`, `Claims`, `AuthError` |
| `mod.rs` | Re-exports |

## `JwksCache`

```rust
pub struct JwksCache {
    inner: tokio::sync::RwLock<Option<CacheEntry>>,
    http:  reqwest::Client,
}
```

- `new(http: reqwest::Client) -> Self`
- `async fn validate(&self, token: &str, jwks_url: &str) -> Result<Claims, AuthError>`

### Cache behaviour

Keys are cached in memory with a 1-hour TTL (`CACHE_TTL = 3600s`).  
`refresh_if_stale` acquires a read lock first (fast path), upgrades to write lock only when stale.  
On refresh, fetches `jwks_url`, parses RSA `n`/`e` components via base64url, builds `DecodingKey`.

### Validation logic

1. Decode JWT header to get `kid`.
2. For each cached `JwkEntry`: if both token and key have a `kid`, they must match; otherwise try all keys.
3. Decode and validate with `Algorithm::RS256`, `validate_exp = true`.

## `Claims`

```rust
pub struct Claims {
    pub sub:  String,           // Supabase user UUID
    pub role: Option<String>,   // "authenticated" | "service_role" | …
    pub exp:  u64,
}
```

## `AuthError`

```rust
enum AuthError {
    InvalidToken,
    NoKeys,
    Fetch(String),
    BadKey,
    NotPaired,
}
```

## JWKS URL

Derived at request time from `core_config.supabase_url` in the DB:

```
{supabase_url}/auth/v1/.well-known/jwks.json
```

`AppState::jwks_url()` queries this. Returns `None` if not yet paired → `AuthUser` extractor returns 401.

## Dev mode bypass

When `config.dev_mode = true`, `AuthUser` extractor returns synthetic `Claims { sub: "dev-owner" }` without touching `JwksCache`. Never enable in prod.

## Rules

- `JwksCache` is stored in `AppState` and shared via `Arc`.
- Do not hardcode any Supabase project URL here.
- JWT signing algorithm is always RS256 — HS256 tokens are rejected.
