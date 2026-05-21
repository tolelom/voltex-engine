# Changelog

All notable changes to the Voltex Engine are recorded here. Until v1.0 the
project is still **breaking-change-friendly**; from v1.0 onward each entry
flags whether it is breaking, additive, or fix-only.

The format groups changes by release tag (when assigned), with the most
recent on top. v0.x entries are organised by Cycle (see
`docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md`).

---

## [v1.0.2] — 2026-05-21 — 600-line rule re-enforcement + full example runtime sweep

### Changed (refactor only — no behavior change)
- `voltex_editor::session` (1060 lines) → `session/{mod.rs 445, tests.rs
  483, e3_tests.rs 152}`. E3 tests carry their own `temp_path` / `cleanup`
  helpers so each file compiles independently.
- `voltex_script::engine_api` (713) → `engine_api/{mod.rs 356, tests.rs
  357}`.
- `voltex_ecs::world` (665) → `world/{mod.rs 351, tests.rs 314}`.
- `voltex_cli::manifest` (612) → `manifest/{mod.rs 450, tests.rs 162}`.
- After splits, the largest source file is `voltex_physics::solver` at
  574 lines (under the project's 600-line guideline).

### Verified
- Runtime smoke-test on the 8 previously-unverified examples
  (model_viewer / many_cubes / asset_demo / multi_light_demo /
  shadow_demo / ibl_demo / deferred_demo / editor_demo) — all 8 exit 124
  (timeout-clean, no panic). With v1.0.1's voltex_survivor +
  tutorial_minigame + runtime_demo + audio_demo + triangle + pbr_demo +
  hierarchy_demo, **all 15 examples + 1 game are Windows-host runtime
  verified**.

### Documentation sync
- STATUS.md M3 row corrected (was 'no violations' — became 4 violations
  during Cycles 4-13 before re-split).
- STATUS.md voltex_ecs test count fixed (was 131, actual 134).
- DEFERRED.md: gizmo / Despawn-undo / mesh AABB picking marked ✅ (done
  in Cycles 3 and 12 but never reflected in DEFERRED).
- DEFERRED.md: new 'v1.0.x 추가 한계' section enumerates every deferral
  introduced in v1.0 / v1.0.1 / v1.0.2 with its source cycle.
- CLAUDE.md 600-line rule references v1.0.2 re-split.

### Tests
- 0 new tests (pure refactor). 921 --lib tests still pass.

---

## [v1.0.1] — 2026-05-21 — Runtime verification + P0/P1 regression patch

### Fixed
- **handler.rs camera ↔ user-system WASD double-binding** (P0). FPS camera
  movement now requires RMB-held, matching scene-view convention. Games can
  use WASD freely via `add_input_system`.
- **`is_key_just_pressed()` always false in user systems** (P1). Moved
  `app.input.begin_frame()` to end-of-frame so just_pressed survives into
  user-system observation.
- **`Bounds::default_bytes` was zero-volume AABB** (P0). Returned 24 zero
  bytes → frustum culling discarded every Inspector "+ bounds" entity. Now
  encodes the unit cube `min=(-0.5)^3, max=(0.5)^3`.
- **`ScriptComponent::default_bytes` was empty path** (P0). Inspector "+
  script" produced an unrunnable component. Now encodes
  `UNBOUND + "scripts/spinner.lua"` so the placeholder exercises the script
  system end-to-end on every `voltex new` project.
- **voltex_survivor enemies / projectiles never executed scripts**. Lua
  `spawn_entity()` returns a bare entity with no ScriptComponent. The
  game's handle-attach system was extended to attach the matching .lua
  based on Tag (`enemy` / `projectile`).

### Added
- 5 regression tests (`voltex_runtime` ×2 input_system order/reference,
  `voltex_ecs` ×3 default_bytes round-trips and non-degenerate Bounds).

### Verified at runtime (Windows host)
- `cargo run -p voltex_survivor`: 6 waves in 30s, 24 enemy kills, no panics.
- `cargo run -p tutorial_minigame`: score system prints, no panics.
- `cargo run -p runtime_demo`: clean exit.

---

## [v1.0.0] — 2026-05-21

### Added (Cycle 15, D3)
- `games/voltex_survivor/` — flagship game. Wave-based arena shooter with
  4 Lua scripts (game/player/enemy/projectile) + scene + Rust input system.
  Game logic 100% in Lua + scene.
- `voltex_runtime::RuntimeApp::add_input_system(F)` — per-frame system that
  receives `&InputState` alongside `&mut World, f32`. Powers WASD movement
  for voltex_survivor.
- `docs/v1.0-retrospective.md` — what worked, what was deferred, statistics,
  next-priority list for v1.1.
- `STATUS.md`: v1.0 release header + Cycle 15 entry.
- Git tag `v1.0.0` (local — user push activates remote release).

### Added (Cycle 14, D1+D2)
- `README.md` — 5-minute quickstart + code-first / editor-first workflows
  + feature list (14 categories) + crate dependency graph + license +
  contributing + verification-limit notes.
- `docs/tutorial.md` — 7-chapter 30-minute tutorial from scaffold to built exe.
- `examples/tutorial_minigame/` — final artifact of the tutorial, dual-workflow
  compatible (cargo run + voltex build). 1 player + 3 enemies + spinner.lua
  + enemy.lua + score system in stdout.

### Deferred to v1.1
- Lua input API (currently Rust handles input via `add_input_system`).
- `voltex_cli editor` subcommand (tutorial points at `examples/editor_demo`).
- 3-OS exe artifact verification (only Windows build verified locally).
- Screenshots — README + tutorial have placeholder markers; PR welcome.

---

## [Unreleased pre-v1.0]

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
