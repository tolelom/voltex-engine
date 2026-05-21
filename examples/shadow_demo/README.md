# shadow_demo

Cascaded Shadow Maps (directional), cube shadow maps (point), spot shadow
maps. PCF 3×3 filtering.

## What it demonstrates

- `voltex_renderer::csm::CascadedShadowMap` — 2 cascades, frustum-fitted.
- `voltex_renderer::point_shadow::PointShadowMap` — cubemap, 6-face render.
- `voltex_renderer::spot_shadow::SpotShadowMap` — single perspective view.
- `voltex_renderer::shadow_pipeline::create_shadow_pipeline` — depth-only pass.
- PCF 3×3 sampling in the PBR fragment shader.

## Run

```bash
cargo run -p shadow_demo --release
```

## Controls

- **Right-click + drag**: rotate camera.
- **WASD + Space/Shift**: move.
- **Esc**: exit.

## Next step

→ [`ibl_demo`](../ibl_demo) — Image-Based Lighting via L2 spherical harmonics
+ GPU BRDF LUT + Hosek-Wilkie procedural sky.
