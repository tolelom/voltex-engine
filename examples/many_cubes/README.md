# many_cubes

400 entities driven by `voltex_ecs` + a dynamic UBO. First introduction to
the ECS loop.

## What it demonstrates

- `voltex_ecs::World` with hundreds of `Transform` entities.
- `query::<Transform>` iteration in a system that animates each cube.
- Per-entity dynamic UBO upload (one bind group, 400 dynamic offsets).
- Frustum culling can be added on top (see [`multi_light_demo`](../multi_light_demo)).

## Run

```bash
cargo run -p many_cubes --release
```

## Controls

- **Right-click + drag**: rotate camera.
- **WASD**: move.
- **Esc**: exit.

## Next step

→ [`hierarchy_demo`](../hierarchy_demo) — adds Parent/Children hierarchy and
`WorldTransform` propagation.
