# tutorial_minigame

Final output of [`docs/tutorial.md`](../../docs/tutorial.md) — a player +
enemies game on top of `RuntimeApp` + Lua scripts.

## What it demonstrates

The tutorial's two main milestones:

- **D1**: scene with player entity + a static obstacle. Player input system
  (WASD) translates a `Transform`.
- **D2**: enemies (driven by Lua `enemy.lua` script attached via
  `ScriptComponent`) chase the player. Game-over detection on player
  proximity.

This is the smallest realistic vertical slice — bootstrap, scene, input,
ECS systems, scripted AI.

## Run

```bash
cargo run -p tutorial_minigame --release
```

## Key files

| File | Purpose |
|---|---|
| `src/main.rs` | RuntimeApp bootstrap + input system + scene seeding |
| `assets/scene.vscn` | starting scene |
| `assets/enemy.lua` | enemy chase logic |

## Controls

- **WASD**: move the player.
- **Esc**: exit.

## Next step

→ [`../games/survivor`](../../games/survivor) — flagship
game written 100% in Lua + scenes on top of the same `RuntimeApp`.
