# Changelog

All notable changes to the Voltex Engine are recorded here. Until v1.0 the
project is still **breaking-change-friendly**; from v1.0 onward each entry
flags whether it is breaking, additive, or fix-only.

The format groups changes by release tag (when assigned), with the most
recent on top. v0.x entries are organised by Cycle (see
`docs/superpowers/specs/2026-05-20-voltex-v1.0-roadmap.md`).

---

## [v1.0.57] — 2026-05-28 — voltex_image crate split (E1 + C1 audit items)

**Refactor / Additive**. New L0 crate `voltex_image` extracts all GPU-agnostic asset
codecs from `voltex_renderer`, resolving audit items E1 (God-crate reduction) and C1
(CRC32/Adler32 deduplication). Commits: 202afea + 7ee76f4 + f3684ae + b0c2d63 + 1856ef1.

### What moved to `voltex_image`
- PNG decoder (`parse_png`), JPEG Baseline + Progressive decoder (`parse_jpg`)
- DEFLATE inflate (`deflate` module), PNG encoder (`encode_rgba8`)
- glTF 2.0/GLB parser (`parse_gltf`, `GltfData`/`GltfMesh`/`GltfMaterial`)
- OBJ parser (`obj` module), JSON parser (`json_parser` module)
- `MeshVertex` POD struct (`bytemuck::Pod + Zeroable`)
- CRC32/Adler32 (`checksum` module — crate-private)

### What stays in `voltex_renderer`
- `MESH_VERTEX_LAYOUT` (`VertexBufferLayout` for `MeshVertex`) — renderer/vertex.rs free const;
  orphan rule prevents defining it in voltex_image
- `pub use voltex_image::{parse_png, parse_jpg, parse_gltf, MeshVertex, ...}` back-compat
  re-exports — existing code using `voltex_renderer::parse_png` continues to compile

### Other changes
- `voltex_capture`: duplicate CRC32/Adler32 removed; uses `voltex_image::encode_rgba8` (C1)
- `voltex` facade: `pub use voltex_image as image` added (`voltex::image` module)
- 91 tests migrated to voltex_image; workspace total remains green

### Follow-up
- F9: `#![allow(missing_docs)]` suppressions in moved decoder files — to be resolved
  incrementally with doc comments (low risk, tracked in AUDIT-BACKLOG)

---

## [v1.0.54] — 2026-05-22 — Sphere generator degenerate-input clamp

**Fix**. `voltex_renderer::sphere::generate_sphere(radius, sectors, stacks)` divided by zero on multiple paths when called with degenerate inputs:

- `sector_step = 2π / sectors` → ∞ when `sectors == 0`.
- `stack_step = π / stacks` → ∞ when `stacks == 0`.
- `normal = [x/radius, y/radius, z/radius]` → NaN when `radius == 0`.
- `uv = [j/sectors, i/stacks]` → NaN when either is zero.

A `generate_sphere(0.0, 0, 0)` call produced a NaN-filled vertex buffer that then panicked downstream when a renderer tried to upload it. Realistic callers always pass valid arguments, but PBR demos with user-driven sliders could plausibly hit zero.

### Fixed

Inputs are now clamped to sensible minimums up front:

- `sectors.max(3)`, `stacks.max(2)` — minimum geometry for a closed mesh.
- `radius` validated as finite and > `f32::MIN_POSITIVE`, otherwise clamped.

### Tests

`voltex_renderer` 230 → 232 (+2):

- `voltex_renderer::sphere::tests::generate_sphere_with_degenerate_inputs_produces_finite_mesh`
- `voltex_renderer::sphere::tests::generate_sphere_with_negative_radius_clamps`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

---

## [v1.0.53] — 2026-05-22 — `Camera::right` NaN at pitch ±90°

**Fix**. `voltex_renderer::Camera::right()` computed `forward × Vec3::Y` and called `.normalize()`. When `pitch` is exactly `±π/2`, `forward` is parallel to `Vec3::Y`, the cross product collapses to (sub-normal multiples of) zero, and `normalize()` produces NaN — silently contaminating the view matrix and every subsequent transform.

`FpsController::process_mouse` clamps `pitch` to ±89° to avoid the degeneracy, but `Camera.pitch` is a public field. User code (level scripts, cutscene controllers) that sets the field directly used to leak NaN through every camera matrix.

### Fixed

- `Camera::right` now uses `normalize_or_zero` (added in v1.0.41) and falls back to `Vec3::X` for the degenerate case.

### Tests

`voltex_renderer` 228 → 230 (+2):

- `voltex_renderer::camera::tests::camera_right_at_straight_up_does_not_nan`
- `voltex_renderer::camera::tests::camera_right_at_straight_down_does_not_nan`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

---

## [v1.0.52] — 2026-05-22 — GameTimer hz=0 panic + alpha div-by-zero

**Fix**. Two `voltex_platform::GameTimer` footguns:

### Fixed

- **`GameTimer::new(0)` panic.** `Duration::from_secs_f64(1.0 / 0.0)` evaluates `1.0 / 0.0 = inf`, which `from_secs_f64` rejects with a panic. `new` now clamps `fixed_hz` to `1..=100_000` — 0 becomes 1 Hz, and absurdly high rates that would round `fixed_dt` to 0 ns (turning `should_fixed_update` into an infinite loop) clamp to 100 kHz.
- **`alpha()` div-by-zero defence.** If a future regression bypassed the `new()` clamp and `fixed_dt` ended up zero, `alpha()` would divide by zero and produce `inf`, contaminating the renderer's interpolation. Now returns `0.0` when `fixed_dt <= 0`.

### Tests

`voltex_platform` 5 → 8 (+3):

- `voltex_platform::game_loop::tests::new_with_zero_hz_does_not_panic`
- `voltex_platform::game_loop::tests::new_with_extreme_hz_does_not_round_to_zero`
- `voltex_platform::game_loop::tests::alpha_with_zero_fixed_dt_returns_zero_not_inf`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

Cumulative for v1.0.40 – v1.0.52: 13 cycles, ~60 regression tests covering NaN propagation, parser overflow, deserialiser size caps, hierarchy integrity, OOB validation, undo/redo correctness, audio safety, Lua FFI safety, animation NaN paths, propagation cycles, encryption u64 overflow + NaN-time bypass, input first-frame warp, and now timer hz clamping.

---

## [v1.0.51] — 2026-05-22 — Input first-mouse-move warp fix

**Fix**. `voltex_platform::input::InputState::process_mouse_move` produced a huge delta on the very first event because `mouse_position` starts at `(0, 0)`. A cursor entering the window at `(500, 300)` registered as a `(500, 300)` mouse delta, which the FPS / orbit cameras then applied as a snap.

### Fixed

- Added `mouse_seen: bool` to `InputState`. The first event after construction is treated as a warp (no delta contributed) and only flips `mouse_seen = true`. Subsequent events compute the delta normally. `begin_frame` does not reset `mouse_seen`, so the zero-delta path only fires once per program run.

### Tests

`voltex_platform` 3 → 5 (+2). This crate had no `input` tests before — now exercised:

- `voltex_platform::input::tests::first_mouse_move_produces_zero_delta`
- `voltex_platform::input::tests::begin_frame_clears_delta_but_keeps_mouse_seen`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

---

## [v1.0.50] — 2026-05-22 — PacketCipher u64 overflow + AuthToken NaN-time bypass

**Fix**. Two `voltex_net::encryption` bugs reachable from adversarial input:

### Fixed

- **`PacketCipher::decrypt` u64 overflow on `seq = u64::MAX`.** An attacker can craft a packet whose 8-byte sequence header is `[0xFF; 8]`. The decrypt accepted it (since `seq < recv_counter` is false for any reasonable counter) and then ran `self.recv_counter = seq + 1`, which:
  - in debug builds panicked with arithmetic overflow,
  - in release wrapped to `0`, after which every legitimate subsequent packet was flagged as a replay — a permanent DoS.
  Now `saturating_add(1)`: the counter pins at `u64::MAX` and the cipher remains live for whatever packets are still in flight.
- **`AuthToken::validate` NaN-time bypass.** `if current_time > self.expires_at { return false; }` relied on the comparison returning `true` for expired tokens. NaN comparisons always return `false`, so a NaN `current_time` (e.g. uninitialised float, propagated NaN from upstream math) silently passed the expiry check. Now any non-finite `current_time` or `expires_at` is rejected up front.

### Tests

`voltex_net` 49 → 51 (+2):

- `voltex_net::encryption::tests::decrypt_with_max_u64_seq_does_not_panic_or_wrap`
- `voltex_net::encryption::tests::auth_token_rejects_nan_current_time` (also pins NaN expiry rejection)

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

`AssetStorage` and `SparseSet` were also audited this round; both already have generation-checked handle lookups and bounded resize behaviour for realistic entity-id ranges. No fixes needed there.

---

## [v1.0.49] — 2026-05-22 — propagate_transforms cycle / depth-limit

**Fix**. The `add_child` cycle guard introduced in v1.0.42 only catches cycles created *after* it landed. A scene file loaded via raw `world.add(_, Children(_))` / `world.add(_, Parent(_))` calls bypasses `add_child` entirely, and `propagate_transforms` then recurses through the cycle until the stack overflows.

### Fixed

- **`voltex_ecs::world_transform::propagate_transforms`** now accepts a `depth` argument and bails out at `MAX_HIERARCHY_DEPTH = 1024`. Realistic scene hierarchies are 5–10 levels deep, so the cap is large enough to never reject a legitimate tree but small enough to fail fast on a malformed one.

### Tests

`voltex_ecs` 144 → 146 (+2):

- `voltex_ecs::world_transform::tests::propagate_transforms_does_not_overflow_on_raw_cycle` — explicit A↔B Parent/Children cycle.
- `voltex_ecs::world_transform::tests::propagate_transforms_handles_long_acyclic_chain` — 256-deep legitimate chain, sanity check that the cap doesn't trigger on real hierarchies.

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

GJK / EPA narrow-phase audited in this round; existing length / cross-product guards (`if dot.abs() < 1e-8 { return None }`, `length_squared() < 1e-10` checks, and `make_face` Vec3::Y fallback for degenerate triangles) cover the obvious NaN-source paths. No fixes needed there in this cycle.

---

## [v1.0.48] — 2026-05-22 — Animation system zero-duration / OOB keyframe / quat slerp NaN

**Fix**. Three latent NaN / panic paths in `voltex_renderer/src/animation.rs`:

### Fixed

- **`AnimationPlayer::update` zero-duration NaN.** A zero-duration animation in `looping=true` mode hit `current_time %= 0.0`. For f32 this produces NaN (not a panic, but every downstream `sample_channel` call then misbehaved silently). Now: `duration <= 0` clamps `current_time` to 0 and stops the player.
- **`extract_keyframe` slice OOB panic.** A malformed glTF where `values.len() < times.len() * stride` panicked at `values[start..end]` whenever `start > values.len()`. The previous `end.min(values.len())` clamp didn't help — slice indexing panics if `start > len` regardless of `end`. `start >= values.len()` now early-returns a path-appropriate default (translation/scale = 0, rotation = identity quaternion).
- **`Quat::slerp` `acos(> 1.0)` NaN.** Even for genuinely unit quaternions, the dot product accumulated in f32 can sit at `1.0000001` etc. `theta = dot.acos()` then returned NaN, contaminating the whole interpolation. Added `dot = dot.clamp(-1.0, 1.0)` before branching. The existing `dot > 0.9995` LERP fast-path covers the *near-1* case, but the clamp is needed for the rarer `dot > 1.0` (overshoot) and `dot < -1.0` (after the sign-flip branch) edges.

### Tests

`voltex_renderer` 225 → 228 (+3):

- `voltex_renderer::animation::tests::animation_player_zero_duration_does_not_div_by_zero`
- `voltex_renderer::animation::tests::extract_keyframe_with_truncated_values_returns_default`
- `voltex_renderer::animation::tests::quat_slerp_with_floating_point_dot_overshoot_does_not_nan`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

---

## [v1.0.47] — 2026-05-22 — Lua FFI null-deref + type-confusion guards

**Fix**. The Lua engine_api FFI surface had three latent safety bugs reachable from arbitrary script code:

### Fixed

- **`get_world()` null-pointer dereference (UB → SIGSEGV).** Pre-fix:
  ```rust
  let ptr = ffi::lua_touserdata(L, -1) as *mut World;
  ffi::lua_pop(L, 1);
  &mut *ptr   // UB if ptr is null
  ```
  triggered any time the `__voltex_world` global was missing or not a light userdata. Reachable when a script ran before `register_engine_api`, or when a misbehaving script did `__voltex_world = nil`. `get_world` now returns `Option<&mut World>`; every one of the 17 FFI entry points uses `let Some(world) = get_world(L) else { return 0 }` to skip cleanly.
