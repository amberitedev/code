# Mrpack Rust File Plan (Revised)

**File:** `apps/core/src/infrastructure/modrinth.rs`
**Reference:** `.plan/modrinth-api-reference.md`

---

## Two Categories of Functions

### 1. Per-Instance Functions (modify instance directory)

| Function | Purpose |
|----------|---------|
| `unpack_mrpack(instance_id, mrpack_path)` | Install modpack to instance |
| `install_mod(instance_id, project_id, version_id)` | Install single mod to instance |
| `install_mod_from_hash(instance_id, hash)` | Install mod by file hash |
| `remove_mod(instance_id, project_id)` | Remove mod from instance |
| `list_installed_mods(instance_id)` | Get mods in instance |

### 2. Pure API Functions (just fetch data, same params as API)

| Function | API Endpoint | Params |
|----------|--------------|--------|
| `search(query, facets, index, offset, limit)` | GET /search | Same as API |
| `get_project(id)` | GET /project/{id} | id: project_id or slug |
| `get_projects(ids)` | GET /projects | ids: array |
| `get_versions(project_id, loaders, game_versions)` | GET /project/{id}/version | Same as API |
| `get_version(version_id)` | GET /version/{id} | version_id |
| `get_versions_batch(version_ids)` | GET /versions | ids: array |
| `get_version_from_hash(hash, algorithm)` | GET /version_file/{hash} | hash, sha1/sha512 |
| `get_versions_from_hashes(hashes, algorithm)` | POST /version_files | hashes array |
| `get_latest_from_hash(hash, loaders, game_versions)` | POST /version_file/{hash}/update | hash, filters |
| `get_dependencies(project_id)` | GET /project/{id}/dependencies | project_id |
| `get_loaders()` | GET /tag/loader | none |
| `get_game_versions()` | GET /tag/game_version | none |
| `get_categories()` | GET /tag/category | none |

---

## Per-Instance Functions Detail

### unpack_mrpack

**Params:**
- `instance_id: Uuid` — which instance to install to
- `mrpack_path: PathBuf` — path to .mrpack file

**What it does:**
1. Read instance directory from DB
2. Open ZIP, extract `modrinth.index.json`
3. Parse manifest
4. Filter files: keep where `env.server != "unsupported"`
5. Download each file from URLs in manifest
6. Save to `<instance_dir>/mods/`
7. Extract `overrides/` folder contents
8. Store manifest in DB for future reference

### install_mod

**Params:**
- `instance_id: Uuid`
- `project_id: String`
- `version_id: String` (optional — if missing, get latest)

**What it does:**
1. Get version info from API
2. Download file from version.files[0].url
3. Save to `<instance_dir>/mods/`
4. Check dependencies from version.dependencies
5. Install required dependencies recursively

### install_mod_from_hash

**Params:**
- `instance_id: Uuid`
- `hash: String` (sha1 of existing mod file)

**What it does:**
1. Call `get_version_from_hash(hash)`
2. Get project_id and version_id from response
3. Call `install_mod(instance_id, project_id, version_id)`

---

## Pure API Functions Detail

These are thin wrappers around HTTP calls. Each function:
- Takes same parameters as API endpoint
- Returns same structure as API response
- No instance involvement

### search

```
params:
  query: String (search text)
  facets: Option<Vec<Vec<String>>>  // [[facet:val OR facet:val], [facet:val AND facet:val]]
  index: Option<String> (relevance, downloads, follows, newest, updated)
  offset: Option<i32>
  limit: Option<i32> (default 10, max 100)

returns: SearchResult { hits: Vec<ProjectHit>, offset, limit, total_hits }
```

### get_project

```
params:
  id: String (project_id or slug)

returns: Project { id, slug, title, description, categories, loaders, game_versions, versions, ... }
```

### get_versions

```
params:
  project_id: String
  loaders: Option<Vec<String>>  // ["fabric"]
  game_versions: Option<Vec<String>>  // ["1.20.1"]
  featured: Option<bool>

returns: Vec<Version>
```

### get_version

```
params:
  version_id: String

returns: Version { id, project_id, name, version_number, files, dependencies, game_versions, loaders, ... }
```

### get_version_from_hash

```
params:
  hash: String (sha1 or sha512)
  algorithm: String ("sha1" or "sha512")

returns: Version
```

### get_versions_from_hashes

```
params:
  hashes: Vec<String>
  algorithm: String

returns: HashMap<String, Version>  // hash -> version
```

### get_dependencies

```
params:
  project_id: String

returns: Dependencies { projects: Vec<Project>, versions: Vec<Version> }
```

---

## File Structure

```
apps/core/src/infrastructure/
├── mod.rs              # Add: pub mod modrinth;
├── modrinth.rs         # NEW - all functions
│   ├── structs.rs      # Response types (Project, Version, SearchResult, etc.)
│   ├── api.rs          # Pure API functions (search, get_project, etc.)
│   ├── instance.rs     # Per-instance functions (unpack_mrpack, install_mod)
│   └── mrpack.rs       # Mrpack manifest parsing (structs from libium)
└── ...
```

Or keep everything in one file (~200 lines max per rule):
```
apps/core/src/infrastructure/
├── mod.rs
├── modrinth.rs         # API + instance functions combined
├── mrpack_structs.rs   # Just the mrpack manifest structs (~50 lines)
```

---

## No New Dependencies

Core already has:
- `reqwest` — HTTP client
- `serde + serde_json` — JSON parsing
- `zip` — ZIP extraction
- `tokio` — async runtime
- `sha1` — add if needed for hash verification

---

## Implementation Order

1. Copy mrpack structs from libium (50 lines)
2. Write pure API functions (each is ~5-10 lines, just HTTP + deserialize)
3. Write `unpack_mrpack` (instance-aware)
4. Write `install_mod` (instance-aware)
5. Wire into handlers (`/instances/{id}/modpack/install`)

---

## Summary

- **Per-instance:** unpack_mrpack, install_mod, install_mod_from_hash, remove_mod
- **Pure API:** search, get_project, get_versions, get_version, get_version_from_hash, etc. (all match API params)
- **Reference:** `.plan/modrinth-api-reference.md` has full endpoint details