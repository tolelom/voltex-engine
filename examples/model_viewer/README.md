# model_viewer

OBJ + glTF model loader with FPS camera + Blinn-Phong lighting.

## What it demonstrates

- `voltex_renderer::obj::parse_obj` / `voltex_renderer::gltf::parse_gltf` for
  asset loading.
- `Mesh` upload to GPU.
- `Camera` + `FpsController` for first-person navigation.
- Blinn-Phong fragment shader (predecessor to PBR — see [`pbr_demo`](../pbr_demo)).
- BMP texture loading via `voltex_renderer::texture::parse_bmp`.

## Run

```bash
cargo run -p model_viewer --release
```

## Controls

- **Right-click + drag**: rotate camera (yaw/pitch).
- **WASD**: move horizontally.
- **Space / Shift**: move up / down.
- **Esc**: exit.

## Next step

→ [`many_cubes`](../many_cubes) — drops a single mesh + camera in favor of
ECS-driven multi-entity rendering.
