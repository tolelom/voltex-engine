# Changelog

All notable changes to the Voltex Engine are recorded here. Until v1.0 the
project is still **breaking-change-friendly**; from v1.0 onward each entry
flags whether it is breaking, additive, or fix-only.

The format groups changes by release tag (when assigned), with the most
recent on top. v0.x entries are organised by Cycle (see
`docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md`).

---

## [Unreleased] — Slice 3 in progress

### Added (Cycle 13, A — 2026-05-21)
- `voltex` facade `prelude` now re-exports `Bounds`, `MeshHandle`,
  `MaterialHandle`, `ScriptComponent`, `RuntimeApp`, `RuntimeConfig`, `Vfs`.
- `voltex::runtime` module re-export (was missing in B0).
- Crate-level rustdoc on `voltex` lib + tier-1 API docstrings on `World`,
  `Entity`, `EditorCommand`, `EditorSession`.

### Fixed (Cycle 13, A — 2026-05-21)
- `cargo doc --workspace --no-deps` now emits zero warnings (intra-doc link
  escapes in `voltex_math`, `voltex_physics`, `voltex_ai`, `voltex_script`,
  `voltex_renderer`; `voltex_cli` bin's `doc = false` skirts cargo issue
  #6313 collision with the `voltex` lib).

### Deferred to v1.1
- Full `#![warn(missing_docs)]` sweep — 1586 missing public-item docstrings
  across the workspace. v1.0 documents prelude + tier-1 API only.
- Public-item `pub(crate)` audit. Candidate list lives in
  `docs/v1.0-pub-audit.md` (notes only, no code changes).

---

## Cycle 12 (E3, flex — 2026-05-21)

### Added
- `EditorCommand::AddComponent { entity, name, data }` and
  `EditorCommand::RemoveComponent { entity, name }`.
- `EntityAllocator::restore_at_generation(id, gen) -> bool` and the
  matching `World::restore_at_generation` delegate.
- `Component` trait gains default-implemented `remove()` and `default_bytes()`
  methods (backward compatible — existing impls auto-pick up the defaults).
  Built-ins override `default_bytes` with sensible values.
- Inspector panel adds a "+ <name>" button row for the six registered
  built-in components.

### Changed (potentially breaking for in-tree callers)
- `Despawn` is no longer history-clearing — it is fully undoable via a new
  `Inverse::RestoreEntity` that snapshots every registered component
  pre-despawn and restores the original `(id, generation)` on undo.
- `ComponentEntry` grows two fn-pointer fields (`remove`, `default_bytes`).
  `register_raw` populates them with no-op stubs.

---

## Cycle 11 (B3 — 2026-05-21)

### Added
- `voltex_audio::backend::CoreAudioBackend` (macOS, `cfg(target_os = "macos")`).
  AudioToolbox AudioQueue via raw FFI, push-style wrap via Condvar-guarded
  free queue.
- `voltex_cli::build::validate_target` enforces native-only build policy
  with friendly cross-compile / manifest-mismatch error messages.
- `docs/build-matrix.md` — 3-OS prerequisites + commands + voltex.toml
  multi-target example + optional GitHub Actions matrix snippet.

---

## Cycle 10 (P1 — 2026-05-21)

### Added
- `voltex_audio::backend` module with `AudioBackend` trait + `BackendError`.
- `WasapiBackend` (Windows), `AlsaBackend` (Linux, raw libasound FFI),
  `NullBackend` (headless / CI fallback).
- `make_backend()` picks the best OS backend, with hard fallback to null.

---

## Cycle 9 (B2 — 2026-05-21)

### Added
- `voltex_asset::vpkg` — custom `.vpkg` archive format (magic + index table
  + concatenated blobs, no external deps).
- `voltex_asset::vfs::Vfs` — `Filesystem(PathBuf)` or `Archive(VpkgReader)`,
  single `read`/`exists`/`list` API.
- `voltex_runtime::RuntimeConfig::with_vfs(Vfs)` — archive-first scene loader
  (filesystem fallback retained for dev mode).
- `voltex build` packs scene + scripts + assets into a `.vpkg` and embeds it
  in the wrapper via `include_bytes!`, producing self-contained executables.

---

## Cycle 8 (B1 — 2026-05-21)

### Added
- `voltex_cli` production CLI with `new` / `build` / `run` subcommands.
- `voltex.toml` manifest (self-contained mini TOML parser, no external deps)
  with `[project]`, `[build]`, `[build.<triple>]` sections.
- Human-friendly error messages (`MissingField`, `ManifestParse`,
  `CargoMissing`, …).

---

## Cycle 7 (B1-spike — 2026-05-21)

### Added
- `voltex_cli` 1-session prototype + `docs/superpowers/notes/2026-05-21-b1-spike-learnings.md`
  documenting target dir, host triple, and packing decisions for Cycle 8.

---

## Cycle 6 (B0 — 2026-05-21)

### Added
- `voltex_runtime` crate (L3): `RuntimeApp` (winit `ApplicationHandler` +
  GPU init + system tick) + `RuntimeConfig` builder + `RuntimeError`/`RuntimeResult`.
- `examples/runtime_demo` — solar-system scene, `main` body 4 lines.

---

## Cycle 5 (S1 — 2026-05-21)

### Added
- `voltex_ecs::ScriptComponent { script_path, instance_id }` as the sixth
  built-in component.
- `voltex_script::ScriptInstancePool` — one `LuaState` per .lua path,
  shared across all entities pointing at it.
- `script_start_all` / `script_update_all` systems (Lua `on_start(id,gen)` /
  `on_update(id,gen,dt)` dispatch).
- Lua `iter_entities()` / `iter_by_tag(tag)` engine API (15 → 17 fns).

---

## Cycle 4 (E2 — 2026-05-20)

### Added
- `EditorMode { Edit, Play }` + `EditorSession::set_mode` with binary-snapshot
  Play→Stop restore.
- `lifecycle` module — `LifecycleEvent { PlayStart, PlayStop }` with callback
  list for systems (physics, scripts) to hook the transitions.
- `EditorError::ModeMismatch` — dispatch is rejected while in Play mode.

---

## Cycle 3 (E1 — 2026-05-20)

### Added
- `voltex_editor::gizmo` — translate / rotate / scale handles, LINE-list wgpu
  pipeline, depth-test Always overlay rendering.
- `picking::pick_entity` upgraded from sphere approximation to AABB picking
  via the `Bounds` component (sphere fallback kept).
- `examples/editor_demo` integrates the gizmo on the toolbar.

---

## Cycle 2 (C0b — 2026-05-20)

### Added
- `voltex_renderer_ecs` crate (L3) — `RenderAssets` (lockstep handle table),
  `BasicRenderer` (per-entity dynamic UBO pipeline), `RenderItem` +
  `collect_render_items` (helper for custom PBR pipelines).
- `editor_demo` + `survivor_game` migrated off the parallel `Vec<Mesh>`
  pattern to ECS-driven rendering.

---

## Cycle 1 (C0a — 2026-05-20)

### Added
- `voltex_ecs::MeshHandle`, `MaterialHandle`, `Bounds` components (with phantom
  `MeshAsset` / `MaterialAsset` marker types to separate handle ID spaces).
- Auto-registered by `ComponentRegistry::register_defaults()`.

---

## Pre-roadmap milestones

### AI-friendly restructure M1–M7 (2026-05-20)
- CODEMAP.md per crate + CLAUDE.md dependency graph (M1).
- File-split: every `.rs` ≤ 600 lines (M2 + M3).
- Error standardization on `thiserror`-based `<Crate>Error` + `Result<T>` aliases (M4).
- `Component` trait + `register::<T>()` standard registration (M5).
- `voltex` facade crate + `prelude` (M6).
- Final verification + `hierarchy_demo` migration (M7).

### Phases 1–8 (2026-03-24 → 2026-05-19)
Foundation through editor command machinery. See `docs/STATUS.md` for the
per-phase breakdown (Foundation → Rendering → ECS+Scene → PBR → Physics →
Audio → Advanced rendering → AI/Net/Lua/Editor + Editor Command).

---

[Unreleased]: https://git.tolelom.xyz/tolelom/game_engine/compare/main...HEAD