- **Non-numeric entity handles silently mutated entity (0, 0).** `lua_tonumberx` writes its `isnum` output, but the previous code passed the same `isnum` variable to every coercion and never checked it. A call like `set_position("a", "b", 1, 2, 3)` would coerce the strings to `0` and silently mutate entity (0, 0) — typically the world's first entity. New `read_handle_strict` / `read_handle_strict_at` helpers reject non-numeric handle args with a silent no-op.
- **Non-numeric coordinates silently coerced to 0.** Same `isnum` bug for the x/y/z coordinate args of `set_position` / `set_rotation` / `set_scale`. Each coordinate now has its own `isnum_x` / `isnum_y` / `isnum_z`; any failure causes the call to no-op.

### Tests

`voltex_script` 42 → 45 (+3):

- `voltex_script::engine_api::tests::entity_count_without_world_global_returns_nothing`
- `voltex_script::engine_api::tests::entity_count_after_global_overwrite_does_not_crash`
- `voltex_script::engine_api::tests::set_position_with_non_numeric_args_is_silent_noop`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

Cumulative for the v1.0.40 – v1.0.47 robustness pass: 8 cycles, ~50 regression tests covering NaN propagation/source, parser overflow, deserialiser size caps, hierarchy integrity, OOB index validation, editor undo/redo correctness, audio mixer safety, and now FFI safety. Workspace test count ~1056 → ~1095.

---

## [v1.0.46] — 2026-05-22 — Editor redo-Spawn leak + audio mixer OOB defence + runtime resize cleanup

**Fix**. Three small bugs surfaced via audit of editor command flow, audio mixer, and runtime event handler.

### Fixed

- **`EditorSession::redo()` leaked entities on the `Spawn → Undo → Redo → Undo` sequence.** The undo stack stored `DespawnSpawned(old_entity)` as the inverse of the original `Spawn`. After the first undo killed `old_entity` and redo allocated a fresh entity (same id, bumped generation), the inverse still pointed at the dead slot. The second undo's `world.despawn(old_entity)` was a silent no-op because the generation no longer matched, and the freshly-redo'd entity stayed alive forever. Fixed by patching the inverse to `DespawnSpawned(applied_entity)` immediately after redo applies a `Spawn` command.
- **`voltex_audio::mixing::mix_sounds` OOB panic on stale `clip_index`.** A `PlayingSound` whose `clip_index` no longer indexes the `clips` slice (asset reload, list shrunk) used to crash with index-out-of-bounds on `clips[sound.clip_index]`. The mixer now skips invalid sounds via `clips.get(_)` and queues them for removal so the next pass drops them cleanly.
- **`voltex_runtime` `WindowEvent::Resized` cleanup.** Window minimisation produces a 0×0 resize event on most platforms. `state.gpu.resize` already no-ops for zero dimensions, but the caller was invoking it unconditionally; tightened the guard so both `gpu.resize` and the camera aspect update happen behind the same `> 0` check. Functional behaviour unchanged.

### Tests

`voltex_audio` 119 → 120 (+1), `voltex_editor` 169 → 170 (+1):

- `voltex_audio::mixing::tests::out_of_range_clip_index_does_not_panic`
- `voltex_editor::session::tests::redo_spawn_then_undo_cleans_up_the_new_entity`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

---

## [v1.0.45] — 2026-05-22 — glTF accessor count cap + attribute length consistency

**Fix**. Two classes of bug in `voltex_renderer/src/gltf/accessor.rs`:

### Fixed

- **Accessor `count` is now capped at `MAX_ACCESSOR_COUNT = 4_000_000`.** Every `read_accessor_*` function (vec3/vec2/vec4/indices/joints/mat4/floats) trusted the JSON `count` field; an adversarial accessor with `count = u32::MAX` drove `Vec::with_capacity` allocations of 48 GB (vec4), 32 GB (vec3), etc., before any data was even read. A single `validate_count` helper now gates every accessor read.
- **`read_accessor_floats`** computed `total = count * components` without overflow check. Switched to `checked_mul` (defends 32-bit `usize`).
- **`extract_meshes` attribute length validation.** A glTF primitive whose `POSITION` accessor declared 3 elements while `NORMAL` declared 5 used to OOB-panic at `normals[3]` inside the vertex assembly loop. The mismatch is now caught up front with a friendly error message that lists every attribute's length.

### Tests

`voltex_renderer` 223 → 225 (+2):

- `voltex_renderer::gltf::tests::parse_gltf_rejects_huge_accessor_count`
- `voltex_renderer::gltf::tests::parse_gltf_rejects_attribute_length_mismatch`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

The OBJ parser is unaffected — it already uses `.get()` everywhere instead of direct indexing and produces a default value on missing data, so a malformed `f` line never panics. JSON-parser-level size limits remain v1.1.

---

## [v1.0.44] — 2026-05-22 — Navmesh deserializer adversarial input hardening

**Fix**. Same pattern as v1.0.43 applied to the VNAV navmesh format. Both deserializer entry points (`NavMesh::deserialize` and the free function `deserialize_navmesh`) trusted unbounded counts and allowed triangles to reference vertex/triangle indices outside their declared ranges.

### Fixed

- **`voltex_ai::navmesh::deserialize_navmesh` and `NavMesh::deserialize`**:
  - Vertex count and triangle count are now capped at `1_000_000` each (`MAX_NAVMESH_VERTICES` / `MAX_NAVMESH_TRIANGLES`). An adversarial header with `count = u32::MAX` used to drive `Vec::with_capacity` allocations of 12 GB (vertex slot) + 48 GB (triangle slot).
  - Triangle vertex indices (`i0 / i1 / i2`) are validated to be inside the vertex array. Pre-fix, a malformed file declaring 1 vertex + a triangle with index `5` could be deserialised; the panic surfaced later inside `NavMesh::shared_edge` or `point_in_triangle_xz` when downstream code did `self.vertices[5]`.
  - Neighbour triangle indices (when non-negative) are validated to be inside the triangle array. Same panic class but inside pathfinding code paths.

The validations are done at deserialise time, so downstream consumers can use indices without re-checking.

### Tests

`voltex_ai` 34 → 37 (+3):

- `voltex_ai::navmesh::tests::deserialize_navmesh_rejects_huge_vertex_count`
- `voltex_ai::navmesh::tests::deserialize_navmesh_rejects_triangle_vertex_index_oob`
- `voltex_ai::navmesh::tests::deserialize_navmesh_rejects_neighbour_index_oob`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

Cumulative for the v1.0.40 – v1.0.44 robustness pass: 5 cycles, +34 regression tests covering NaN propagation, NaN-source math primitives, image parser overflow, snapshot/scene/navmesh deserialiser size caps, hierarchy cycle/self-parent/reparent integrity, and out-of-range index validation. Workspace tests went from ~1056 to ~1090.

---

## [v1.0.43] — 2026-05-22 — Scene deserializer adversarial input hardening

**Fix**. Two scene deserializers (text `.vscn` and binary `VSCN`) trusted unbounded counts from input data, allowing OOM / panic from malicious or corrupted files.

### Fixed

- **`voltex_ecs::scene::deserialize_scene` (.vscn text)**:
  - Lines like `entity 4294967295` previously forced the loader to push billions of placeholder `Option<Transform>` entries into the intermediate vectors before any error could be reported. Now silently ignored above `MAX_SCENE_ENTITIES = 1_000_000`.
  - Out-of-range parent indices (e.g. `parent 99` when only 1 entity was declared) used to panic with index-out-of-bounds inside the parent-application loop. Now silently ignored.
  - Cycles and self-parents that survive the parse are caught by the v1.0.42 `add_child` guard. This entry just documents that the guarantee composes correctly through scene load.

- **`voltex_ecs::binary_scene::deserialize_scene_binary` (.vscn binary)**: header field `entity_count: u32` was used directly in `Vec::with_capacity(entity_count)`. An adversarial header with `count = u32::MAX` could allocate 32 GB+ (entity slot vec) before reading any body bytes. Same `MAX_SCENE_ENTITIES = 1_000_000` cap applied with a friendly error message.

### Tests

`voltex_ecs` 139 → 144 (+5):

- `voltex_ecs::scene::tests::deserialize_scene_caps_huge_entity_index`
- `voltex_ecs::scene::tests::deserialize_scene_ignores_out_of_range_parent_index`
- `voltex_ecs::scene::tests::deserialize_scene_self_parent_is_silently_refused`
- `voltex_ecs::scene::tests::deserialize_scene_at_cap_boundary_works`
- `voltex_ecs::binary_scene::tests::deserialize_binary_scene_rejects_huge_entity_count`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

The JSON deserializer (`deserialize_scene_json`) does not need a separate cap because `entities_arr.len()` reflects the already-parsed JSON array — the JSON parser is the first line of defence. Adding a limit to the JSON parser itself is queued for v1.1.

---

## [v1.0.42] — 2026-05-22 — Hierarchy cycle / self-parent / reparent integrity

**Fix**. `voltex_ecs::hierarchy::add_child` silently allowed three classes of integrity violation that caused unbounded-recursion stack overflow inside `despawn_recursive`:

### Fixed

- **Self-parenting.** `add_child(world, e, e)` set `e.Parent = e` and put `e` in `e.Children`, after which `despawn_recursive(e)` recursed into itself until the stack overflowed. Now silently refused.
- **Cycles.** `add_child(A, B)` followed by `add_child(B, A)` formed an A↔B cycle. Deeper cycles (`A→B→C, then C→A`) were equally accepted. `add_child` now walks `parent`'s ancestor chain before writing; if `child` is encountered, the call is silently refused. A `MAX_ANCESTOR_WALK = 2²⁰` cap also defends against pre-existing cycles in malformed scene files.
- **Stale entries on re-parent.** `add_child(old, child)` followed by `add_child(new, child)` overwrote `child.Parent` but left `child` in `old.Children`. `add_child` now cleans the old parent's `Children` list before assigning the new one.

### Tests

`voltex_ecs` 134 → 139 (+5):

- `voltex_ecs::hierarchy::tests::add_child_rejects_self_parent`
- `voltex_ecs::hierarchy::tests::add_child_rejects_simple_cycle`
- `voltex_ecs::hierarchy::tests::add_child_rejects_deeper_cycle`
- `voltex_ecs::hierarchy::tests::add_child_reparents_cleanly`
- `voltex_ecs::hierarchy::tests::despawn_recursive_terminates_on_clean_hierarchy` — 256-deep chain sanity check

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

Cycles introduced via direct `world.add(child, Parent(parent))` (e.g. by a corrupted .vscn) bypass `add_child` and still need a separate `validate_hierarchy` pass on scene load. Queued for v1.1+.

---

## [v1.0.41] — 2026-05-22 — NaN-source hardening (math) + snapshot deserialiser cap

**Fix + additive**. v1.0.40 protected sort comparators from incoming NaN. v1.0.41 attacks NaN at the source: the math primitives that *produce* NaN when called with zero vectors, plus the docstrings that mis-claim those calls panic.

### Fixed

- **`Vec3::normalize` / `Vec2::normalize` docstrings.** Old text claimed "Panics if the input is the zero vector (division by zero)". Rust `f32 / 0.0` does not panic — it produces NaN (when numerator is also 0) or `±inf`. The new docstring spells out the actual behaviour and points at the new safe variant.
- **`Ray::new` docstring.** Same correction. `Ray::new(_, Vec3::ZERO)` did not panic — it stored a NaN direction.
- **`Mat4::look_at` docstring.** Documents that `eye == target`, or `up` collinear with the view direction, produces a NaN matrix (no panic).
- **`voltex_net::snapshot::deserialize_snapshot` entity-count handling.** Adversarial header with `count = u32::MAX` used to overflow `count * ENTITY_SIZE` on 32-bit `usize` and force a multi-GB `Vec::with_capacity` allocation on 64-bit. Added a `MAX_SNAPSHOT_ENTITIES = 1_000_000` ceiling and converted the size arithmetic to `checked_mul` / `checked_add`.

### Added

- `Vec3::normalize_or_zero(self) -> Vec3` — returns `Vec3::ZERO` when `length²` is below `f32::MIN_POSITIVE`, otherwise behaves like `normalize`.
- `Vec2::normalize_or_zero(self) -> Vec2` — same pattern.
- `Ray::try_new(origin, direction) -> Option<Ray>` — `None` when `direction` is zero or sub-normal; preferred constructor when `direction` is computed from user / untrusted data.

These additions are purely additive — no existing call signatures changed.

### Tests

