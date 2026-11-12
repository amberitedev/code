# Shared Rust

Code the App and Copal both need exists once, in these crates. The App (`packages/engine`,
`apps/app`) and Copal stay thin: state, Tauri commands or HTTP routes, progress.

| Crate                                  | Job                                                                              | Rule                             |
| -------------------------------------- | -------------------------------------------------------------------------------- | -------------------------------- |
| `packages/contracts`                   | The protocol: Endpoints, `ApiError`, limits and share rules, generated to TS     | Ours. Imports none of ours.      |
| `packages/modrinth-content-management` | Modrinth's content decisions: `resolve_content`, content and configuration diffs | Upstream's. Never edit; call it. |
| `packages/bedrock`                     | Everything both sides share that is not sharing                                  | Ours.                            |
| `packages/sharing-client`              | Shares, reaching Cores, and free disk space                                      | Ours.                            |
| `packages/sharing-storage`             | The store process                                                                | Ours.                            |

Imports go one way: `bedrock` and `sharing-client` each import `modrinth-content-management`,
`sharing-storage` imports `sharing-client`, and every crate may import `contracts`. sharing-client
never imports bedrock, and Copal never depends on the engine.

bedrock holds three kinds of code:

1. Borrowed engine files, compiled in place with `#[path]` (`state/`). The engine imports those
   types from bedrock instead of compiling them. Prefer this for any engine file whose dependencies
   are small enough for bedrock to provide.
2. Clones of upstream code tangled with `engine::Engine` (`upstream/`), kept byte for byte where
   possible with all adaptation in one glue file, and declared in `packages/bedrock/ports.toml`.
   The App keeps the originals.
3. Our own shared code: content rules.

sharing-client is one folder per concern: `shares` (how both sides read the backend's share
routes, which they call through `contracts::backend::sharing`), `versions` (reading a share
version and its `RemovedFile`s), `rules` (which files a share carries,
what `blocked` means, and the listing of pickable files), `push` (reading an owner's rows into a
version, the version builder and the writer), `pool` (the store client and the fetch), `install`
(the member install plan: a version's files at each side's paths, less its modpack, with the
configs an update writes, keeps or deletes; each side applies it with its own writer), `diff` (the
share diff both sides show), `crypto`, `net` (a Core's address, TLS pinned to its certificate and
public IPs) and `disk` (free space).

New and restructured Rust follows the engine's layout: `api/` for entry points, `state/<domain>/`
with `model/` (plain types), `adapters/` (sqlite rows, filesystem, network), `commands/` (one
operation per file), optional `domain/` (pure rules), and `mod.rs` re-exports. Use the engine's
names for the same concepts (`ContentSet`, `ContentEntry`, `list_content`, ...).
