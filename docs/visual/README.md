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

Full *offscreen pixel* capture of the deferred path is deferred to v1.1: the
lighting pass samples SSGI and RT-shadow textures that are only meaningful once
those passes run (which need RT features + a BLAS), so a passes-skipped offscreen
replica would read uninitialised inputs. The deferred lighting BRDF is the same
Cook-Torrance already pixel-verified by `pbr_grid` / `multi_light`.

## Not yet captured

- Deferred offscreen pixel capture (see above — runtime-verified instead).
- The flagship game runtime, actual mouse/gizmo drag interaction.

The reference PNGs here are downscaled (≤240px) and uncompressed (the encoder
emits stored deflate blocks). They are a point-in-time record; regenerate with
the command above to diff against the current build.
