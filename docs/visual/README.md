# Visual verification

Until now the engine's rendering had **never been visually verified** — the
windowed examples can't be observed by automation, and the 6-second smoke
tests only proved "no panic", not "draws the right thing". The
[`voltex_capture`](../../crates/voltex_capture) crate closes that gap: it
renders the *real* GPU pipelines into an offscreen texture and writes a PNG,
turning "does it actually draw?" into a reproducible artifact.

## Reproduce

```bash
# Full-resolution captures into target/captures/ (needs a GPU):
cargo run -p voltex_capture --bin capture

# Small reference thumbnails (cap longest side to 240px) into docs/visual/:
cargo run -p voltex_capture --bin capture -- docs/visual 240
```

The bin prints per-scene stats (mean luminance, % black pixels, distinct
colors) and exits non-zero if any scene renders blank (single flat color).

GPU-gated scene tests live alongside each scene and are `#[ignore]`'d (no GPU
in CI):

```bash
cargo test -p voltex_capture -- --ignored
```

## What was verified (2026-05-28)

GPU: NVIDIA GeForce RTX 4080 SUPER (Vulkan).

| Scene | Pipeline exercised | Result |
|---|---|---|
| `cube_grid` | `voltex_renderer_ecs::BasicRenderer` (Blinn-Phong, ECS-driven — the runtime/editor viewport path) | ✅ per-face shading correct |
| `pbr_grid` | `create_pbr_pipeline` (Cook-Torrance) + `IblResources` — 7×7 metallic×roughness grid | ✅ correct BRDF gradient + IBL ambient (4135+ colors) |
| `shadow_scene` | two-pass directional shadow map (`create_shadow_pipeline` depth pass → PBR pass) | ✅ shadows correctly placed for the light direction |
| `multi_light` | PBR + `LightsUniform` (dim directional + 4 colored point lights + spot) | ✅ distinct colored illumination with distance attenuation (7000+ colors) |
| `gizmo_{translate,rotate,scale}` | `voltex_editor::GizmoRenderer` line overlay over a cube | ✅ all three handle sets (arrows / rings / boxes) at the right origin with correct per-axis R/G/B colors |
| `editor_ui` | `voltex_editor::UiRenderer` (immediate-mode UI + bitmap font atlas + alpha) | ✅ text + widgets render legibly |

### Findings

- **BasicRenderer / PBR / shadows / multi-light / gizmos / editor UI all render
  correctly.** This is the first confirmation that the engine produces correct
  images, not just non-panicking frames.
- **Bug found & fixed:** the immediate-mode UI had no horizontal layout, so the
  `editor_demo` toolbar's buttons stacked vertically and overlapped the
  viewport. Added `UiContext::same_line()` (see the `fix(editor)` commit).
- **Note (not a bug):** the default light `LightUniform::new()` direction
  `(0,-1,-1)` lights the +Y and +Z faces identically, so axis-aligned scenes
  look flat under it — use an asymmetric direction to see per-face shading.
- **Known limitation:** with SH irradiance coefficients zeroed, metallic
  surfaces reflect a flat gray IBL environment and look desaturated. Consistent
  across pbr_grid and shadow_scene; matches the simplified-IBL design.

## What was verified (2026-06-03 — game-ready 2D stack)

GPU: NVIDIA GeForce RTX 4080 SUPER (Vulkan). All have `#[ignore]` GPU
regressions in `voltex_capture/src/scene.rs` (run `cargo test -p voltex_capture
-- --ignored`).

| PNG / test | Pipeline exercised | Result |
|---|---|---|
| `spot_cone.png` (`spotlight_lights_cone_footprint_not_corners`) | `PointLight::spot` cone falloff in `mesh_colored.wgsl` | ✅ soft-edged circular cone; center lit, corners dark; aiming away darkens |
| `debug_lines.png` (`debug_lines_draw_colored_segments`) | `debug_line` `LineList` pipeline + `BasicRenderer::set_debug_lines` | ✅ red row + green column in their colors, corners black |
| `sprite_billboard.png` (`billboard_sprite_faces_camera_from_any_angle`) | `Sprite` billboard pass | ✅ footprint identical from front + side orbit |
| (`sprite_sheet_frame_selects_atlas_cell`) | `Sprite.uv_rect` / `set_sheet_frame` | ✅ each of a 2×2 atlas's 4 frames samples the right cell |
| (`render_assets_png_loader_roundtrips_into_a_sprite`) | `RenderAssets::add_texture_png` | ✅ a red PNG loads + renders red |
| (`overlapping_sprites_blend_independent_of_spawn_order`) | back-to-front sprite sort | ✅ overlap composite is spawn-order-independent, near on top |
| `particle_burst.png` (`particles_spread_outward_over_time`) | `Particle` + `update_particles` + sprite pass | ✅ mean radius grows over time |
| (`orthographic_footprint_is_depth_independent`) | `Camera::orthographic` | ✅ footprint depth-independent (vs perspective foreshortening) |
| `demo_frame_{a,b}.png` (`demo_frames_render_nonblank`) | full `game_ready_demo` (sun + 2 point + spot + shadows + sprites + gizmos + skinned totem) | ✅ all features composite correctly |
| `pbr_runtime.png` / `skinned_pbr.png` (`basic_renderer_pbr_path_shades_differently_than_colored`, `runtime_skinned_pbr_path_renders_and_animates`, `pbr_normal_map_perturbs_shading`) | runtime PBR (Cook-Torrance + multi-light + maps + IBL) for static + skinned meshes | ✅ metallic gradient, normal maps perturb shading, skinned meshes PBR-lit |
| `demo_frame_pbr.png` (`demo_renders_under_pbr_runtime_path`) | the whole demo scene through `with_pbr(true)` | ✅ all features composite under PBR |

## Runtime-verified on real hardware (no offscreen PNG)

The windowed demos were each run for ~6 s on the RTX 4080 (Vulkan) under
`RUST_LOG=warn` and produced **no panics and no wgpu validation errors**
(only benign Vulkan loader instance warnings):

- `deferred_demo` — the full deferred stack: G-buffer MRT → SSGI → RT-shadow
  compute → deferred lighting → bloom → ACES tonemap. This exercises ray-tracing
  **feature acquisition + BLAS build + a 9-entry lighting bind group** on real
  hardware — coverage neither CI (headless, no RT) nor the offscreen harness
  reaches.
- `ibl_demo`, `shadow_demo` — initialise and render clean.
- `voltex_survivor` (flagship), `runtime_demo`, `survivor_game` — the full
  `RuntimeApp` bootstrap (window + ECS + systems + scheduler + audio + Lua
  script + render loop) runs clean for ~6 s.

Full *offscreen pixel* capture of the deferred path is deferred to v1.1: the
lighting pass samples SSGI and RT-shadow textures that are only meaningful once
those passes run (which need RT features + a BLAS), so a passes-skipped offscreen
replica would read uninitialised inputs. The deferred lighting BRDF is the same
Cook-Torrance already pixel-verified by `pbr_grid` / `multi_light`.

## Not yet captured

- Deferred offscreen pixel capture (see above — runtime-verified instead).
- Actual mouse/gizmo drag interaction (needs synthetic input injection).

The reference PNGs here are downscaled (≤240px) and uncompressed (the encoder
emits stored deflate blocks). They are a point-in-time record; regenerate with
the command above to diff against the current build.
