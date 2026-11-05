# asset_demo

One OBJ mesh stored in `voltex_asset::Assets` and shared by 100 ECS entities
through a `Handle<Mesh>`.

## What it demonstrates

- `Assets::insert` storing the parsed mesh and returning a `Handle<Mesh>`.
- 100 entities in a 10×10 grid that all carry the same handle; each frame
  resolves it once with `Assets::get` and draws every entity with it.
- `Assets::count::<Mesh>()` — the window title shows the entity count and the
  mesh asset count (removing entities does not remove the shared mesh).

## Run

```bash
cargo run -p asset_demo
```

Press **R** to despawn 10 entities; the title and the log show what is left.

## Next step

→ [`pbr_demo`](../pbr_demo) — switches from Blinn-Phong to a Cook-Torrance
PBR shader.
