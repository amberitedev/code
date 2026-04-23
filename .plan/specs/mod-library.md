# Amberite Mod Library Specification

**Name:** `amberite-mods` (or `modkit`)
**Language:** Pure Rust
**License:** MIT or Apache-2.0 (permissive, unlike libium's MPL-2.0)
**Base:** Fork libium, strip CurseForge/GitHub, add Amberite-specific features

---

## Goals

- Minimal dependencies (use what Core already has: reqwest, serde, zip, tokio)
- Modrinth-only focus (simpler than libium's multi-platform)
- Server-side modpack handling
- Dependency resolution
- Conflict detection
- Log analysis for problem identification
- Data collection for future AI/troubleshooting

---

## Module Structure

```
amberite-mods/
├── src/
│   ├── lib.rs                 # Entry point, public API
│   ├── api/
│   │   ├── mod.rs             # Modrinth API client
│   │   ├── projects.rs        # Search, get project info
│   │   ├── versions.rs        # Version listing, filtering
│   │   └── tags.rs            # Loaders, game versions, categories
│   ├── manifest/
│   │   ├── mod.rs             # .mrpack format handling
│   │   ├── read.rs            # Parse from ZIP
│   │   ├── write.rs           # Create modpack
│   │   └── structs.rs         # Manifest data types
│   ├── install/
│   │   ├── mod.rs             # Installation orchestration
│   │   ├── download.rs        # File downloads with progress
│   │   ├── server.rs          # Server-side filtering (side: server/both)
│   │   ├── dependencies.rs    # Dependency resolution
│   │   └── verify.rs          # Hash verification
│   ├── conflict/
│   │   ├── mod.rs             # Conflict detection
│   │   ├── detector.rs        # Known incompatible pairs
│   │   └── rules.rs           # Conflict rule database
│   ├── logs/
│   │   ├── mod.rs             # Log parsing
│   │   ├── parser.rs          # Extract errors from Minecraft logs
│   │   ├── identify.rs        # Match errors to mods
│   │   └── patterns.rs        # Common error patterns
│   └── collect/
│       ├── mod.rs             # Data collection
│       ├── problems.rs        # Store problem reports
│       └── export.rs          # Export for analysis/AI training
├── Cargo.toml
└── README.md
```

---

## Core Features

### 1. API Module (Modrinth)

**What libium uses:** ferinth (external dep)
**What we'll do:** Write thin wrapper using reqwest (Core already has it)

```rust
pub struct ModrinthApi {
    client: reqwest::Client,
    base_url: String,  // api.modrinth.com
}

impl ModrinthApi {
    pub async fn search(&self, query: &str, filters: SearchFilters) -> Result<Vec<Project>>;
    pub async fn get_project(&self, id: &str) -> Result<Project>;
    pub async fn get_versions(&self, project_id: &str) -> Result<Vec<Version>>;
    pub async fn get_version_from_hash(&self, hash: &str) -> Result<Version>;
}
```

**SearchFilters:**
- `game_version: Option<String>`
- `loader: Option<Loader>` (fabric, forge, quilt, neoforge)
- `category: Option<Vec<String>>`
- `side: Option<Side>` (client, server, both)

---

### 2. Manifest Module (.mrpack)

**Parse existing .mrpack:**
```rust
pub fn read_mrpack(zip_path: &Path) -> Result<Manifest>;
pub fn read_mrpack_from_bytes(data: &[u8]) -> Result<Manifest>;
```

**Create new modpack:**
```rust
pub fn write_mrpack(manifest: &Manifest, mods_dir: &Path, output: &Path) -> Result<()>;
```

**Manifest struct:**
```rust
pub struct Manifest {
    pub format_version: i32,
    pub game: Game,
    pub version_id: String,
    pub name: String,
    pub summary: Option<String>,
    pub files: Vec<ModpackFile>,
    pub dependencies: Dependencies,
}

pub struct ModpackFile {
    pub path: PathBuf,
    pub hashes: HashMap<HashType, String>,  // sha1, sha512
    pub env: Option<EnvSupport>,
    pub downloads: Vec<Url>,
    pub file_size: i64,
}

pub struct EnvSupport {
    pub client: SupportLevel,  // Required, Optional, Unsupported
    pub server: SupportLevel,
}

pub struct Dependencies {
    pub minecraft: String,
    pub fabric_loader: Option<String>,
    pub forge: Option<String>,
    pub quilt_loader: Option<String>,
    pub neoforge: Option<String>,
}
```

---

### 3. Install Module

**Server-side installation:**
```rust
pub async fn install_modpack(
    manifest: &Manifest,
    output_dir: &Path,
    side: Side,  // Server
    progress: impl Fn(Progress),
) -> Result<InstallReport>;
```

**Dependency resolution:**
```rust
pub async fn resolve_dependencies(
    version: &Version,
    api: &ModrinthApi,
) -> Result<Vec<Dependency>>;
```

**InstallReport:**
```rust
pub struct InstallReport {
    pub installed: Vec<InstalledMod>,
    pub skipped: Vec<SkippedMod>,  // Client-only
    pub dependencies: Vec<Dependency>,
    pub conflicts: Vec<Conflict>,
    pub errors: Vec<InstallError>,
}
```

---

### 4. Conflict Module

**Detection:**
```rust
pub fn detect_conflicts(mods: &[ModInfo]) -> Vec<Conflict>;

pub struct Conflict {
    pub mod_a: String,
    pub mod_b: String,
    pub reason: ConflictReason,
    pub severity: Severity,  // Warning, Error, Fatal
}

pub enum ConflictReason {
    KnownIncompatible,  // From database
    DuplicateFunction,  // Two mods do same thing
    VersionMismatch,    // Dependency version conflict
    LoaderConflict,     // Fabric vs Forge
}
```

**Rule database:** JSON file of known conflicts (collected from community)

---

### 5. Logs Module

**Parse Minecraft logs:**
```rust
pub fn parse_log(log_text: &str) -> Vec<LogEntry>;

pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,  // INFO, WARN, ERROR, FATAL
    pub source: Option<String>,  // Mod name
    pub message: String,
    pub is_error: bool,
}
```

**Identify problem mods:**
```rust
pub fn identify_problems(entries: &[LogEntry]) -> Vec<ProblemMod>;

pub struct ProblemMod {
    pub mod_id: String,
    pub mod_name: String,
    pub error_type: ErrorType,
    pub message: String,
    pub suggested_fix: Option<String>,
}
```

**Common patterns:**
- Mixin failures → mod crashed during injection
- Dependency missing → required mod not installed
- Version mismatch → incompatible versions
- Config error → misconfiguration

---

### 6. Collect Module

**Store problem reports:**
```rust
pub fn save_problem(problem: &ProblemMod, data_dir: &Path) -> Result<()>;

pub struct ProblemReport {
    pub timestamp: DateTime,
    pub mod_id: String,
    pub version: String,
    pub minecraft_version: String,
    pub loader: String,
    pub error_type: ErrorType,
    pub log_snippet: String,
    pub stack_trace: Option<String>,
}
```

**Export for training:**
```rust
pub fn export_problems(data_dir: &Path, output: &Path) -> Result<ExportData>;
```

---

## Dependencies

**Use what Core already has:**
```toml
[dependencies]
reqwest = { version = "0.12", features = ["json", "rustls-tls", "stream"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
zip = "2.2"
tokio = { version = "1.47", features = ["full"] }
chrono = { version = "0.4", features = ["serde"] }
sha1 = "0.10"  # For hash verification (small, add this)
thiserror = "1.0"
url = "2.5"
```

**Total new deps:** +1 (sha1), everything else Core already has.

---

## Fork Strategy

**Option A:** Fork libium, strip CurseForge/GitHub, add features
- Pros: Working base, less initial work
- Cons: MPL-2.0 license, extra baggage

**Option B:** Write from scratch using Core's deps
- Pros: MIT/Apache license, cleaner, exactly what you need
- Cons: More initial work (~2-3 days)

**Recommendation:** Option B — write from scratch, reference libium for patterns.

---

## Integration with Amberite

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Amberite Core  │────▶│  amberite-mods  │────▶│  Modrinth API   │
│  (Axum server)  │     │  (library)      │     │  (api.modrinth) │
└─────────────────┘     └─────────────────┘     └─────────────────┘
        │                       │
        │                       ▼
        │               ┌─────────────────┐
        │               │  Supabase       │
        │               │  (modpack meta) │
        └──────────────▶└─────────────────┘

┌─────────────────┐     ┌─────────────────┐
│  Amberite App   │────▶│  amberite-mods  │
│  (Tauri/Vue)    │     │  (client sync)  │
└─────────────────┘     └─────────────────┘
```

---

## Implementation Order

1. **Day 1:** API module + manifest parsing
2. **Day 2:** Install module (download, server filtering, dependencies)
3. **Day 3:** Conflict + logs modules (basic detection)
4. **Later:** Collect module, pack creation, advanced features

---

## Questions

1. **Library name:** `amberite-mods`? `modkit`? Something else?

2. **License:** MIT or Apache-2.0?

3. **Scope for "usable product":**
   - Just API + manifest + install? (Days 1-2)
   - Or include conflict/logs too? (Days 1-3)

4. **Where to put it:**
   - `libs/amberite-mods/` in monorepo?
   - Separate repo?

5. **Fork or write fresh:**
   - Fork libium (MPL-2.0, less work)?
   - Write fresh (MIT, cleaner)?