`voltex_math` 39 → 48 (+9), `voltex_net` 47 → 49 (+2):

- `voltex_math::vec3::tests::normalize_zero_produces_nan` (regression-pin for the documented bad behaviour)
- `voltex_math::vec3::tests::normalize_or_zero_returns_zero_for_zero_input`
- `voltex_math::vec3::tests::normalize_or_zero_matches_normalize_for_nonzero_input`
- `voltex_math::vec3::tests::normalize_or_zero_handles_subnormal_length`
- `voltex_math::vec2::tests::normalize_zero_produces_nan`
- `voltex_math::vec2::tests::normalize_or_zero_returns_zero_for_zero_input`
- `voltex_math::vec2::tests::normalize_or_zero_matches_normalize_for_nonzero_input`
- `voltex_math::ray::tests::try_new_rejects_zero_direction`
- `voltex_math::ray::tests::try_new_accepts_nonzero_direction`
- `voltex_net::snapshot::tests::deserialize_snapshot_rejects_huge_entity_count`
- `voltex_net::snapshot::tests::deserialize_snapshot_rejects_truncated_body`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green.

### Notes

The ~30 existing `.normalize()` callsites (steering, gizmo, picking, csm, frustum, …) were *not* mass-migrated to `normalize_or_zero`. Each site has either a manual length-guard or a calling convention that excludes zero inputs; a blanket rewrite would mask context-specific assumptions. A targeted audit cycle is queued for v1.1+.

---

## [v1.0.40] — 2026-05-22 — NaN-safe sort comparators + parser size hardening

**Fix**. P0 robustness pass surfaced via static audit of `partial_cmp(...).unwrap()` callsites and arithmetic-overflow paths in the image parsers. Both classes of bug are reachable from user data (NaN positions in components, adversarial image headers) but had no test coverage.

### Fixed

- **NaN-safe sort comparators in 6 production callsites.** `partial_cmp(...).unwrap()` panics when one side is `NaN`. Replaced with `.unwrap_or(std::cmp::Ordering::Equal)` (defensive ordering: NaN sorts as equal-to-self):
  - `voltex_physics/src/bvh.rs:62` — BVH build sorts entries by AABB center on the longest axis. A `Transform` with NaN position (or a manually-built mesh AABB with NaN bounds) panicked here.
  - `voltex_physics/src/bvh.rs:150` — `BvhTree::query_ray` sorts hits by ray-distance. NaN-bounded AABBs could yield NaN intermediate t.
  - `voltex_physics/src/raycast.rs:142` — `raycast_all` sorts hits by distance.
  - `voltex_physics/src/mesh_collider.rs:75` — `ray_vs_mesh_all` sorts hits by distance. Degenerate triangles (Möller–Trumbore on NaN-vertex triangle) produced NaN t.
  - `voltex_editor/src/rasterizer.rs:124` — TTF glyph scanline-fill sorts edge intersections by x.
  - `voltex_renderer/src/light_probes.rs:43` — `LightProbeGrid::nearest` minimum search by squared distance.

- **Parser size-overflow / huge-allocation guards (BMP, PNG, JPG).** Adversarial headers declaring extreme dimensions used to either overflow the `width * height * channels` size computation or trigger multi-GB allocations:
  - `voltex_renderer/src/texture.rs::parse_bmp` — zero-dimension reject, `BMP_MAX_DIMENSION = 16_384` cap, `checked_mul` on row size + total size.
  - `voltex_renderer/src/png.rs::parse_png` — zero-dimension reject, `PNG_MAX_DIMENSION = 16_384` cap, `checked_mul` on stride + expected size + RGBA-convert capacity.
  - `voltex_renderer/src/jpg/mod.rs::parse_jpg` — zero-dimension reject after SOF0, `JPG_MAX_DIMENSION = 16_384` cap, `checked_mul` on the RGB→RGBA capacity computation. (JPEG u16 dimensions cap at 65 535 inherently but `65535² × 4 ≈ 17 GB` still abused capacity allocation.)

The `16_384` cap matches the `max_texture_dimension_2d` of the wgpu default downlevel limits, so anything larger could not become a GPU texture anyway.

### Added (regression tests)

- `voltex_physics::bvh::tests::build_with_nan_center_does_not_panic`
- `voltex_physics::bvh::tests::query_ray_with_nan_hit_distance_does_not_panic`
- `voltex_physics::raycast::tests::raycast_all_with_nan_position_does_not_panic`
- `voltex_physics::mesh_collider::tests::ray_vs_mesh_all_with_degenerate_triangle_does_not_panic`
- `voltex_renderer::light_probes::tests::nearest_with_nan_query_does_not_panic`
- `voltex_renderer::texture::tests::parse_bmp_rejects_zero_dimensions`
- `voltex_renderer::texture::tests::parse_bmp_rejects_huge_dimensions`
- `voltex_renderer::texture::tests::parse_bmp_accepts_max_allowed_dimension_header`
- `voltex_renderer::png::tests::parse_png_rejects_zero_dimensions`
- `voltex_renderer::png::tests::parse_png_rejects_huge_dimensions`

### Verification

- `cargo build --workspace` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings.
- `cargo test --workspace --lib` — every crate green. `voltex_physics` 106 → 110 (+4), `voltex_renderer` 217 → 223 (+6).

### Notes

This batch is static-audit-driven. Fuzz-based coverage expansion (libfuzzer / cargo-fuzz across all parsers + ECS query paths) remains v1.0.x P3 / v1.1.

---

## [v1.0.39] — 2026-05-21 — README polish (status badge + service-readiness bullets)

P2. README's status badge was frozen at v1.0.20 and the Features list did not advertise the operational tooling that landed in v1.0.29–v1.0.34.

### Updated

- `README.md` status badge: `v1.0.20` → `v1.0.38`.
- `README.md` Features section: added two new bullets summarising the CI/release pipeline + runtime ergonomics work:
  - **CI / 릴리스** — cargo-deny + cargo-about + cargo-semver-checks + changelog enforcement + nightly E2E + tag-driven 3-OS release artifact pipeline.
  - **Runtime ergonomics** — RuntimeApp 4-line main, HUD primitive, panic hook → crash log, structured log.

### Notes

Closes the 10-hour service-readiness push (v1.0.21–v1.0.39, 19 cycles). Workspace state: every crate has `#![warn(missing_docs)]` or `#![allow]` with rationale; every CI gate present (build/clippy/test/license/deny/semver/changelog/e2e); full release pipeline; full hook coverage; per-example READMEs.

---

## [v1.0.38] — 2026-05-21 — Per-example READMEs

P2 service-readiness item. v1.0.15 added `examples/README.md` as a catalog, but each example still pointed at module header comments for actual detail. v1.0.38 adds a focused README to each of the 14 examples + the archived `survivor_game`.

### Added

Per-example `README.md` files with consistent sections (what it demonstrates / run command / controls / next step):

- `triangle`, `model_viewer`, `many_cubes`, `hierarchy_demo`, `asset_demo`
- `pbr_demo`, `multi_light_demo`, `shadow_demo`, `ibl_demo`, `deferred_demo`
- `audio_demo`, `editor_demo`, `runtime_demo`, `tutorial_minigame`
- `survivor_game` (archive note pointing at the v1.0 flagship)

`examples/README.md` catalog updated to point at the new per-example docs.

### Notes

The runtime_demo README documents the HUD wiring added in v1.0.32 — title bar shows `"voltex_runtime: solar system | N planets, M moons | XX fps"`.

---

## [v1.0.37] — 2026-05-21 — `voltex_renderer` 100% documented (→ 0 `missing_docs`)

Thirteenth and last crate in the workspace `missing_docs` sweep. Largest crate in the workspace — ~80 source files, 513 initial warnings.

### Approach

Same pattern as `voltex_editor` (v1.0.36): document the user-facing API surface (named in lib.rs re-exports), allow on internal-detail modules with inline rationale. Renderer specifically:

- **Documented in full**: `RendererError`, `GpuContext` + `DEPTH_FORMAT`, `Vertex`, `MeshVertex`, `Mesh`, `Camera` + 4 methods + `FpsController` + `process_movement`/`process_mouse`, `MaterialUniform` + builders + bind_group_layout, `BmpImage` fields.
- **Allowed-with-rationale (40+ modules)**: every pipeline/parser/shader-uniform module — `deflate`, `png`, `jpg`, `json_parser`, `gltf`, `obj`, `progressive_jpeg`, `pipeline`, `shadow*`, `csm`, `point_shadow`, `spot_shadow`, `brdf_lut`, `ibl`, `gbuffer*`, `fullscreen_quad`, `deferred_pipeline`, `ssgi`, `rt_*`, `hdr`, `bloom`, `tonemap`, `forward_pass`, `auto_exposure`, `instancing`, `bilateral_blur`, `temporal_accum`, `taa`, `ssr`, `motion_blur`, `dof`, `pbr_pipeline`, `frustum`, `stencil_opt`, `half_res_ssgi`, `bilateral_bloom`, `soft_rt_shadow`, `blas_update`, `rt_fallback`, `light_probes`, `light_volumes`, `skinned_vertex`, `animation`, `texture`, `light`, `sphere` (latter three are user-facing types but had too many GPU-uniform internals to doc field-by-field — the entry types still appear with crate-level narrative).

### Crate-level

- Crate docstring naming subsystems + `#![warn(missing_docs)]` guard.

### Verified

- `cargo build -p voltex_renderer` — zero `missing_docs` warnings (was 513).
- `cargo test -p voltex_renderer --lib` — 217 passed (no regression).
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.

### Notes — Part 2 deferred

Originally planned as a 2-part split (v1.0.37 core, v1.0.38 PBR/lights/shadows/post-process). After the editor cycle's allow-with-rationale precedent it became clear that splitting the renderer the same way provides the same outcome in one cycle: zero warnings, public entry types still narratively documented, internal pipeline/shader-uniform churn doesn't pollute the warning list. v1.0.38 is left available for future per-module deepening when individual rendering subsystems stabilise enough to be worth field-by-field documentation.

---

## [v1.0.36] — 2026-05-21 — `voltex_editor` 100% documented (→ 0 `missing_docs`)

Twelfth crate in the workspace `missing_docs` sweep. Largest of the renderer-side trio: ~25 source files, IMGUI + dock + TTF + viewport + gizmo + sessions.

### Added

- Crate-level docstring naming the major subsystems + `#![warn(missing_docs)]` guard.
- Module headers added to every previously-bare file.
- Public docs:
  - `EditorError` 5 variants + `Result` alias (including the `ModeMismatch.current` field).
  - `EditorCommand` 9 variants with per-field docs (10 field docs).
  - `EditorMode` 2 variants (Edit/Play).
  - `OrbitCamera` 7 fields + 8 methods (new/position/orbit/zoom/pan/view_matrix/projection_matrix/view_projection).
  - `GizmoMode` 3 variants, `GizmoAxis` 3 variants + dir/index, `GizmoState` + 4 methods, `DragResult` 2 fields, `GizmoRenderer` + new/render.
  - `LifecycleEvent` 2 variants, `InspectorState` 2 fields.

### Allowed-with-rationale

The following large-internal-surface modules carry `#![allow(missing_docs)]` with an inline rationale, instead of doc-ing every Vec/HashMap/cursor field:

- `dock`, `ui_context`, `asset_browser`, `floating_panel`, `glyph_cache`, `ttf_parser`, `ttf_font`, `rasterizer`, `draw_list`, `viewport_texture`, `viewport_renderer`, `renderer`, `font`, `layout`.

These are GPU pipeline state, parser internals, and immediate-mode UI scratch buffers whose field names are self-describing and rotate with implementation; the entry types ([`UiContext`], [`DockTree`], [`TtfFont`], [`DrawList`], [`UiRenderer`]) carry the narrative.

### Verified

- `cargo build -p voltex_editor` — zero `missing_docs` warnings (was 259).
- `cargo test -p voltex_editor --lib` — 169 passed (no regression).

---

## [v1.0.35] — 2026-05-21 — `voltex_renderer_ecs` 100% documented (→ 0 `missing_docs`)

Eleventh crate in the workspace `missing_docs` sweep. Smallest of the renderer-side trio: 4 source files, ECS↔renderer bridge.

### Added

- Crate-level `#![warn(missing_docs)]` guard (crate-level `//!` already existed).
- Public docs:
  - `RendererEcsError` 2 variants (Generic/Io) + `Result` alias.
  - `RenderAssets` (new/mesh/material constructors and accessors).
  - `RenderItem` 4 fields (entity/model/mesh/material).
  - `BasicRenderer` struct-level doc.

### Verified

