# survivor_game (archived)

Archive snapshot of the early "Survivor" prototype written directly against
ECS + physics + facade — kept for contrast with
[`games/survivor`](../../games/survivor) (the same game
rebuilt on `RuntimeApp`, with its rules in Lua).

## Why kept

- Shows what a pre-`RuntimeApp`, pre-Lua game looked like — every system
  hand-wired in Rust.
- Useful when comparing the "render assets uploaded inline" pattern with
  the `RuntimeApp::with_gpu_init` callback pattern.
- Reference for the `Tag`-driven enemy / projectile attachment pattern
  the flagship game now does in Lua.

## Run

```bash
cargo run -p survivor_game --release
```

## Not recommended as a learning path

New users should start with [`runtime_demo`](../runtime_demo) →
[`tutorial_minigame`](../tutorial_minigame) → [`../games/survivor`](../../games/survivor).
This crate stays in the workspace only for the comparison + as a smoke
test that direct-Rust gameplay code still compiles.
