# triangle

First-pixel example. Pure wgpu pipeline that draws a colored RGB triangle
into a winit window — the smallest possible Voltex program.

## What it demonstrates

- `VoltexWindow` (winit wrapper) + `GpuContext` (surface/device/queue setup).
- A single `wgpu::RenderPipeline` with the simplest possible vertex/fragment
  shader.
- Vertex buffer upload using `bytemuck` casting.

## Run

```bash
cargo run -p triangle             # debug
cargo run -p triangle --release   # release
```

## Key files

| File | Purpose |
|---|---|
| `src/main.rs` | window + GPU init + main loop |
| `src/shader.wgsl` (in `voltex_renderer`) | the vertex + fragment shader |

## Next step

→ [`model_viewer`](../model_viewer) — adds OBJ/glTF loading, FPS camera,
Blinn-Phong lighting on top of this pipeline.
