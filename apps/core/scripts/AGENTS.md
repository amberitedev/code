# scripts/

Developer tooling for Amberite Core. These scripts exercise the live API — they require the server to be running.

## Files

| File | Purpose |
|------|---------|
| `diagnostic.py` | Hits every REST endpoint; reports pass / fail / skip |
| `core_exercise.ts` | Live workflow runner for scripted Core smoke tests against real instances |
| `run_diag.ps1` | PowerShell wrapper — auto-installs `requests`, forwards all args |

## Running

Start Core first: `cargo run` (or `docker-compose up -d`) from `apps/core/`.

```powershell
# Unauthenticated endpoints only (no token needed)
.\scripts\run_diag.ps1

# Full run — all endpoints
.\scripts\run_diag.ps1 -Token <auth_jwt>

# Include first-run pairing
.\scripts\run_diag.ps1 -Token <jwt> -PairingCode <6-digit-code> `
    -ConvexUrl https://deployment.convex.cloud -AuthJwksUrl https://issuer/.well-known/jwks.json -OwnerId <uid>
```

Live vanilla workflow runner:

```powershell
pnpm exercise vanilla --version 1.21.1 --port 25565
pnpm exercise add-mod --instance <uuid> --project lithium --gameVersion 1.21.1 --loader fabric
```

`core_exercise.ts` uses the HTTP API plus generated instance files (`server.jar`, `launch.json`) to avoid starting before async install finishes.

## What gets tested

| Phase | Endpoints |
|-------|-----------|
| System | GET /health, /version, /java, /setup/status |
| Setup | POST /setup (skipped without `-PairingCode`) |
| Instances | GET /instances, POST /instances, GET /instances/:id |
| Lifecycle | start, stop, restart, kill, command, ws-token |
| Mods | list, upload dummy .jar, toggle, update, delete, Modrinth add, update-all |
| Logs | list logs, read log file, list crash-reports, read crash report |
| Properties | GET + PATCH server.properties |
| Stats | GET stats (CPU/RAM/players) |
| Modpack | GET, POST install, GET export, DELETE |
| Macros | list, spawn (404 expected), delete if spawned |
| Cleanup | DELETE instance |

## Output symbols

- `✓` — passed
- `✗` — failed (non-zero exit code)
- `~` — skipped (requires credentials or running instance)

## Exit codes

- `0` — all checks passed or skipped
- `1` — one or more failures

## Notes

- `diagnostic.py` targets `http://localhost:16662` by default.
- The script creates a real instance (download is async, so it may be in `offline` status during the test run).
- Mod Modrinth install (`add_mod`) requires internet connectivity.
