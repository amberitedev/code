# Presentation Layer — API Reference

Complete REST + WebSocket API for Amberite Core. Base URL: `http://localhost:16662`

## Architecture

**Layer:** Presentation (HTTP interface to domain services)  
**Framework:** Axum 0.7  
**Port:** 16662  
**State:** `Arc<ServiceRegistry>` injected via `.with_state()`  

### Request Flow
```
HTTP Request → Router → Extractor → Handler → Service → Domain
                                          ↓
Response ← Error Mapping ← Serialization ← Result
```

### Files
| File | Purpose |
|------|---------|
| `router.rs` | `create_router()` — assembles all routes |
| `error.rs` | `ApiError` enum, `IntoResponse` impl, status mapping |
| `extractors.rs` | `AuthExtractor`, `InstanceServiceExtractor` |
| `handlers/auth_api.rs` | `/login`, `/setup` |
| `handlers/instance_api.rs` | `/instances/:id/{start,stop,kill,command}` |
| `handlers/diagnostics_api.rs` | `/health`, `/stats` |
| `handlers/websockets.rs` | `/instances/:id/console` (WS) |

---

## Endpoints

### Diagnostics

#### `GET /health`
Public health check — no authentication, no state dependency.

**Handler:** `diagnostics_api::health_check()`  
**Input:** None  
**Output (200 OK):**
```json
{ "status": "healthy" }
```
**Headers:** `Content-Type: application/json`  
**Errors:** None — always returns 200

---

#### `GET /stats`
System statistics — CPU, memory, running instance count, version.

**Handler:** `diagnostics_api::get_system_stats(State<Arc<ServiceRegistry>>)`  
**Input:** None  
**Output (200 OK):**
```json
{
  "cpu_usage": 0.0,
  "memory_usage": 0,
  "total_memory": 0,
  "running_instances": 3,
  "version": "0.1.0"
}
```
**Fields:**
- `cpu_usage` — `f64`, percentage (0.0–100.0), currently placeholder
- `memory_usage` — `u64`, bytes used, currently placeholder
- `total_memory` — `u64`, total system bytes, currently placeholder
- `running_instances` — `usize`, count from `registry.instance_service.running_count()`
- `version` — `String`, from `env!("CARGO_PKG_VERSION")`

**Errors:** None

---

### Authentication

#### `POST /setup`
Initial server setup — creates first admin user with PASETO key. Call once on fresh install.

**Handler:** `auth_api::setup(State<Arc<ServiceRegistry>>, Json<SetupRequest>)`  
**Input (JSON body):**
```json
{
  "key": "setup-key-minimum-16-characters",
  "username": "admin",
  "password": "securepassword123"
}
```
**Validation (garde):**
- `key` — `length(min = 16)`
- `username` — `length(min = 3, max = 100)`
- `password` — `length(min = 8, max = 256)`

**Output (200 OK):**
```json
{ "token": "<paseto-token>" }
```

**Errors:**
| Status | Body | Cause |
|--------|------|-------|
| 422 | `{ "errors": [{ "field": "...", "message": "..." }] }` | Validation failed |

**Behavior:**
1. Validates request body with `garde`
2. Calls `registry.auth_service.setup()` (implementation in `application/auth_service.rs`)
3. Returns PASETO token for immediate authentication

---

#### `POST /login`
User authentication — validates credentials, returns PASETO token.

**Handler:** `auth_api::login(State<Arc<ServiceRegistry>>, Json<LoginRequest>)`  
**Input (JSON body):**
```json
{
  "username": "admin",
  "password": "securepassword123"
}
```
**Validation (garde):**
- `username` — `length(min = 3, max = 100)`
- `password` — `length(min = 8, max = 256)`

**Output (200 OK):**
```json
{
  "token": "<paseto-token>",
  "user_id": "<uuid-string>"
}
```

**Errors:**
| Status | Body | Cause |
|--------|------|-------|
| 401 | `{ "error": "Invalid credentials" }` | Username/password mismatch |
| 422 | `{ "errors": [...] }` | Validation failed |

**Behavior:**
1. Validates request body
2. Calls `registry.auth_service.authenticate(&username, &password)`
3. Returns token + user ID on success
4. Returns `ApiError::unauthorized()` on failure

**Token Usage:** Include in `Authorization` header: `Bearer <token>`

---

### Instance Management

All instance endpoints require `:id` path parameter (UUID v4 format).

