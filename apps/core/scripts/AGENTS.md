# scripts/

Developer tooling for Amberite Core.

## Files

| File | Purpose |
|------|---------|
| `diagnostic.py` | Hits every REST API endpoint; reports pass / fail / skip |
| `run_diag.ps1` | PowerShell wrapper — auto-installs `requests`, forwards args |

## Running the Diagnostic

Requires the server to be running (`cargo run` from `apps/core/`).

```powershell
# Minimal — unauthenticated endpoints only
.\scripts\run_diag.ps1

# Full — all endpoints
.\scripts\run_diag.ps1 -Token <supabase_jwt>

# First-run pairing + full test
.\scripts\run_diag.ps1 -Token <jwt> -PairingCode <code> -SupabaseUrl https://xxx.supabase.co -OwnerId <uid>
```

## What Gets Tested

| Phase | Endpoints |
|-------|-----------|
| System | GET /health, /version, /java, /setup/status |
| Setup | POST /setup (skipped without --pairing-code) |
| Instances | GET+POST /instances, GET /instances/:id, install poll |
| Lifecycle | start, stop, restart, kill, command, ws-token |
| Mods | list, upload dummy jar, toggle, update, delete, Modrinth add, update-all |
| Logs | list logs, read log file, list crash-reports, read crash report |
| Properties | GET + PATCH server.properties |
| Stats | GET stats (CPU/RAM/players) |
| Modpack | GET, POST install, GET export, DELETE |
| Macros | list, spawn (404 expected), delete if spawned |
| Cleanup | DELETE instance |

## Exit Codes

- `0` — all checks passed or skipped
- `1` — one or more checks failed
