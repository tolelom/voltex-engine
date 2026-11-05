# hierarchy_demo

Solar-system style scene graph — sun + planets + moons — driven by
`Parent` / `Children` and `WorldTransform` propagation.

## What it demonstrates

- `voltex_ecs::add_child` / `remove_child` API.
- Top-down `propagate_transforms` system that builds each entity's
  `WorldTransform` from its parent chain.
- Different orbit speeds per body type (sun spins slow, planets faster,
  moons fastest).
- `Tag` component for distinguishing body types in queries.

## Run

```bash
cargo run -p hierarchy_demo --release
```

## Controls

- **Right-click + drag**: rotate camera.
- **WASD + Space/Shift**: move.
- **Esc**: exit.

## Next step

→ [`asset_demo`](../asset_demo) — shares one mesh across many entities through
`voltex_asset::Assets` handles.
