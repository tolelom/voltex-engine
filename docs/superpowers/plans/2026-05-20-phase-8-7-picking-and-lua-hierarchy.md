# Phase 8-7: Viewport Picking + Seed Commands + Lua Hierarchy API

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the editor actually clickable (viewport pick → selection), resolve the seed-entity persistence question, and let Lua scripts manipulate scene hierarchy / multi-tag queries.

**Architecture:**
- Picking lives in `voltex_editor::picking` as a pure function: NDC-from-mouse → world ray (via inverse VP) → sphere test per Transform. No new dependency on voltex_physics; we re-implement the trivial sphere test locally so voltex_editor stays L3.
- editor_demo: seed entities go through `EditorCommand::Spawn` + `SetTransform` + `SetTag` so Save/Load is self-consistent.
- Lua: extend `engine_api` with `add_child / remove_child / find_all_by_tag` following the existing `(id, gen)` handle contract.

**Tech Stack:** Rust 1.x, wgpu 28.0, winit 0.30 (already wired). No new dependencies.

---

## Task A: Lua hierarchy API + collection return

**Files:**
- Modify: `crates/voltex_script/src/engine_api.rs`

- [ ] **A1: Add `add_child(p_id, p_gen, c_id, c_gen)` Lua binding**
  - Reject if either entity not alive.
  - Calls `voltex_ecs::hierarchy::add_child(world, parent, child)`.
  - Test: spawn 2, attach, assert `Parent` + `Children`.

- [ ] **A2: Add `remove_child(p_id, p_gen, c_id, c_gen)` Lua binding**
  - Symmetric to add_child, calls `hierarchy::remove_child`.
  - Test: attach then detach, assert `Parent` removed.

- [ ] **A3: Add `find_all_by_tag(tag) -> table` Lua binding**
  - Returns a single Lua table (array) of `{id, gen}` sub-tables.
  - Test: 3 entities with same tag, length == 3.

- [ ] **A4: cargo test -p voltex_script --lib**

- [ ] **A5: Commit `feat(script): add hierarchy + find_all_by_tag Lua bindings`**

---

## Task B: Viewport raycast picking

**Files:**
- Create: `crates/voltex_editor/src/picking.rs`
- Modify: `crates/voltex_editor/src/lib.rs` (re-export)
- Modify: `examples/editor_demo/src/main.rs` (wire click → pick → session.selected)

- [ ] **B1: Helper `ndc_from_viewport(mouse, viewport_rect) -> (f32, f32)`**
  - Converts mouse pixel pos to NDC (-1..1) within the viewport rect.
  - Returns None semantics handled by caller (we pre-check rect.contains).
  - Test: corner cases (top-left, center, bottom-right).

- [ ] **B2: Helper `ray_from_ndc(view_proj_inv, cam_pos, ndc) -> Ray`**
  - Unproject NDC at depth 1.0 to world point; subtract cam_pos for direction.
  - Returns a `voltex_math::Ray`.
  - Test: center NDC produces ray pointing along camera forward axis (compare with `(target - position).normalize()` from a default OrbitCamera).

- [ ] **B3: Sphere intersection `ray_vs_sphere_t(ray, center, radius) -> Option<f32>`**
  - Returns smallest non-negative `t` if the ray hits the sphere; else None.
  - Test: hit straight-on, miss to the side, behind origin.

- [ ] **B4: `pick_entity(world, view_proj_inv, cam_pos, viewport_rect, mouse, default_radius) -> Option<Entity>`**
  - Iterates `world.query::<Transform>()`, tests each as sphere at `transform.position` with `radius = default_radius * max(scale.x, scale.y, scale.z)`.
  - Returns the entity with the smallest hit `t`.
  - Test (with World fixture): 3 cubes at known positions, mouse pointing through a known position picks the right one.
  - Test: empty world or no-hit returns None.

- [ ] **B5: Wire picking into editor_demo**
  - Track mouse-down position when over viewport. On left-button release within a small movement threshold (e.g. 3 px total drag), call `pick_entity` and set `session.selected = Some(e)` (or `None` if no hit).
  - Don't pick when the orbit/pan drag was active.
  - Pull `view_proj` and inverse on click only.

- [ ] **B6: cargo test -p voltex_editor --lib && cargo build -p editor_demo**

- [ ] **B7: Commit `feat(editor): viewport raycast picking (sphere) → session.selected`**

---

## Task C: Seed entities via EditorCommand in editor_demo

**Files:**
- Modify: `examples/editor_demo/src/main.rs`

- [ ] **C1: Replace direct `session.world.spawn()` + `world.add` calls with `session.dispatch(EditorCommand::Spawn)` then `SetTransform` + `SetTag`.**
  - Use `AddChild` command for the Cube1→Cube2 attachment as well.
  - This means seed becomes recoverable via the jsonl log and round-trips through Save/Load identically to user-spawned entities.

- [ ] **C2: cargo build -p editor_demo (no test changes; behavior verified by visual smoke run if available)**

- [ ] **C3: Commit `refactor(editor_demo): seed entities through EditorCommand for log/Save parity`**

---

## Task D: Doc + memory update

- [ ] **D1: Update `crates/voltex_editor/CODEMAP.md`**
  - Add `picking::pick_entity` to public API list.
  - Note: viewport panel now wires left-click to picking.

- [ ] **D2: Update `crates/voltex_script/CODEMAP.md` (engine_api function count 12 → 15)**

- [ ] **D3: Update `docs/dev/STATUS.md` Phase 8-7 section + test counts**

- [ ] **D4: `cargo test --workspace --lib` — verify 783 + new tests all pass**

- [ ] **D5: Commit `docs(status): Phase 8-7 picking + seed-as-command + Lua hierarchy`**
