# src/infrastructure/auth — RS256 JWT validation

JWKS-based JWT validator for the configured auth provider. The only consumer is `presentation/extractors.rs` (`AuthUser`).

## File structure

```
auth/
  mod.rs    — re-exports: jwks
  jwks.rs   — JwksCache, Claims, AuthError
```

## jwks.rs

### JwksCache

Holds an `RwLock<Option<CacheEntry>>` with a 1-hour TTL (`CACHE_TTL = 3600s`). The cache is refreshed lazily on the first call after the TTL expires — there is no background refresh task.

`validate(token, jwks_url)`: fetches JWKS if stale, extracts `kid` from the JWT header, and iterates cached keys. If both token and key have `kid` they must match; if either lacks `kid` the key is tried unconditionally. Returns `Claims { sub, role, exp }` on first successful validation.

`AuthError::NotPaired` is returned by the extractor before pairing is complete — not by `JwksCache` itself.

## Gotchas

- **1-hour stale window**: A rotated key will continue to work until the TTL expires. This is intentional (avoids hammering the JWKS endpoint) but means a key rotation takes up to 1 hour to take effect.
- **`refresh_if_stale` double-lock pattern**: Acquires a read lock to check TTL, drops it, then acquires a write lock to update. There is a TOCTOU window where two concurrent requests could both trigger a refresh. The second write just overwrites with an identical result — harmless.
- **`jwks_url` is not from `Config`**: The JWKS URL comes from `auth_jwks_url` in the `core_config` DB row written by `POST /setup`. If Core is unpaired, `jwks_url()` returns `None` and all protected routes return 401.
- **Only RSA keys are parsed**: Non-RSA keys in the JWKS are silently skipped (`if raw.kty != "RSA" { continue }`).
