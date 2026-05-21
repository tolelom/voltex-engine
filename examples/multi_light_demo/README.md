# multi_light_demo

Directional + Point + Spot lights (up to `MAX_LIGHTS = 16`) with
Gribb-Hartmann frustum culling.

## What it demonstrates

- `voltex_renderer::light::LightData` + `LightsUniform` packing.
- Mixing all three light types in a single PBR pass.
- `voltex_renderer::frustum::sphere_vs_frustum` + `cull_lights` reducing the
  active light count per frame.
- The PBR fragment shader's multi-light loop.

## Run

```bash
cargo run -p multi_light_demo --release
```

## Controls

- **Right-click + drag**: rotate camera.
- **WASD + Space/Shift**: move.
- **Esc**: exit.

## Next step

→ [`shadow_demo`](../shadow_demo) — adds Cascaded Shadow Maps + cube
shadow maps + spot shadow maps.