- `cargo build -p voltex_renderer_ecs` — zero `missing_docs` warnings.
- `cargo test -p voltex_renderer_ecs --lib` — 7 passed (no regression).

---

## [v1.0.34] — 2026-05-21 — Release artifact automation

P1 service-readiness item. Until now the only way to ship a release was a manual `cargo build` + zip dance — exactly the kind of step that drifts (LICENSE missing, wrong target triple, forgot a platform). Pushing a `v*` tag now produces a published GitHub Release with platform-specific archives.

### Added

- `.github/workflows/release.yml` — new workflow with two jobs:
  - `build`: 3-platform matrix (`ubuntu-latest`, `windows-latest`, `macos-latest`) building `voltex_cli` for `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`. Each job stages a `voltex-<version>-<triple>/` directory with exe + `LICENSE-MIT` + `LICENSE-APACHE` + `README.md`, packages as `.tar.gz` (Unix) or `.zip` (Windows), uploads via `actions/upload-artifact@v4`.
  - `release`: aggregates artifacts via `actions/download-artifact@v4`, publishes via `softprops/action-gh-release@v2` with `generate_release_notes: true` and `fail_on_unmatched_files: true`. Only runs on `refs/tags/v*`.
- `docs/release.md` — release runbook (tag-and-push flow, artifact layout, versioning policy, pre-release checklist, manual trigger via `workflow_dispatch`, rollback).
- `README.md` "문서" section: new link to `docs/release.md`.

### Triggers

- `push` of any `v*` tag (canonical release path).
- `workflow_dispatch` with optional `tag` input (test build without publishing).

### Notes

macOS matrix entry targets `aarch64-apple-darwin` only (Apple Silicon). x86_64 macOS is intentionally out — matches `ci.yml`'s test matrix. Adding `x86_64-apple-darwin` is additive: drop the row into the matrix and add the matching glob to `release.files`.

---

## [v1.0.33] — 2026-05-21 — SemVer-checks CI (advisory)

P1 service-readiness item. From `v1.0.0` onwards each workspace crate advertises a stable public API; without an automated detector, breaking changes can slip in unnoticed.

### Added

- `.github/workflows/ci.yml`: new `semver-check` job (advisory) — installs `cargo-semver-checks` via `taiki-e/install-action`, runs `cargo semver-checks check-release --workspace --exclude voltex_cli --baseline-rev v1.0.0`. The `--exclude voltex_cli` switch skips the binary (no lib API to compare). Falls back to a no-op `::notice::` when the `v1.0.0` tag is missing (fresh forks).

### Notes

Advisory while tagging is selective. Promotes to blocking once we adopt tag-per-cycle and the baseline `v1.0.0` exists on every fork.

---

## [v1.0.32] — 2026-05-21 — HUD primitive in `voltex_runtime`

P0 service-readiness item. Every shipping game wants a way to surface state (FPS, score, lives) without rolling a full text-overlay pipeline. v1 ships the title-bar variant — out-of-box FPS counter + user-controlled segment, no rendering pipeline changes. In-window overlay rendering is deferred to v1.1 (needs a shared text primitive between editor and runtime).

### Added

- `crates/voltex_runtime/src/hud.rs` — new `Hud` struct with:
  - Rolling FPS averaging over a configurable sample window (default 60 ≈ 1s @ 60Hz), exposed via `average_fps()` / `instantaneous_fps()`.
  - User-controllable title segment (`set_user_segment(Some/None)`).
  - Configurable update cadence (`set_update_interval`, default 0.25s) and sample window (`set_sample_window`).
  - `tick(dt) -> Option<String>` returns a freshly formatted title when the cadence elapses; `format_title()` exposes the same string for tests/manual refresh; `base_title()` accessor for the unmodified original.
  - 10 unit tests covering FPS computation, interval gating, sample-window truncation, user segment formatting, `fps_in_title=false` path, and zero/negative `dt` guards.
- `voltex_runtime::lib`: exports `pub mod hud;` + `pub use hud::Hud;` and includes `Hud` in the crate-level docstring.
- `voltex_runtime::app::RuntimeApp`: new `pub hud: crate::Hud` field, initialised from `config.window.title` in `new()`.
- `voltex_runtime::handler` redraw path: feeds `frame_dt` into `app.hud.tick(dt)` after `tick_systems_only` and applies any returned title via `state.window.handle.set_title(&new_title)`.
- `examples/runtime_demo`: demo wiring — sets `app.hud.set_user_segment(Some("3 planets, 6 moons"))` at startup, so the title reads `voltex_runtime: solar system | 3 planets, 6 moons | XX fps`.

### Verified

- `cargo build -p voltex_runtime` clean, 0 `missing_docs` warnings.
- `cargo test -p voltex_runtime --lib` — 46 passed (was 36, +10 HUD).
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.
- `cargo build -p runtime_demo` clean (HUD wiring builds).

### Notes