#### `POST /instances/:id/start`
Start a stopped Minecraft server instance.

**Handler:** `instance_api::start_instance(Path<Uuid>, State<Arc<ServiceRegistry>>)`  
**Input:**
- Path: `id` — `Uuid` (e.g., `550e8400-e29b-41d4-a716-446655440000`)

**Output (200 OK):**
```json
{ "status": "booting" }
```

**Errors:**
| Status | Body | Cause |
|--------|------|-------|
| 400 | `{ "error": "Already running" }` | Instance state is `Running` |
| 404 | `{ "error": "Instance not found" }` | UUID not in database |

**Behavior:**
1. Parses `Uuid` from path → `InstanceId::new(id)`
2. Calls `registry.instance_service.start_instance(id).await`
3. Maps `InstanceServiceError::NotFound` → `ApiError::not_found()`
4. Maps `InstanceServiceError::AlreadyRunning` → `ApiError::bad_request()`

---

#### `POST /instances/:id/stop`
Gracefully stop a running instance (sends stop command, waits for shutdown).

**Handler:** `instance_api::stop_instance(Path<Uuid>, State<Arc<ServiceRegistry>>)`  
**Input:**
- Path: `id` — `Uuid`

**Output (200 OK):**
```json
{ "status": "stopping" }
```

**Errors:**
| Status | Body | Cause |
|--------|------|-------|
| 400 | `{ "error": "Not running" }` | Instance state is `Stopped` |
| 404 | `{ "error": "Instance not found" }` | UUID not in database |

**Behavior:**
1. Parses `Uuid` → `InstanceId::new(id)`
2. Calls `registry.instance_service.stop_instance(id, graceful=true).await`
3. Maps `InstanceServiceError::NotRunning` → `ApiError::bad_request()`
4. Maps `InstanceServiceError::NotFound` → `ApiError::not_found()`

---

#### `POST /instances/:id/kill`
Force-kill a running instance (immediate process termination).

**Handler:** `instance_api::kill_instance(Path<Uuid>, State<Arc<ServiceRegistry>>)`  
**Input:**
- Path: `id` — `Uuid`

**Output (200 OK):**
```json
{ "status": "killed" }
```

**Errors:**
| Status | Body | Cause |
|--------|------|-------|
| 404 | `{ "error": "Instance not found" }` | UUID not in database |

**Behavior:**
1. Parses `Uuid` → `InstanceId::new(id)`
2. Calls `registry.instance_service.kill_instance(id).await`
3. Maps error → `ApiError::not_found()`

---

#### `POST /instances/:id/command`
Send a console command to a running instance.

**Handler:** `instance_api::send_command(Path<Uuid>, State<Arc<ServiceRegistry>>, Json<CommandRequest>)`  
**Input:**
- Path: `id` — `Uuid`
- Body (JSON):
```json
{ "command": "say Hello World" }
```

**Output (200 OK):**
```json
{ "status": "sent" }
```

**Errors:**
| Status | Body | Cause |
|--------|------|-------|
| 404 | `{ "error": "Instance not found" }` | UUID not in database |

**Behavior:**
1. Parses `Uuid` → `InstanceId(id)` (direct construction)
2. Calls `registry.instance_service.send_command(id, &req.command).await`
3. Maps error → `ApiError::not_found()`

**Notes:**
- No validation on `command` string — passes directly to Minecraft console
- Command execution is async; response confirms queueing, not completion

---

### WebSocket

#### `GET /instances/:id/console`
WebSocket upgrade for real-time console output streaming.

**Handler:** `websockets::ws_handler(WebSocketUpgrade, Path<Uuid>, State<Arc<ServiceRegistry>>)`  
**Input:**
- Path: `id` — `Uuid`
- Headers: `Upgrade: websocket`, `Connection: Upgrade`, `Sec-WebSocket-Key`, `Sec-WebSocket-Version: 13`

**Output (101 Switching Protocols):**
- Upgraded WebSocket connection

**Message Format (Server → Client):**
```json
{
  "timestamp": 1234567890,
  "line": "[Server] Done (3.145s)! For help, type \"help\"",
  "type": "stdout"
}
```

**Fields:**
- `timestamp` — `i64`, Unix epoch seconds
- `line` — `String`, raw console output line
- `type` — `String`, one of: `"stdout"`, `"stderr"`, `"system"`

**Errors:**
| Status | Body | Cause |
|--------|------|-------|
| 404 | `{ "error": "Instance not found" }` | UUID not in database |

