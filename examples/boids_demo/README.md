# boids_demo

A flocking swarm built almost entirely from the declarative authoring and
runtime primitives.

## What it demonstrates

- **Declarative renderables** — each boid is `spawn_bundle((Transform,
  PrimitiveMesh::cube, MaterialDesc::color, Boid))`; no manual mesh upload, the
  runtime materializes GPU meshes/materials each frame.
- **Deterministic RNG** (`Rng`) — random start positions, headings, and per-boid
  colors from a fixed seed (same swarm every run).
- **Flocking AI** (`flocking_system`) — separation / cohesion / alignment over a
  `SpatialGrid`, run once per frame; a toroidal wrap keeps the flock on screen.

## Run

```
cargo run -p boids_demo
```

A top-down view of ~120 cubes that organize into flowing flocks. No controls.

## Visual reference

`docs/visual/boids_swarm.png` (the initial scattered state; flocking clusters
them over time).

## Next step

Read `examples/game_ready_demo` for input/nav/audio wiring, or
`games/survivor` for a Lua-scripted game on the same runtime.
