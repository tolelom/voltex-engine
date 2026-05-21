# ibl_demo

Image-Based Lighting — L2 spherical harmonics irradiance + GPU BRDF LUT +
Hosek-Wilkie procedural sky.

## What it demonstrates

- `voltex_renderer::sh::compute_sh_coefficients` — CPU-side L2 SH from the
  procedural sky.
- `voltex_renderer::brdf_lut::generate_brdf_lut` — GPU compute shader that
  bakes the split-sum BRDF integral into a 2D LUT.
- `voltex_renderer::ibl::IblResources` — bundles the SH + BRDF LUT into a
  bind group consumed by the PBR shader.
- Hosek-Wilkie sky model (sun direction + turbidity).

## Run

```bash
cargo run -p ibl_demo --release
```

## Controls

- **Right-click + drag**: rotate camera.
- **WASD + Space/Shift**: move.
- **Esc**: exit.

## Next step

→ [`deferred_demo`](../deferred_demo) — switches to a deferred renderer with
G-Buffer + RT shadows + bloom + tone mapping + TAA.