**Behavior:**
1. Parses `Uuid` → `InstanceId::new(id)`
2. Calls `ws.on_upgrade(|socket| async move { ... })`
3. Spawns task to stream console events to WebSocket
4. Client receives JSON messages as server produces output

**Client Usage:**
```javascript
const ws = new WebSocket('ws://localhost:16662/instances/550e8400-e29b-41d4-a716-446655440000/console');
ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log(`[${data.type}] ${data.line}`);
};
```

---

## Error Handling

### `ApiError` Enum (`error.rs`)
```rust
pub enum ApiError {
    NotFound(String),
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    Internal(String),
    Validation { fields: Json<ValidationError> },
}
```

### `IntoResponse` Mapping
| Variant | Status | Body Format |
|---------|--------|-------------|
| `NotFound(msg)` | 404 | `{ "error": msg }` |
| `BadRequest(msg)` | 400 | `{ "error": msg }` |
| `Unauthorized(msg)` | 401 | `{ "error": msg }` |
| `Forbidden(msg)` | 403 | `{ "error": msg }` |
| `Internal(msg)` | 500 | `{ "error": msg }` |
| `Validation { fields }` | 422 | `{ "errors": [{ "field": "...", "message": "..." }] }` |

### Creating Errors in Handlers
```rust
// Simple error
return Err(ApiError::not_found("Instance not found"));

// Validation error
ApiError::validation(vec![ValidationErrorDetail {
    field: "username".into(),
    message: "must be at least 3 characters".into(),
}])
```

---

## Request/Response Patterns

### State Injection
All handlers (except `health_check`) receive `State<Arc<ServiceRegistry>>`:
```rust
pub async fn handler(
    State(registry): State<Arc<ServiceRegistry>>,
    // ... other extractors
) -> Result<Json<T>, ApiError> {
    registry.instance_service.some_method().await?;
    // ...
}
```

### Path Parameters
UUID path params use Axum's `Path<Uuid>` extractor:
```rust
pub async fn handler(
    Path(instance_id): Path<Uuid>,
    // ...
) -> Result<Json<T>, ApiError> {
    let id = InstanceId::new(instance_id);
    // ...
}
```

### JSON Body
Deserialize with `Json<T>` where `T: Deserialize`:
```rust
#[derive(Deserialize, Validate)]
pub struct LoginRequest {
    #[garde(length(min = 3, max = 100))]
    pub username: String,
    #[garde(length(min = 8, max = 256))]
    pub password: String,
}

pub async fn login(
    Json(req): Json<LoginRequest>,
    // ...
) -> Result<Json<LoginResponse>, ApiError> {
    req.validate(&())?;
    // ...
}
```

### Validation
Uses `garde` crate for request validation:
```rust
use garde::Validate;

#[derive(Deserialize, Validate)]
pub struct Request {
    #[garde(length(min = 1))]
    pub field: String,
}

// In handler:
req.validate(&()).map_err(|e| ApiError::validation(...))?;
```

---

## Adding New Endpoints

1. **Create handler** in appropriate `handlers/*.rs` file:
```rust
pub async fn my_handler(
    State(registry): State<Arc<ServiceRegistry>>,
    Json(req): Json<MyRequest>,
) -> Result<Json<MyResponse>, ApiError> {
    // Implementation
}
```

2. **Register route** in `router.rs`:
```rust
.route("/my/path", post(handlers::my_handler))
```

3. **Define request/response types** with `Deserialize`/`Serialize`:
```rust
#[derive(Deserialize, Serialize)]
pub struct MyRequest { /* ... */ }

#[derive(Serialize)]
pub struct MyResponse { /* ... */ }
```

4. **Add validation** if needed:
```rust
#[derive(Deserialize, Validate)]
pub struct MyRequest {
    #[garde(required)]
    pub field: String,
}
```

5. **Map errors** from service layer:
```rust
.service_call().await
    .map_err(|e| match e {
        ServiceError::NotFound(_) => ApiError::not_found("..."),
        ServiceError::InvalidState => ApiError::bad_request("..."),
        _ => ApiError::internal("..."),
    })?;
```

---

## Testing

Colocated tests in each file:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_check() {
        let response = health_check().await;
        assert_eq!(response.status, "healthy");
    }
}
```

Run tests:
```bash
cargo test -- presentation
cargo test -- presentation -- --nocapture
```
