# src/ports — Async trait interfaces

Dependency inversion boundary: application services call these traits; infrastructure provides the implementations. Keeping this layer thin ensures services are testable without real I/O.

## File structure

```
ports/
  mod.rs              — re-exports: instance_store, java_store, modpack_store, process_spawner
  instance_store.rs   — InstanceStore trait + StoreError
  java_store.rs       — JavaStore trait
  modpack_store.rs    — ModpackStore trait
  process_spawner.rs  — ProcessHandle, ProcessSpawner, AnySpawner, SpawnError
```

## instance_store.rs

Eight-method trait. The notable one is `reset_transient_statuses` — called once at startup to reset any `starting` or `stopping` instances to `offline` after an unclean shutdown. It returns the count of rows updated.

`StoreError::NotFound` holds a `String`, not an `InstanceId`. The string is the UUID text. Callers that need to convert it back to an `InstanceError::NotFound(InstanceId)` do so manually in the service layer.

## process_spawner.rs — The two-trait pattern

There are two spawner traits, and both live here. This is the most complex port.

`ProcessSpawner` has an associated type `Handle: ProcessHandle`. This makes it non-object-safe — you cannot put it behind `dyn ProcessSpawner`. This is fine for generic code but cannot be stored in `AppState`.

`AnySpawner` is the object-safe version: `spawn_any` returns `Box<dyn ProcessHandle>`. This IS storable as `Arc<dyn AnySpawner>` in `AppState`.

The bridge: a blanket impl at line 75 automatically makes any `ProcessSpawner` satisfy `AnySpawner`. So `PtySpawner` and `MockSpawner` only need to implement `ProcessSpawner`; they get `AnySpawner` for free.

`ProcessHandle` has a second blanket impl (line 30): `Box<dyn ProcessHandle>` itself implements `ProcessHandle`. This lets `spawn_actor` in `instance_actor.rs` be generic over `H: ProcessHandle`, accepting both a concrete handle from `PtySpawner` and a `Box<dyn ProcessHandle>` from `AnySpawner`.

`take_stdout_rx` can only succeed once per handle instance — it moves the `mpsc::Receiver` out. Calling it a second time returns `None`. The actor calls it on startup and panics/logs an error if it gets `None`.

## java_store.rs

Three methods: `sync_all` (upsert batch), `find_by_version` (returns the first matching Java binary path for a given major version), `list_all`. Called only by `restore_instances` and `start_instance`.

## modpack_store.rs

Three methods: `save`, `get_for_instance`, `delete_for_instance`. One manifest per instance — `save` overwrites any existing row. `get_for_instance` returns `Option<ModpackManifest>`.
