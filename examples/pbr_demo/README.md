# pbr_demo

Cook-Torrance PBR shader showcased on a 7×7 grid of spheres varying in
metallic + roughness.

## What it demonstrates

- `voltex_renderer::pbr_pipeline::create_pbr_pipeline` (GGX NDF + Smith
  geometry + Fresnel-Schlick).
- `MaterialUniform` per sphere with varying metallic / roughness.
- 8-binding texture layout (albedo / normal / ORM / emissive) — this demo
  uses constant material values, see [`ibl_demo`](../ibl_demo) for full
  texture sets.
- Procedural sphere generator (`voltex_renderer::sphere`).

## Run

```bash
cargo run -p pbr_demo --release
```

Rows = increasing roughness (top: mirror-smooth, bottom: chalk-rough).
Columns = increasing metallic (left: dielectric, right: fully metallic).

## Controls

- **Right-click + drag**: rotate camera.
- **WASD + Space/Shift**: move.
- **Esc**: exit.

## Next step

→ [`multi_light_demo`](../multi_light_demo) — adds Directional / Point /
Spot lights with frustum culling.