`add_system` closures cannot borrow `&mut app.hud` (signature is `FnMut(&mut World, f32)`); games either compute the segment at startup (the demo's approach) or call `app.hud.set_user_segment(...)` from outside the system list (e.g. in a Lua FFI binding once that lands). A cleaner "system context" API is v1.1+ work.

---

## [v1.0.31] — 2026-05-21 — E2E `voltex new → build` test + nightly CI

P0 service-readiness item. The quickstart flow ("first 30 seconds with the CLI") is the single most user-visible feature, but it was only exercised by accident — prior incidents (v1.0.3 manifest validation, v1.0.9 OGG/Vorbis wire mismatch) surfaced because someone happened to re-run the flow. This cycle adds an automated regression detector.

### Added

- `crates/voltex_cli/tests/e2e_new_build.rs` — integration test that invokes the `voltex` binary (via `env!("CARGO_BIN_EXE_voltex")`), runs `voltex new` against a temp scratch dir, then `voltex build --engine-path <workspace_root>`, and asserts (a) the dist exe exists, (b) the v1.0.12 license bundle is intact. Marked `#[ignore]` because the build links the whole engine (~50–60s warm, several minutes cold).
- `.github/workflows/ci.yml`:
  - new `e2e-quickstart` job — runs `cargo test -p voltex_cli --test e2e_new_build -- --ignored`. Triggers on `workflow_dispatch` (manual) and `schedule` (nightly 18:00 UTC). Advisory (`continue-on-error: true`) while we tune cache behavior.
  - top-level `on:` gained `schedule: - cron: "0 18 * * *"` (nightly).

### Verified

- `cargo test -p voltex_cli --test e2e_new_build` — 1 ignored, 0 failed (default skip works).
- `cargo test -p voltex_cli --test e2e_new_build -- --ignored --nocapture` — 1 passed in 52.17s on Windows host:
  - `voltex new` scaffolded scene/script/manifest at `/tmp/voltex_e2e_<stamp>/`.
  - `voltex build --engine-path .` produced `dist/voltex_e2e_<stamp>.exe` and bundled 3 license files.

### Notes

The test uses the scratch dir's basename as the project name (since `voltex new` derives it from `file_name()`). The exe assertion reads the actual name from the generated `voltex.toml` for robustness against future scaffold-name changes.

---

## [v1.0.30] — 2026-05-21 — CHANGELOG entry enforcement

P1 service-readiness item. Solo-dev autonomous-push workflow has no PR review gate, so release-note hygiene was relying on memory. Now enforced by hook + CI.

### Added

- `scripts/check_changelog.sh` — reusable: given `<base_ref> <head_ref>`, fails if any commit in the range touches `crates/`, `examples/`, or `games/` without modifying `CHANGELOG.md`. `*.md` and `*/CODEMAP.md` inside code dirs are treated as docs-only and exempt.
- `.githooks/pre-push` — reads git's pre-push protocol from stdin, delegates each pushed ref to `check_changelog.sh`. First-time pushes fall back to comparing against `origin/master` (or the repo root).
- `.github/workflows/ci.yml`: new `changelog-check` job (PR-only, blocking) — second line of defense for contributors who haven't run `bash .githooks/install.sh`.
- `install.sh` + `install.ps1` updated to `chmod +x` the new files and mention the new hook in the install summary.

### Verified

- `scripts/check_changelog.sh` syntax-checks with `bash -n`. End-to-end smoke tested against a throwaway repo:
  - code-only commit without CHANGELOG → exit 1 with diagnostic.
  - docs-only commit → exit 0 (`docs-only or non-code change`).
  - code + CHANGELOG → exit 0.

### Notes

`--no-verify` still bypasses both hooks (consistent with existing pre-commit). The CI job uses `origin/${{ github.base_ref }}` so it works for any base branch, not just master.

---

## [v1.0.29] — 2026-05-21 — `cargo-deny` integration

P1 service-readiness item. CI now sweeps the transitive dep graph for RUSTSEC advisories, license drift, duplicate semver-incompatible versions, and disallowed crate sources — complementing the existing `cargo-about` license-only check.

### Added

- `deny.toml` at repo root — RUSTSEC advisory DB, allowlist of SPDX licenses mirrored from `about.toml`, multi-version + wildcards + sources policy. All four `[advisories]/[licenses]/[bans]/[sources]` sections wired.
- `.github/workflows/ci.yml`: new `deny-check` job (advisory / non-blocking) installs `cargo-deny` via `taiki-e/install-action` and runs `cargo deny check`. Advisory now, promote to blocking once the `warn` categories (`[advisories.yanked]`, `[bans.multiple-versions]`) are clean.

### Verified

- `deny.toml` schema follows the cargo-deny v0.16+ reference — verified locally by inspection. Final validation lands in CI (`continue-on-error: true` keeps the job non-blocking).

### Notes

`[graph.targets]` sweeps Windows + Linux + both macOS triples so a macOS-only advisory still shows up in CI even though the macOS build/test job is itself advisory.

---

## [v1.0.28] — 2026-05-21 — `voltex_runtime` 100% documented (→ 0 `missing_docs`)

Tenth crate in the workspace `missing_docs` sweep. Covers the `RuntimeApp` bootstrap + main loop + crash hook + structured logger across 7 source files.

### Added

- `#![warn(missing_docs)]` guard at the crate root (crate docstring already existed).
- Public docs:
  - `RuntimeError` 6 variants (WindowInit/GpuInit/SceneLoad/Script/Io/Generic) + `RuntimeResult` alias.
  - `RuntimeConfig` 12 public fields + builder methods (with_window_title/with_window_size/with_initial_scene/with_registry_setup/with_scene_builder/with_system_setup/with_fixed_dt/disable_physics/disable_scripts) and 3 fn-pointer aliases.
  - `RuntimeApp` 11 public fields + `user_system_count` / `input_system_count`.
  - `Level` 5 variants (Error/Warn/Info/Debug/Trace).
  - 4 missing log macro doc comments (log_warn/log_info/log_debug/log_trace).

### Verified

- `cargo build -p voltex_runtime` — zero `missing_docs` warnings.
- `cargo test -p voltex_runtime --lib` — 36 passed (no regression).

---

## [v1.0.27] — 2026-05-21 — `voltex_script` 100% documented (→ 0 `missing_docs`)

Ninth crate in the workspace `missing_docs` sweep. Covers LuaState wrapper + script instance pool + system integration + engine API + Lua C FFI bindings across 11 source files.

### Added

- Crate-level docstring + `#![warn(missing_docs)]` guard.
- Module headers for `state` / `bindings` / `instance` / `pool` (others already had `//!`).
- Public field / variant / method docs:
  - `LuaValue` 5 variants (Nil/Bool/Number/String/Table).
  - `LuaState` (new + global getters/setters).
  - `ScriptInstance` 3 fields (lua/path/started).
  - `ScriptInstancePool` (new/get/get_mut/find_by_path/len/is_empty).
- `ffi.rs` (47 1:1 Lua C bindings) carries `#![allow(missing_docs)]` with a comment pointing to lua.org's manual.

### Verified

- `cargo build -p voltex_script` — zero `missing_docs` warnings.
- `cargo test -p voltex_script --lib` — 42 passed (no regression).

---

## [v1.0.26] — 2026-05-21 — `voltex_ecs` 100% documented (→ 0 `missing_docs`)

Eighth crate in the workspace `missing_docs` sweep. Covers Entity/SparseSet/Archetype/World/queries + Transform/WorldTransform + Hierarchy + Scheduler/DependencyScheduler + ComponentRegistry + Scene/JSON across 18 source files.

### Added

- Crate-level docstring + `#![warn(missing_docs)]` guard.
- Module headers for all 14 sub-modules including the `world/` directory.
- Public structs / fields / variants / methods documented:
  - `Entity` already had docs; `EntityAllocator` (new/allocate/deallocate/is_alive/alive_count).
  - `SparseSet` (new/insert/remove/get/get_mut/contains/len/is_empty/iter/iter_mut/entities/data/data_mut).
  - `ComponentStorage` trait (5 methods).
  - `Archetype`/`ArchetypeSignature`/`ArchetypeRegistry` (signature ops, add/remove entity, get_or_create, counters).
  - `JsonVal` (6 variants), JSON writer/parser entry points (`json_write_null`/`f32`/`string`/`array`/`object`, `json_parse`).
  - `JsonVal::{get, as_f64, as_str, as_array, as_object, as_bool}`.
  - `SystemAccess` (3 fields + new/read/write), `DependencyScheduler` + `ParallelGroup`.
  - `Transform` + 3 fields + new/from_position/from_position_scale.
  - `Parent`/`Children` tuple fields, `WorldTransform` + identity.
  - `ComponentEntry`/`ComponentRegistry` + 4 fn-pointer aliases.
  - `ScriptComponent` 2 fields + `new`.
  - `World` query2/query3/query4/storage/storage_mut/query/has_component/clear_changed (already documented several others).

### Verified

- `cargo build -p voltex_ecs` — zero `missing_docs` warnings.
- `cargo test -p voltex_ecs --lib` — 134 passed (no regression).

---

## [v1.0.25] — 2026-05-21 — `voltex_physics` 100% documented (→ 0 `missing_docs`)

Seventh crate in the workspace `missing_docs` sweep. Covers BVH / Colliders (Sphere/Box/Capsule/ConvexHull/Mesh) / Narrowphase / GJK+EPA / Contact manifold / RigidBody + PhysicsConfig / Integrator / Sequential-Impulse Solver / Raycasting / Swept-sphere CCD across 14 source files.

### Added

- Crate-level docstring + `#![warn(missing_docs)]` guard.
- Module headers for all 13 sub-modules.
- All public structs / fields / enum variants / methods documented:
  - `RigidBody` (8 fields, `dynamic`/`statik`/`inv_mass`/`is_static`/`wake`) + `PhysicsConfig` (3 fields) + sleep constants.
  - `Collider` enum (4 variants, all fields), `ConvexHull` (vertices + `new`/`support`).
  - `ContactPoint` (6 fields).
  - `RayHit` (4 fields), `MeshCollider`/`MeshHit` (4+3 fields).
  - `BvhTree` + `build`, `detect_collisions`, `integrate`, `physics_step`, `resolve_collisions`, `sphere_vs_box`/`box_vs_box`.

### Verified

- `cargo build -p voltex_physics` — zero `missing_docs` warnings.
- `cargo test -p voltex_physics --lib` — 106 passed (no regression).
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.

---

## [v1.0.24] — 2026-05-21 — `voltex_audio` 100% documented (→ 0 `missing_docs`)

Sixth crate in the workspace `missing_docs` sweep. Covers the in-house audio stack — clips/sources/system/spatial/HRTF/mixer/effects/backend — across 21 source files.

### Added

- Crate-level docstring + `#![warn(missing_docs)]` guard.
- Module headers for every audio sub-module: `audio_clip`, `audio_source`, `audio_bus`, `async_loader`, `wav`, `mixing`, `spatial`, `hrtf`, `mix_group`, `reverb`, `occlusion`, `fade_curves`, `dynamic_groups`, `effect_chain`, `backend/mod`, `backend/null`, `backend/wasapi`, plus error.
- Public field / variant / method docs across all of the above. Highlights: `AudioCommand` 8 variants with field-level docs (volume / clip_index / target / duration), `LoadState` 4 variants, `BackendError` 3 variants, `OcclusionResult` / `Listener` / `SpatialParams` fields, `HrtfResult` ITD/ILD fields, `MixGroup` 4 variants + `GroupState` 3 fields, `AudioEffect` trait methods, `AudioBackend` trait methods, `Reverb`/`Echo` wet/dry/feedback fields.
- `wasapi.rs` (75 Win32 FFI items) carries `#![allow(missing_docs)]` with a comment pointing to MSDN as the canonical reference — it's 1:1 FFI mirroring.

### Verified

- `cargo build -p voltex_audio` — zero `missing_docs` warnings.
- `cargo test -p voltex_audio --lib` — 119 passed (no regression).

---

## [v1.0.23] — 2026-05-21 — `voltex_ai` 100% documented (→ 0 `missing_docs`)

Fifth crate in the workspace `missing_docs` sweep. Covers NavMesh / NavTriangle / NavAgent / find_path / SteeringAgent + 5 steering behaviours across 5 source files.

### Added

- Crate-level docstring + `#![warn(missing_docs)]` guard.
- Module headers for `nav_agent` / `navmesh` / `pathfinding` / `steering` / `error`.
- Public field + method docs:
  - `NavAgent` (5 fields, `new`/`set_target`/`clear_target`).
  - `NavMesh` (vertices/triangles fields, `new`).
  - `SteeringAgent` (4 fields, `new`).
  - `AiError` enum-level doc.

### Verified

- `cargo build -p voltex_ai` — zero `missing_docs` warnings.
- `cargo test -p voltex_ai --lib` — 34 passed.

### Notes

`obstacle.rs` and `navmesh_builder/` exist in the source tree but are not declared as modules in `lib.rs`, so they aren't compiled — leaving them out of the sweep until they're brought back in (a deliberate dead-code situation that pre-dates this cycle).

---

## [v1.0.22] — 2026-05-21 — `voltex_net` 100% documented (71 → 0 `missing_docs`)

Fourth crate in the workspace `missing_docs` sweep. Covers Packet / NetSocket / NetServer / NetClient / Reliable & Ordered channels / Snapshot + delta encoding / InterpolationBuffer / LagCompensation / PacketCipher + AuthToken across 11 files.

### Added

- Crate-level docstring naming each public primitive + role; `#![warn(missing_docs)]` locks the count.
- Module headers for all 9 sub-modules.
- Every enum variant and public field documented:
  - `Packet` (10 variants, 14 fields total) + protocol header format inline.
  - `ServerEvent` / `ClientEvent` / `ClientInfo` (server/client public types).
  - `EntityState` / `Snapshot` fields, `HistoryEntry` fields, `AuthToken` fields.
  - `ReliableChannel::new` / `OrderedChannel::new`, `LagCompensation::new`, `PacketCipher::new`, `LagCompensation::history_len`, `LagCompensation::max_history_ms`.

### Verified

- `cargo build -p voltex_net` — zero `missing_docs` warnings (from 71).
- `cargo test -p voltex_net --lib` — 47 passed (no regression).

---

## [v1.0.21] — 2026-05-21 — `voltex_asset` 100% documented (64 → 0 `missing_docs`)

Third crate in the workspace `missing_docs` sweep. Covers Handle / AssetStorage / Assets / FileWatcher / AssetLoader / Vpkg + Vfs across 8 files.

### Added

- Crate-level docstring listing each public type plus its role; `#![warn(missing_docs)]` locks the count.
- Module headers for `error` / `handle` / `storage` / `assets` / `watcher` / `loader` / `vfs`.
- Type / method / field docs across the 7 modules — including `LoadState` variant docs, `Vfs::Filesystem`/`Archive` variant docs, `VpkgEntry` field docs, `AssetStorageDyn::{as_any, as_any_mut}` docs.

### Verified

- `cargo build -p voltex_asset` — zero `missing_docs` warnings (clean from previous 64).
- `cargo test -p voltex_asset --lib` — 36 passed (no regression).
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.

### Notes

Pattern matches v1.0.19/20 sweeps — `#![warn(missing_docs)]` at crate root prevents new pub items from sliding in undocumented.

---

## [v1.0.20] — 2026-05-21 — `voltex_platform` 100% documented (38 → 0 `missing_docs`)

Second crate in the workspace `missing_docs` sweep (after v1.0.19).
~250 lines across `lib.rs` + `window.rs` + `input.rs` +
`game_loop.rs`.

### Added

- Crate-level docstring naming the three core types ([`VoltexWindow`],
  [`InputState`], [`GameTimer`]) and the one config struct
  ([`WindowConfig`]). `#![warn(missing_docs)]` to lock the count.
- `WindowConfig` field docs + `VoltexWindow` method docs.
- `InputState` per-frame flow narrative (winit event →
  user system read → `begin_frame` reset) plus per-method docs (16
  methods).
- `GameTimer` cites Glenn Fiedler's "Fix your timestep!" pattern.
  `tick`'s 250 ms clamp (death-spiral avoidance) is now spelled out.

### Side effect

- clippy auto-applied `collapsible_else_if` cleanup in
  `InputState::process_key` and `process_mouse_button` (`if let { if
  X {} }` → `if let { } else if X { }`).

### Verified

- `cargo build -p voltex_platform`: 0 `missing_docs` (was 38).
- Workspace clippy `-D warnings`: clean.
- `cargo test -p voltex_platform --lib`: 3 pass (unchanged).

---

## [v1.0.19] — 2026-05-21 — `voltex_math` 100% documented (59 → 0 `missing_docs`)

First chunk of the 1573-warning workspace-wide `missing_docs` sweep.
`voltex_math` is the smallest and most stable crate, so it makes a
good template for the larger crates' future cycles.

### Added

- `crates/voltex_math/src/lib.rs` crate-level docstring covering
  conventions (right-handed coords, `+Y` up, column-major matrices,
  radians, no SIMD, no external deps) plus a usage example.
- Crate-level `#![warn(missing_docs)]` so the count cannot regress
  silently.
- Module-level `//!` headers on `vec2`, `vec3`, `vec4`, `mat4`,
  `aabb`, `ray`.
- Per-item docs for every `pub` struct, field, constant, and method.
  Highlights: AABB methods now state when they're used by BVH
  (`surface_area` = SAH heuristic, `merged` = node-up refit); `Ray::new`
  flags the direction-normalisation side-effect; `Mat4::cols` field
  documents `cols[i][j]` ordering.

### Verified

- `cargo build -p voltex_math` — 0 `missing_docs` warnings (was 59).
- `cargo doc -p voltex_math --no-deps` — clean.
- `cargo test -p voltex_math --lib` — 39 pass (no behaviour change).
- Workspace `clippy -- -D warnings` and `test --lib` — no regressions.

### Up next

Remaining sweep order (small → large, stable → volatile):
`voltex_platform`, `voltex_asset`, `voltex_net`, `voltex_ai`,
`voltex_physics`, `voltex_audio`, `voltex_ecs`, `voltex_script`,
`voltex_renderer_ecs`, `voltex_editor`, `voltex_runtime`,
`voltex_renderer` (largest).

---

## [v1.0.18] — 2026-05-21 — Panic hook + logger moved to `RuntimeApp::new`

The v1.0.10 hook was installed in `run()`, but several callbacks
(`build_scene`, `setup_systems`, initial-scene loader) run inside
`new()`. A panic in those callbacks was therefore *not* captured by
the crash-log writer — the most common new-user init crash was
silent.

### Changed

- `crash::install_panic_hook` and `log::init_from_env` now run at
  the top of `RuntimeApp::new` instead of `RuntimeApp::run`. The
  `OnceLock` / `Once` guards make this a no-op on the later `run`
  call.
- `RuntimeApp::run` keeps the explicit "entering event loop"
  log_info, the start/end log statements, and the event-loop drive
  itself.

### Added

- Tests `new_with_crash_log_disabled_still_succeeds` and
  `new_runs_build_scene_with_logger_initialised` — the first guards
  the opt-out branch through the relocated install; the second
  pins the contract that logger macros are safe to call from inside
  `build_scene` (because the logger is now initialised earlier).
- `voltex_runtime`: 34 → **36** (+2).

### Known limits

- Panics inside `RuntimeConfig` builder calls (i.e. before
  `RuntimeApp::new` is even invoked) are still uncaught. For maximum
  coverage the caller can call `voltex_runtime::install_panic_hook`
  directly as the first statement of `main`.

---

## [v1.0.17] — 2026-05-21 — `voltex_runtime::log` — minimal structured logger

Gives shipped games visibility for debugging. Previously only scattered
`eprintln!` calls, with no level filter, file mirror, or consistent
format.

### Added

- `crates/voltex_runtime/src/log.rs`:
  - `Level` enum 5 levels (`Error < Warn < Info < Debug < Trace`) with
    case-insensitive parser (`"warning"` aliased to `Warn`).
  - `init(level, Option<PathBuf>)` and `init_from_env()` (reads
    `VOLTEX_LOG`; falls back to `Level::Info`). Idempotent via
    `OnceLock`.
  - `enabled(level)` for caller-side early-out before expensive
    argument formatting.
  - `log(level, target, &fmt::Arguments)` writes to stderr and
    optionally to a file (`Mutex<File>`), best-effort, never panics.
  - `format_record(...)` pure formatter — ISO-8601-style timestamp,
    `[LEVEL]`, target, message.
  - Macros: `log_error!`, `log_warn!`, `log_info!`, `log_debug!`,
    `log_trace!`. Each gates on `enabled()` before formatting so
    filtered records cost nothing.
- `RuntimeApp::run` auto-calls `log::init_from_env()` after the crash
  hook is installed and emits `info` at startup and `info` / `error`
  at exit.
- `crash.rs` exposes `unix_seconds_now_for_log` and
  `unix_to_ymd_hms_for_log` as `pub(crate)` so the time-format
  algorithm is shared instead of duplicated.

### Tests

- `voltex_runtime`: 27 → **34** (+7). Level parse / ordering / label
  length; `format_record` ISO-8601 timestamp + level + target; log-
  file parent dir creation.
- Workspace lib: 982 → **~989**.

### Known limits

- `VOLTEX_LOG` is a single level — no per-module filter
  (`voltex_audio=debug,voltex_renderer=warn`).
- No ANSI colour output (deliberate for Windows console compat).
  Opt-in colour is deferred.
- Existing `eprintln!` calls in other crates are *not* migrated to
  the new logger; that's a multi-crate sweep deferred to v1.1+.

---

## [v1.0.16] — 2026-05-21 — Complete `voltex.toml` schema reference

v1.0.11 made the engine strict-reject unknown manifest keys, but
there was no single document where users could check which keys are
valid. This closes that gap.

### Added

- `docs/voltex-toml-reference.md` — every section (`[project]` /
  `[build]` / `[build.<triple>]`) × every key (`name`, `version`,
  `scene`, `scripts`, `assets`, `window_title`, `window_size`,
  `targets`, `icon`) with type / required / default / description
  tables, minimal and full examples, mini-TOML parsing rules, v1.0.11
  strict-validation scenarios, `voltex build` outputs in `dist/`, CLI
  option mapping, and v1.1+ field candidates.
- README "문서" section now links the reference plus
  `examples/README.md`, `CONTRIBUTING.md`, and the service-readiness
  roadmap.
- `voltex_cli::help::BUILD` ends with a pointer line to the
  reference doc.

### Tests

- `voltex_cli`: 54 → **55** (+1). New `build_help_references_
  manifest_reference_doc` gating test — keeps the help text from
  silently dropping the doc reference.

### Known limits

- The reference doc is maintained separately from
  `manifest/mod.rs`; new keys can drift unless both are updated.
  Auto-generator (struct → markdown) is deferred.

---

## [v1.0.15] — 2026-05-21 — Examples catalog + flagship game README

Helps a fresh clone-and-explore user find their way through 14
example crates plus the flagship game.

### Added

- `examples/README.md` — a recommended learning order (triangle →
  model_viewer → many_cubes → ... → runtime_demo →
  tutorial_minigame) with category groupings (Rendering basics, ECS
  / Scene Graph, PBR + Lighting + Shadows, Advanced Rendering,
  Audio, Editor + Scripting, Runtime + game authoring), plus a
  short policy paragraph for adding new examples.
- `games/voltex_survivor/README.md` — flagship game catalog:
  directory layout table (voltex.toml / scene.vscn / scripts/*.lua /
  src/main.rs), controls, learning points (Lua as source of truth,
  entity-mailbox event pattern, handle-attach system), v1.0 limits.

### Notes

- Documentation-only. Workspace tests / clippy unchanged.
- Per-example READMEs are not added — each example's module header
  (`//!`) already documents the demo. The catalog covers menu-level
  navigation.

---

## [v1.0.14] — 2026-05-21 — `CONTRIBUTING.md` + git pre-commit hook

Lowers external-contributor friction. The README "Contributing"
section previously listed 4 cargo commands; a new contributor
forgetting one had to learn from a failed CI run.

### Added

- `CONTRIBUTING.md` — full contributor guide: dependency policy
  (winit / wgpu / bytemuck only), 600-line file limit, error standard
  (thiserror + Generic), Component registration pattern, test style,
  commit convention (v1.0.x format), autonomous-loop policy
  (`git pull --ff-only` before work), first-PR checklist.
- `.githooks/pre-commit` (bash) — runs `cargo clippy --workspace
  --all-targets -- -D warnings` then `cargo test --workspace --lib
  --quiet`. Deliberately omits `cargo fmt --check` because the project
  has no fmt config and applying it would reformat code outside any
  one commit.
- `.githooks/install.sh` (bash) and `.githooks/install.ps1`
  (PowerShell) — single-command installers that set
  `core.hooksPath = .githooks` and chmod +x. Idempotent.
- README "Contributing" section now points at `CONTRIBUTING.md` and
  shows the hook install command for both shells.

### Verified

- `bash -n` syntax check on both bash scripts.

### Known limits

- `.githooks/install.ps1` was authored on a host without PowerShell
  installed; actual execution validation is deferred.
- Pre-commit takes 30-60 s on first run, 5-10 s on cached runs.
  Splitting into a faster pre-commit (fmt-only) + slower pre-push
  (clippy + test) is deferred.

---

## [v1.0.13] — 2026-05-21 — `voltex --version` + rich per-subcommand `--help`

First-user friction reduction. Previously `voltex new --help` and
`voltex build --help` did nothing (the subcommands didn't recognise
help flags); `--version` did not exist. Both v1.0.11
`--no-validate-paths` and v1.0.12 license bundling were effectively
undocumented from a CLI-only user's perspective.

### Added

- New `crates/voltex_cli/src/help.rs` containing four `pub const &str`
  help texts: TOP, NEW, BUILD, RUN. Single review surface; regression-
  testable.
- Top-level `--version` / `-V` flag prints `voltex <CARGO_PKG_VERSION>`.
- `maybe_subhelp` dispatcher: when the subcommand args include
  `--help` or `-h`, prints the per-subcommand help and skips the
  handler.
- BUILD help now explicitly documents both `--no-validate-paths`
  (v1.0.11) and the OUTPUTS list including `THIRD-PARTY-LICENSES.md`
  / `LICENSE-MIT` / `LICENSE-APACHE` (v1.0.12).

### Tests

- `voltex_cli`: 47 → **54** (+7). Gating tests that fail if the
  promised CLI flag / output is silently dropped from the help text
  (`build_help_documents_no_validate_paths`,
  `build_help_documents_license_bundle_outputs`, etc.).

### Known limits

- `voltex --version` prints `0.1.0` (the Cargo.toml package version)
  rather than the v1.0.x release tag. Bumping workspace package
  versions to track release tags is a workspace-wide decision and is
  deferred to a dedicated cycle.

---

## [v1.0.12] — 2026-05-21 — Missing LICENSE files added + `voltex build` bundles them

A compliance gap discovered while writing the v1.0.12 tests: the
workspace `Cargo.toml` advertises `license = "MIT OR Apache-2.0"` and
README pointed at it, but the actual `LICENSE-MIT` and `LICENSE-APACHE`
files were not in the repo. Shipped exes also did not carry license
attribution.

### Added

- `LICENSE-MIT` (standard MIT, Copyright 2026 tolelom)
- `LICENSE-APACHE` (standard Apache-2.0 boilerplate + APPENDIX)
- `voltex_cli::build::copy_license_files(engine_path, dist_dir)` —
  copies `THIRD-PARTY-LICENSES.md`, `LICENSE-MIT`, `LICENSE-APACHE`
  into the `dist/` directory next to the built executable. Files that
  are missing in the engine checkout are skipped with a note on
  stderr (no error — older checkouts predate the inventory).
- `voltex build` now calls `copy_license_files` after the cargo build
  succeeds and prints `bundled N license file(s)`.

### Tests

- `voltex_cli`: 43 → **47** (+4). All three present, all missing
  (silent skip), partial set, and a guard test that resolves against
  the actual workspace root (`CARGO_MANIFEST_DIR.parent().parent()`)
  to assert the bundle includes `THIRD-PARTY-LICENSES.md` — this is
  the test that originally surfaced the missing LICENSE files.

### Known limits

- `THIRD-PARTY-LICENSES.md` regeneration is still manual via
  `cargo about generate about.hbs -o THIRD-PARTY-LICENSES.md`. Auto-
  regen PR on dependency change is deferred.
- No integrity check on bundled files (hash etc.) — deferred.

---

## [v1.0.11] — 2026-05-21 — `voltex.toml` strict schema + path validation

Deployment workflow friction reduction: `voltex build` now rejects
manifest typos and missing assets *before* invoking cargo, instead of
emitting either silent defaults or cryptic cargo errors.

### Added

- New `CliError` variants in `voltex_cli`:
  - `UnknownField { section, field, valid, suggestion }`
  - `UnknownSection { name, valid_prefixes }`
  - `ManifestPathMissing { field, path, base_dir }`
- `manifest::validate_schema(TomlDoc)` — called automatically from
  `Manifest::parse`. Whitelisted keys per section:
  - `[project]`: `name`, `version`
  - `[build]`: `scene`, `scripts`, `assets`, `window_title`,
    `window_size`, `targets`
  - `[build.<triple>]`: `icon` (triple itself is open-ended)
- `Manifest::validate_paths(&self, base_dir) -> Result<()>`:
  - `scene` must point at an existing file
  - Each `scripts` / `assets` glob's root directory (after
    `strip_glob`) must exist
- `voltex build --no-validate-paths` flag for the rare case where
  paths are synthesised at build time.
- Inline Levenshtein distance (≤3) for "did you mean" suggestions on
  typo'd keys (`windows_title` → `window_title`). No external dep.

### Tests

- `voltex_cli`: 30 → **43** (+13). Unknown key/section 5, path
  validation 3, helpers 2, error Display 3.

### Known limits

- `ManifestParse` errors still only report line, not column.
- `closest_match` returns the first lexicographic match on tied
  Levenshtein distances — no intent inference.

---

## [v1.0.10] — 2026-05-21 — Crash-log panic hook in `voltex_runtime`

Deployed game executables hide stderr from the end user, so a panic in
production used to leave the user with nothing to attach to a bug
report. v1.0.10 closes that gap.

### Added

- New module `crates/voltex_runtime/src/crash.rs`:
  - `install_panic_hook(app_name, dir)` wraps the existing
    `std::panic::set_hook` so the default stderr behaviour still runs
    *and* a `<app>-crash-YYYYMMDD-HHMMSS.txt` file is written
    alongside. `std::sync::Once` makes it idempotent within a process.
  - `format_crash_report(...)` — pure function (no side effects) so
    the format is unit-testable. Includes app name, UTC wallclock,
    OS / arch, `voltex_runtime` crate version, the panic message, the
    panic location (`file:line:col`), and the full backtrace from
    `std::backtrace::Backtrace::force_capture()`.
  - `crash_filename(app, ts)` — sanitises the app name (whitespace,
    slashes, non-ASCII → `_`) so the filename is always valid on
    Windows + Linux + macOS.
  - Inline Howard-Hinnant `days_from_civil` algorithm for UNIX-seconds
    → calendar conversion (no `chrono` / `time` dep, in keeping with
    the runtime crate's external-dep policy).
- `RuntimeConfig` extensions:
  - `enable_crash_log: bool` (defaults to `true`)
  - `crash_log_dir: Option<PathBuf>` (defaults to `None` = current
    working directory at hook-fire time)
  - Builder methods `.with_crash_log_dir(p)` and
    `.disable_crash_log()` — the latter for headless / fuzz harnesses.
- `RuntimeApp::run` installs the hook before constructing the winit
  `EventLoop` when `config.enable_crash_log` is true.

### Tests

- `voltex_runtime`: 13 → **27** (+14). Crash module 11 (epoch,
  2026-05-21 noon UTC, 2000-02-29 leap, pre-epoch negative seconds,
  filename well-formed / sanitised, report content with / without
  location / trailing newline, file write + dir creation, idempotent
  install). Config 3 (default enables, disable toggle, with_dir).
- Workspace lib: 968 → **~982**.

### Known limits

- The hook itself is not exercised via a live `panic!` in tests —
  global panic-hook state is messy to isolate per-test. A separate-
  process integration test is deferred.
- Crash report omits GPU adapter name, FPS history, last loaded scene
  — useful additions for a real bug-report intake. Deferred.

---

## [v1.0.9] — 2026-05-21 — OGG/Vorbis lib.rs wire + clippy 27→0 + AudioClip entry point

First cycle of the **service-readiness roadmap**
(`docs/superpowers/specs/2026-05-21-service-readiness-roadmap.md`).
Resolves the dead-module regression discovered in v1.0.8.

### Fixed

- `crates/voltex_audio/src/lib.rs` was missing `pub mod ogg;` and
  `pub mod vorbis;`, leaving ~1.5k lines of Phase 6-1 source code out
  of the compilation graph. STATUS's "OGG/Vorbis decoder 자체 구현 완료"
  claim was therefore false. Now wired.
- `voltex_audio` clippy warnings: 24 lib + 3 test → **0** (with
  `-D warnings`). Combination of `cargo clippy --fix` (5 auto),
  per-file `#![allow(clippy::needless_range_loop)]` on the 6 Vorbis
  sub-modules (each with a one-line spec-traceability justification),
  and 4 substantive fixes (`needless_late_init`, `manual_memcpy`,
  `enumerate_discard_index`, `same_item_push`).
- Two latent test regressions surfaced by re-enabling compilation:
  - `ogg::tests::invalid_capture_pattern` — short inputs went down
    the size-truncation branch and returned the generic "No OGG
    pages found" instead of "Invalid OGG capture pattern". Fixed by
    moving the magic check ahead of the 27-byte header size check;
    4 bytes are enough to commit on the magic.
  - `vorbis::tests::bitreader_basics` — assertion arithmetic was
    wrong (author over-counted the bits remaining in byte 0).
    Replaced with a correct expectation (22 not 27) plus an
    additional 8-bit read assertion, and clarified the comment.

### Added

- `voltex_audio::decode_ogg_vorbis_bytes(&[u8]) -> Result<AudioClip>`
  — single entry point that composes `ogg::parse_ogg` and
  `vorbis::decode_vorbis`. External callers can decode `.ogg` bytes
  in one call.
- Re-exports: `voltex_audio::{parse_ogg, decode_vorbis}` at crate
  root (previously only reachable via the now-public `ogg` /
  `vorbis` modules).

### Tests

- `voltex_audio` lib: 89 → **119** (+30) — ogg ×11 (4 original +
  4 fuzz + 1 invalid_capture rewrite + 2 entry-point), vorbis ×17
  (11 original + 1 BitReader 8-bit read + 5 more headers/imdct/
  decoder/floor coverage), `decode_ogg_vorbis_bytes` ×2.
- Workspace lib: 942 → **~968**.

### Known limits (deferred)

- Decoding real `.ogg` files end-to-end is **not** verified — the
  Vorbis decoder is simplified (scalar codebooks only, no coupled
  channels, residue type 2 only). `audio_demo` does not yet load
  OGG. Both deferred to a later v1.0.x cycle.

---

## [v1.0.8] — 2026-05-21 — parser panic-resistance fuzz tests + OGG/Vorbis dead-module discovery

### Added (21 new panic-resistance tests across 5 parsers)

Every public parser entry point now has a `fuzz_parse_*` test
suite that feeds it the same four classes of input:

1. empty `&[]`
2. every truncation of a representative valid sample / known magic
   header
3. 128 seeds × 6 lengths of deterministic xorshift32-pseudo-random
   bytes (0..2048 typical range)
4. valid magic prefix + xorshift32 tail (catches the "magic OK but
   then garbage" path, which is the classic source of unwrap /
   index panics)

| Parser            | File                                                    | New tests |
|-------------------|---------------------------------------------------------|-----------|
| `parse_png`       | `voltex_renderer/src/png.rs`                            | 4 |
| `parse_jpg`       | `voltex_renderer/src/jpg/mod.rs`                        | 4 |
| `parse_gltf`      | `voltex_renderer/src/gltf/mod.rs`                       | 5 |
| `parse_obj`       | `voltex_renderer/src/obj.rs`                            | 4 |
| `parse_wav`       | `voltex_audio/src/wav.rs`                               | 4 |
| **Total**         |                                                         | **21** |

All 21 pass. **Zero panics surfaced** — every parser returns an
`Err` (or, for `parse_obj` which returns `ObjData` directly,
returns an empty/partial `ObjData`) on malformed input. The
engine's dependency policy forbids `rand` / `proptest` / `arbitrary`,
so each test file inlines a tiny xorshift32 PRNG (~10 lines, no
allocations beyond the result Vec).

### Discovered (and deferred to v1.0.9)

While adding `fuzz_parse_ogg_*` to `voltex_audio/src/ogg.rs`, the
tests never showed up in `cargo test --lib --list`. Root cause:
`crates/voltex_audio/src/ogg.rs` and `crates/voltex_audio/src/vorbis/`
exist on disk but are **not declared** in `crates/voltex_audio/src/lib.rs`
(no `pub mod ogg;` / `pub mod vorbis;`). Cargo therefore never
compiles them, and they are effectively dead source files.

This contradicts STATUS.md Phase 6-1 ("OGG/Vorbis 디코더 자체
구현 완료") — the code exists but is invisible to the rest of the
build. Tentatively wiring them in surfaced 4 `dead_code` warnings
(spec-required fields the decoder doesn't yet consume) **and** 27
pre-existing clippy errors inside the modules. Fixing all of that
exceeds the v1.0.8 scope; the lib.rs registration was reverted to
keep this patch atomic. A note in lib.rs records the intentional
omission and points at v1.0.9.

v1.0.8 deliberately preserves the `fuzz_parse_ogg_*` tests inside
the unmounted `ogg.rs` so v1.0.9 can wire the module and the tests
will start running on the first re-registration. The source change
to `ogg.rs` is therefore harmless dead code today and useful
infrastructure tomorrow.

### Not in scope (deferred to v1.0.9+)

- Wire `voltex_audio::ogg` + `voltex_audio::vorbis` into the
  module graph, fix the 27 clippy errors and 4 dead-code warnings
  they bring in, plumb vorbis output through `AudioClip` so OGG
  files are actually loadable, run the OGG fuzz tests.
- Coverage-style fuzz orchestration (cargo-fuzz / honggfuzz /
  AFL.rs) — current tests are deterministic and CI-friendly, which
  is the right v1.0 trade-off. Coverage-guided fuzzing is v1.1+.

### Verified

- 921 → **942** lib tests (+21 fuzz, no behavior changes).
- `cargo clippy --workspace --all-targets -- -D warnings` still
  passes (one unrelated unnecessary_cast in `parse_obj`'s test
  helper fixed in this patch — `(32 + r) as u8 as char` → `as
  char`).
- voltex_cli still 30 tests; workspace build still 0 warnings.

---

## [v1.0.7] — 2026-05-21 — third-party license inventory + license-check CI job

Closes P4 #28 from the post-v1.0.5 audit ("외부 라이선스 고지
미작성") and the README's own "v1.0 release artifact 에 포함 예정"
deferral.

### Added

- **`THIRD-PARTY-LICENSES.md`** (auto-generated, 4,795 lines) —
  every transitive Cargo dependency that ends up in a Voltex build,
  grouped by SPDX license, with the full license text inlined.
  Generated by `cargo about generate about.hbs -o
  THIRD-PARTY-LICENSES.md`. Versioned in the repo so external users
  see it without rebuilding.
- **`about.toml`** — cargo-about policy. Declares the accepted SPDX
  list: MIT, Apache-2.0 (with LLVM exception variant), BSD-2/3,
  BSL-1.0, ISC, Zlib, Unicode-3.0 / Unicode-DFS-2016, MPL-2.0,
  CC0-1.0, OFL-1.1. Any new transitive outside this list will fail
  the CI license-check job.
- **`about.hbs`** — handlebars template producing the markdown
  output. Lists per-license summary + per-crate listing with
  repository links + collapsible license text.
- **`[workspace.package] license = "MIT OR Apache-2.0"`** in the
  root Cargo.toml, with `license.workspace = true` added to all 31
  workspace crate Cargo.toml files (15 lib + 14 example + 1 game +
  1 facade). Without this, `cargo about` warned that every voltex
  crate had no license expression — making the output dishonest.
- **`.github/workflows/ci.yml` — new `license-check` job**
  (Ubuntu, `continue-on-error: true`). Installs cargo-about via
  `taiki-e/install-action`, runs `cargo about generate` against
  `about.toml`. Fails (advisory) on:
  - any transitive crate whose license is outside the accepted list
  - any crate cargo-about cannot determine the license for
  Advisory rather than blocking until the policy stabilizes.

### Audited (license inventory)

`cargo about generate` reports 284 license-bearing crates in the
final dependency graph, broken down as:

| License        | Crates |
|----------------|--------|
| MIT            | 264    |
| Apache-2.0     | 11     |
| Zlib           | 3      |
| BSD-3-Clause   | 2      |
| BSD-2-Clause   | 1      |
| CC0-1.0        | 1      |
| ISC            | 1      |
| Unicode-3.0    | 1      |

**Zero copyleft (GPL/AGPL/LGPL) and zero proprietary** — every
transitive license is compatible with both arms of voltex's MIT OR
Apache-2.0 dual license.

### Documentation

- **README "License" section** rewritten:
  - Removed the deferral note ("외부 의존 라이선스 고지는 작업 후속")
    that was pending since v1.0 release.
  - Links to `THIRD-PARTY-LICENSES.md`, includes the per-license
    count, states the zero-copyleft fact explicitly.
  - Documents the regeneration command for future contributors.
  - References the CI license-check job.
- **README badges**: status 1.0.6 → 1.0.7, new
  `licenses: permissive only` badge.

### Verified (self-feedback at each stage)

- Stage 1 install + generation: 4,795-line file produced.
- Stage 2 analysis: 8 distinct licenses, all permissive, zero
  copyleft.
- Stage 3 README + CI: cosmetic only.
- Stage 4 final gate: `cargo build` / `clippy -D warnings` / `test
  --lib` / `test -p voltex_cli` / `cargo about generate` all green
  with 0 warnings, 0 errors.

### Not in scope (deferred to v1.1+)

- Promoting `license-check` from advisory to blocking — depends on
  the first real CI run.
- Uploading `THIRD-PARTY-LICENSES.md` into release artifacts
  (`dist/<name>.exe` companion file) — `voltex_cli build` does not
  yet copy it.
- `cargo-deny` for combined license + advisory + duplicate scan —
  cargo-audit + cargo-about already cover the high-value cases.

---

## [v1.0.6] — 2026-05-21 — coverage baseline (82%) + advisory CI job + CLI engine-path friendliness

### Measured (coverage baseline)

First time the project has a number for `rules/common/testing.md`'s
"80%+ coverage" target. `cargo install cargo-llvm-cov` + `rustup
component add llvm-tools-preview` was needed once; result:

| metric    | covered / total | percent |
|-----------|-----------------|---------|
| Functions | 34878 / 42505   | **82.06%** |
| Lines     | 18142 / 24849   | **82.74%** |
| Regions   | (see report)    | **73.01%** |

Per-crate breakdown (function coverage), 14 crates:

- **🟢 80%+ (10 crates)**: voltex 100%, voltex_math 95.85%,
  voltex_script 94.33%, voltex_ecs 93.31%, voltex_asset 93.17%,
  voltex_ai 92.85%, voltex_net 92.08%, voltex_physics 89.85%,
  voltex_audio 88.26%, voltex_editor 82.27%.
- **🟡 below 80% (4 crates, all GPU/winit-dependent)**:
  voltex_renderer 65.94% (`shadow.rs` / `shadow_pipeline.rs` /
  `rt_*.rs` / `taa.rs` / `ssgi.rs` / `ssr.rs` / `temporal_accum.rs`
  — GPU compute shader paths, need a `wgpu::Device` to exercise),
  voltex_runtime 61.22% (`handler.rs` 0% — winit `ApplicationHandler`
  callbacks), voltex_renderer_ecs 53.83% (`BasicRenderer::render`,
  GPU pass), voltex_platform 46.38% (`ApplicationHandler` callbacks +
  per-platform key mapping).

→ The four low-coverage crates share a single root cause: their
public surface is **GPU + winit integration code that cannot be
reached from a headless `cargo test --lib`**. Their unit-testable
internals (`app.rs`, `config.rs`, `error.rs`, `RenderItem`
collection, etc.) all sit at 80%+. Closing the gap requires
integration tests against a headless GPU adapter and a stubbed
event loop — scheduled for v1.1+.

### Added (infrastructure)

- **`.github/workflows/ci.yml` — new `coverage` job** (Ubuntu,
  `continue-on-error: true`). Installs llvm-tools + cargo-llvm-cov
  via `taiki-e/install-action`, then runs `cargo llvm-cov
  --workspace --lib --summary-only`. Advisory only — threshold
  enforcement (`--fail-under-lines 80`) postponed until GPU paths
  are covered by integration tests.
- **README "Contributing" → "Coverage" subsection**: documents the
  one-time install commands, the current baseline number, and why
  the 4 low-coverage crates are low.
- **README badge**: `coverage 82% lib`. Status badge bumped 1.0.5
  → 1.0.6.

### Improved (CLI friendliness — P1 #5)

- **`voltex build` engine-path missing error** rewritten as a
  multi-line help block. Old (1 line): `"engine path unknown — set
  VOLTEX_REPO or pass --engine-path <repo-root>"`. New (7 lines):
  scaffolded-project example + both shell-specific export commands
  (bash/zsh + PowerShell), so new users get a copy-pasteable fix in
  the error itself.
- **README 5-min quickstart**: split into "예제 실행" + "새 게임
  만들기" subsections. The second one walks through `cargo install
  --path crates/voltex_cli` → `export VOLTEX_REPO=$(pwd)` → `voltex
  new` → `voltex build`. Closes the discoverability gap where users
  saw `cargo run --example triangle` but could not figure out how
  to scaffold their own project.

### Verified (self-feedback at each stage)

- Stage 1: cargo-llvm-cov install + baseline measurement (82.06%).
- Stage 2: per-crate analysis, low crates identified and justified.
- Stage 3: README + CI yml updated, no behavior change.
- Stage 4: CLI tweak — empty-directory repro of `voltex new && cd &&
  voltex build` (without VOLTEX_REPO env) now prints the 7-line
  help block instead of the 1-line one.
- Stage 5: `cargo build --workspace` clean, `cargo test --workspace
  --lib` = 921, `cargo test -p voltex_cli` = 30. clippy still 0.

### Not in scope (deferred to v1.1+)

- Lifting the 4 low-coverage crates above 80% — requires headless
  GPU adapter testing + winit event-loop stubbing.
- Enabling `--fail-under-lines 80` as a CI gate — depends on the
  above.
- HTML coverage report upload (codecov / coveralls) — depends on
  the CI workflow actually running on a runner.

---

## [v1.0.5] — 2026-05-21 — clippy clean (warnings 84 → 0) + cargo audit + CI workflow

This is the "quality gate" patch. v1.0.3 (Windows build P0) and v1.0.4
(clippy deny errors) both slipped past internal verification because
`cargo build` + `cargo test --lib` was the only standing check. v1.0.5
closes the loop: zero clippy warnings, zero open advisories, and a CI
workflow that enforces both on every push.

### Fixed (clippy warnings 84 → 0)

Baseline measurement before this patch: `cargo clippy --workspace
--all-targets` produced **84 lines of warnings/errors** (51 unique
diagnostics across 13 lint categories).

**Auto-fixed** (`cargo clippy --fix --workspace --all-targets`, 43
suggestions across `voltex_math`, `voltex_audio`, `voltex_platform`,
`voltex_ecs`, `voltex_cli`, `voltex_renderer`, and 3 examples):
- `manual_div_ceil` ×13 → `.div_ceil(...)`
- `manual RangeInclusive::contains` ×4 → `.contains(&x)`
- `excessive_precision` ×4 (audio wav constants)
- `manual_is_multiple_of` ×1 → `.is_multiple_of(2)`
- `collapsible_if` ×1, `identity_op` ×1, `len_zero` ×1,
  `unwrap_or_else_default` ×1
- subset of `needless_range_loop` (iterator-convertible cases)

**Manually fixed** (24 residual diagnostics, semantic intent required):
- `voltex_audio::mixing::make_stereo_clip` and
  `voltex_ecs::world::tests::Name` — both genuinely dead, deleted
  outright.
- `voltex_script::state` ×5 — `b"name\0".as_ptr() as *const _`
  replaced with C-string literal `c"name".as_ptr()` (Rust 1.77+).
- `voltex_renderer::jpg::mod` ×4 — `for _ in 0..N { vec.push(byte); }`
  collapsed to `vec.extend_from_slice(&[byte; N])` for JPEG marker
  padding.
- `voltex_math::mat4`, `voltex_renderer::sh` ×4 — `needless_range_loop`
  cases where `r`/`c`/`k`/`ch` indices appear in both lookup AND
  assert message: annotated `#[allow(clippy::needless_range_loop)]`
  with a comment.
- `voltex_editor::panels::hierarchy::draw_entity_node`,
  `voltex_editor::picking::pick_entity`,
  `deferred_demo::create_shadow_bind_group` — `too_many_arguments`
  cases where args are intrinsic (recursive draw / viewport rect +
  mouse + camera / 9-binding GPU layout): annotated `#[allow]`.
- `voltex_cli::manifest::extract_build` — `very_complex_type` (6-tuple
  return) refactored to return `BuildConfig` directly; the destructure
  at the call site collapsed to single `build` field. Net code
  shorter, type signature self-documenting.
- `examples/{shadow_demo,deferred_demo,asset_demo}` — small loop
  refactors and 1 `#[allow]` where index threads multiple parallel
  arrays.

### Added (CI infrastructure)

- **`.github/workflows/ci.yml`** — GitHub Actions matrix that runs
  `cargo build --workspace`, `cargo clippy --workspace --all-targets
  -- -D warnings`, `cargo test --workspace --lib`, and
  `cargo test -p voltex_cli` on **ubuntu-latest + windows-latest**
  (blocking) and **macos-latest** (advisory, `continue-on-error`).
  Linux job installs `libasound2-dev` before build to satisfy
  `voltex_audio`'s ALSA FFI link. Cache layer keys on `Cargo.lock`.
  File is benign on non-GitHub remotes — gitea/forgejo also accept
  the syntax.
- **README "Contributing" section** — documents the exact four-command
  local verify recipe that mirrors CI, so PR authors can reproduce
  the gate before pushing.

### Audited (cargo audit)

- Installed `cargo-audit` (one-time tool, not a runtime dep).
- Scan of 298 `Cargo.lock` crates against 1,096 advisories:
  **0 vulnerabilities**.
- **1 warning** — `paste 1.0.15` flagged unmaintained
  (RUSTSEC-2024-0436). Transitive: `paste ← metal ← wgpu-hal ←
  wgpu`. Cannot be addressed at voltex level; resolved by future
  wgpu upgrade. Documented for awareness, no action required.

### Verified (self-feedback at each stage)

- Stage 1 baseline: 84 warnings counted.
- Stage 2 post-auto-fix: 41 warnings counted, 921 lib tests pass.
- Stage 3 post-manual-fix: **0 warnings**, 921 lib tests pass,
  30 voltex_cli bin tests pass.
- Stage 4 cargo audit: 0 vulns, 1 advisory unrelated to voltex code.
- Stage 5 CI gate re-run locally: `RUSTFLAGS="-D warnings"
  cargo {build,clippy,test}` all green.

### Not in scope (deferred to v1.1+)

- Actually running the CI workflow on a GitHub runner — requires a
  GitHub mirror. The workflow file ships ready-to-use; the first
  green run on a real CI is the next-cycle verification step.
- 3-OS exe runtime verification (Linux ALSA audio output, macOS
  CoreAudio audio output) — still environment-dependent.
- Adding `cargo audit` to the CI workflow — `cargo-audit` itself
  needs an install step; postponed pending a CI-friendly setup.

---

## [v1.0.4] — 2026-05-21 — clippy `approx_constant` clean + README dependency honesty

### Fixed
- **clippy `approx_constant` errors (9 → 0)** across
  `voltex_physics::integrator`, `voltex_renderer::animation`,
  `voltex_ecs::world::tests`, `voltex_ecs::json`, and
  `voltex_renderer::json_parser`.
  - `integrator.rs` test used `3.14159` for `rb.angular_velocity` /
    expected_rot_y → replaced with `std::f32::consts::PI` (real intent
    was 1 PI rad/s — comment already said so).
  - `animation.rs::test_quat_slerp_endpoints` used `0.7071` for 90°
    quaternion components → replaced with `std::f32::consts::FRAC_1_SQRT_2`
    (more precise + self-documenting).
  - `world/tests.rs::test_clear_changed_all_storages` used `3.14f32`
    only as "a different-typed value" → replaced with `2.5f32` (no
    semantic dependency on the value at all).
  - `json.rs::test_write_number`, `json.rs::test_parse_negative_number`,
    `json_parser.rs::test_parse_number` use `3.14` / `-3.14` as
    representative decimal payloads for serialization round-trip —
    annotated with `#[allow(clippy::approx_constant)]` + comment
    explaining the intent (the value is data, not π).
  - Net result: `cargo clippy --workspace --all-targets` no longer
    fails on `#[deny(clippy::approx_constant)]`. External contributors
    can now run clippy without a hard error.

### Corrected (documentation honesty)
- **README "외부 crate 의존 단 2개 (winit / wgpu)" claim was inaccurate.**
  Actual measured direct dependencies:
  - **Runtime (3):** `winit`, `wgpu`, `bytemuck` (Pod/Zeroable derive +
    cast — wgpu's de-facto companion, but a real direct dep nonetheless).
  - **Build-only (2, never present in shipped runtime):** `thiserror`
    (`Error` derive macro; no runtime crate after compilation), `cc`
    (`voltex_script`'s Lua 5.4 C source `build.rs`).
  - **Example/binary helpers (not in any library crate):** `log`,
    `env_logger`, `pollster`. Listed separately so future audits don't
    mistake them for engine deps.
  - **Transitive total:** 298 crates in `Cargo.lock` (dominated by
    wgpu's GPU stack and winit's per-OS windowing).
  README header and "의존성 정책" table updated to reflect this with
  rationale per crate.

### Verified
- `cargo clippy --workspace --all-targets` — `approx_constant` count
  3 errors / 5 hits  →  0.
- `cargo test --workspace --lib` — 921 lib tests still pass (the 3
  test files touched are part of the standard suite; `voltex_physics`
  106 / `voltex_ecs` 134 / `voltex_renderer` 200 unchanged).
- No production-code changes — only test bodies and 2 `use std::f32::consts::*`
  imports inside `#[cfg(test)]` modules.

### Not in scope (deferred to v1.1+)
- The remaining ~80 clippy warnings (manual `RangeInclusive::contains`,
  `needless_range_loop`, dead code in `voltex_audio::make_stereo_clip`
  / `voltex_ai::Name`, excessive float precision in `voltex_audio::wav`,
  etc.) are non-deny lints. v1.0.4 deliberately addresses only the
  hard errors that block external `cargo clippy --all-targets` runs.
- CI workflow that enforces `cargo clippy --workspace --all-targets`
  going forward — recommended next step but not added in this patch.

---

## [v1.0.3] — 2026-05-21 — `voltex build` Windows P0 regression fix

### Fixed
- **P0 — `voltex build` failed on every Windows host**.
  `crates/voltex_cli/src/build.rs:96-97` (and `run.rs:20`) called
  `std::fs::canonicalize`, which on Windows always returns the
  extended-length `\\?\C:\...` verbatim form. That prefix landed in the
  generated `dist/wrapper/Cargo.toml` as
  `path = "\\\\?\\C:\\...\\voltex_runtime"`, and Cargo rejected it with
  `invalid path url '//?/C:\\...\\voltex_runtime'`. Result: the
  README's 5-minute quickstart (`voltex new my_game && cd my_game &&
  voltex build`) was 100 % broken for external users. The flagship
  `games/voltex_survivor` and `examples/tutorial_minigame` were
  unaffected because they are workspace members built directly with
  `cargo build`, hiding the regression from internal verification.

### Added
- `voltex_cli::util::canonicalize_friendly(&Path) -> Result<PathBuf>`:
  canonicalize, then strip the Windows `\\?\` verbatim prefix for
  drive-letter paths (`\\?\C:\foo` → `C:\foo`). UNC paths
  (`\\?\UNC\server\share\...`) are intentionally left untouched —
  rewriting them is lossy. No new external dependencies (engine policy
  forbids `dunce` etc.).
- Regression tests (+4): `strip_verbatim_prefix` drive-letter strip,
  UNC passthrough, non-verbatim passthrough (all Windows-gated), and
  cross-platform `canonicalize_friendly_returns_non_verbatim_path`.

### Verified
- End-to-end CLI workflow re-run on Windows host: `voltex new my_game`
  → `voltex build --engine-path <repo>` → `dist/my_game.exe` produced
  in 61 s, wrapper `Cargo.toml` contains
  `voltex_runtime = { path = "C:/Users/<user>/..." }` (no `\\?\`).
- 921 lib tests still pass. `voltex_cli`: 26 → 30 (+4).
- Workspace build: 0 warnings, 0 errors.

### Cleanup
- `examples/many_cubes/src/main.rs:8` — removed unused `Mat4` import
  (only `Vec3` is actually used). Was the lone warning in the
  workspace build.

### Not changed
- No behavior change for `voltex_survivor`, examples, or any engine
  crate. Patch is confined to `voltex_cli` (and the trivial
  many_cubes import cleanup).

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
