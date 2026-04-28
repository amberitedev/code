# Desktop App Backend Plan

Plan for everything that goes into `amberite-backend` — the Rust library crate
that powers the `plugin:amberite|*` Tauri namespace.

## What This Is

`amberite-backend` (`apps/app/backend/`) is Amberite's own Rust library crate.
All custom features that are NOT Theseus pass-through logic live here.
The Tauri shell calls into it via `api/amberite/` commands under the
`plugin:amberite|*` namespace.

## What It Is NOT

- It does not replace Theseus. Theseus still handles Minecraft auth,
  profile management, mod downloading, and .mrpack install on the client side.
- It is not the Core. Core (`apps/core/`) is the server-side process.
  This library is the *client* that talks to Core.

## Plan Files

| File | Contents |
|------|----------|
| `features.md` | All backend modules, Tauri commands, Tauri events, frontend changes, Supabase changes |
| `decisions.md` | Architectural decisions with rationale |

## Related Plans

- `.plan/core-rewrite-plan.md` — Core server design (semi-implemented)
- `.plan/theseus-integration-plan.md` — Mod sync workflow
- `.plan/active/decisions.md` — Top-level architectural decisions

## Current State (as of 2026-04-27)

`amberite-backend` is a placeholder. Only `get_placeholder()` exists.
The Tauri plugin registers one command: `plugin:amberite|hello`.

### Immediate Blockers

1. **`mod.rs` duplicate declarations bug** — every `pub mod` is listed twice
   (lines 5–18 duplicated as lines 20–33; `oauth_utils` duplicated on lines 43–44).
   This prevents the app from compiling at all.

2. **Dual error types** — `AmberiteError` in `lib.rs` and `BackendError` in
   `error.rs` are inconsistent. Must be reconciled before adding new modules.

## Compilation Fix Prompt (for Minimax M2.5)

Give this prompt verbatim to fix the compilation bug:

---

> You are fixing a Rust compilation error in a Tauri v2 desktop app project.
>
> **File to fix:** `apps/app/tauri/src/mod.rs`
>
> **Problem:** Every `pub mod` declaration is listed twice (e.g., `pub mod auth;`
> appears on two separate lines). The module `oauth_utils` is also declared twice.
> This is a simple copy-paste duplicate bug.
>
> **Fix:** Remove all duplicate `pub mod` lines so each module appears exactly
> once. Do not change anything else in the file — do not reorder, do not rename,
> do not touch the `TheseusSerializableError` enum or the `impl_serialize!`
> macro below the module list.
>
> After fixing, run `cargo build` from the `apps/app/tauri/` directory.
> If it fails, read the error carefully and fix the specific issue. Do not guess.
> Keep looping — fix → build → fix → build — until `cargo build` reports
> zero errors. Do not stop until it compiles successfully.

---

## Implementation Order

1. Fix `mod.rs` compilation bug (Minimax M2.5 task above)
2. Reconcile dual error types → single `AmberiteError` in `error.rs`
3. `settings.rs` + `auth.rs` — foundational, everything else depends on these
4. `core_launcher.rs` + `pairing.rs` — get Core talking to the app
5. `core_client.rs` + instance Tauri commands — main feature surface
6. `console_stream.rs` + `useCoreConsole.ts` — real-time console
7. Onboarding wizard + Library page merge
8. `supabase_client.rs` + `friends.rs` + `groups.rs` + Supabase tables
9. `mod_sync.rs` + push/pull flow
10. `tunnel.rs` + DNS provisioning (V2)
