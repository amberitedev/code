# Modrinth API Reference

**Base URL:** `https://api.modrinth.com/v2`
**Staging URL:** `https://staging-api.modrinth.com/v2`
**Rate limit:** 300 requests/minute per IP

---

## All API Endpoints

### Projects

| Endpoint | Method | Description | Params |
|----------|--------|-------------|--------|
| `/search` | GET | Search projects | query, facets, index, offset, limit |
| `/project/{id}` | GET | Get project info | id (project_id or slug) |
| `/projects` | GET | Get multiple projects | ids (array in query) |
| `/project/{id}/version` | GET | List project versions | id, loaders?, game_versions?, featured? |
| `/project/{id}/dependencies` | GET | Get dependencies | id |
| `/project/{id}/team` | GET | Get team members | id |

### Versions

| Endpoint | Method | Description | Params |
|----------|--------|-------------|--------|
| `/version/{id}` | GET | Get version info | id (version_id) |
| `/versions` | GET | Get multiple versions | ids (array in query) |
| `/version/{id}/file` | GET | Get version file info | id |

### Version Files (Hash-based)

| Endpoint | Method | Description | Params |
|----------|--------|-------------|--------|
| `/version_file/{hash}` | GET | Get version from file hash | hash, algorithm (sha1/sha512), multiple? |
| `/version_files` | POST | Get versions from multiple hashes | body: { hashes[], algorithm } |
| `/version_file/{hash}/update` | POST | Get latest version from hash | hash, body: { loaders[], game_versions[] } |
| `/version_files/update` | POST | Get latest versions from hashes | body: { hashes[], algorithm, loaders[], game_versions[] } |

### Tags

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/tag/category` | GET | List categories |
| `/tag/loader` | GET | List loaders (fabric, forge, quilt, neoforge, paper, etc.) |
| `/tag/game_version` | GET | List Minecraft versions |
| `/tag/license` | GET | List licenses |
| `/tag/project_type` | GET | List project types (mod, modpack, resourcepack, shader) |
| `/tag/side_type` | GET | List side types (required, optional, unsupported) |

---

## Search Filters (Facets)

Facet format: `[[facet1:val1 OR facet1:val2], [facet2:val3]]` (OR within array, AND between arrays)

| Facet Type | Examples |
|------------|----------|
| `project_type` | `project_type:mod`, `project_type:modpack` |
| `categories` | `categories:fabric`, `categories:adventure` |
| `versions` | `versions:1.20.1` |
| `client_side` | `client_side:required`, `client_side:optional` |
| `server_side` | `server_side:required`, `server_side:optional` |
| `open_source` | `open_source:true` |
| `license` | `license:mit` |
| `author` | `author:my_user` |

**Example:**
```
facets=[["categories:fabric"],["versions:1.20.1"],["project_type:mod"]]
```

---

## Response Structures

### Project
```json
{
  "id": "AABBCCDD",
  "slug": "my_project",
  "title": "My Project",
  "description": "Short description",
  "body": "Long description",
  "project_type": "mod",
  "categories": ["fabric", "technology"],
  "client_side": "required",
  "server_side": "optional",
  "downloads": 12345,
  "followers": 500,
  "icon_url": "https://...",
  "license": { "id": "MIT", "name": "MIT License" },
  "game_versions": ["1.20.1", "1.20.2"],
  "loaders": ["fabric"],
  "versions": ["IIJJKKLL", "QQRRSSTT"]
}
```

### Version
```json
{
  "id": "IIJJKKLL",
  "project_id": "AABBCCDD",
  "name": "Version 1.0.0",
  "version_number": "1.0.0",
  "changelog": "Changes...",
  "version_type": "release",
  "game_versions": ["1.20.1"],
  "loaders": ["fabric"],
  "featured": true,
  "downloads": 1000,
  "files": [
    {
      "url": "https://cdn.modrinth.com/...",
      "filename": "my_mod-1.0.0.jar",
      "primary": true,
      "size": 50000,
      "hashes": { "sha1": "...", "sha512": "..." }
    }
  ],
  "dependencies": [
    { "project_id": "QQRRSSTT", "dependency_type": "required" }
  ]
}
```

### Search Result
```json
{
  "hits": [
    {
      "project_id": "AABBCCDD",
      "slug": "my_project",
      "title": "My Project",
      "description": "Short description",
      "categories": ["fabric"],
      "project_type": "mod",
      "downloads": 12345,
      "follows": 500,
      "icon_url": "https://...",
      "game_versions": ["1.20.1"],
      "loaders": ["fabric"],
      "latest_version": "1.20.1"
    }
  ],
  "offset": 0,
  "limit": 10,
  "total_hits": 100
}
```

---

## What Core Needs

### Per-Instance Functions (modify instance directory)
1. **unpack_mrpack** — Install modpack to instance
2. **install_mod** — Install single mod to instance
3. **remove_mod** — Remove mod from instance

### Pure API Functions (just fetch data)
All these are just HTTP calls, same parameters as API:

| Function | API Endpoint | Purpose |
|----------|--------------|---------|
| search_projects | GET /search | Search for mods/modpacks |
| get_project | GET /project/{id} | Get project info |
| get_projects | GET /projects | Batch get projects |
| get_versions | GET /project/{id}/version | List versions for project |
| get_version | GET /version/{id} | Get version details |
| get_versions_batch | GET /versions | Batch get versions |
| get_version_from_hash | GET /version_file/{hash} | Identify mod from file hash |
| get_versions_from_hashes | POST /version_files | Batch identify mods |
| get_latest_from_hash | POST /version_file/{hash}/update | Check for updates |
| get_dependencies | GET /project/{id}/dependencies | Get dependency tree |
| get_loaders | GET /tag/loader | List available loaders |
| get_game_versions | GET /tag/game_version | List Minecraft versions |
| get_categories | GET /tag/category | List categories |

---

## Notes

- No auth needed for GET requests
- Auth needed for POST/PUT/DELETE (user actions)
- Use `User-Agent: amberite/0.1.0 (amberite.dev)` header required