# game_ready_demo

Hands-on demo of the v1.0.59 "game-ready" wiring — all in one window.

```bash
cargo run -p game_ready_demo
```

## What you control / see

| Feature | In the demo |
|---|---|
| **Lua input API** | The player cube is moved by `WASD`, driven by `player_move.lua` calling `input.is_key_pressed(...)`. |
| **nav_agent_system** | The smaller chaser cube paths toward the player every frame via the navmesh. |
| **audio_sync_system** | The chaser carries a spatial `AudioSource` that beeps the moment it catches you (listener = player). |
| **HUD runtime API** | `set_hud_fn` draws distance / `RUN!` vs `CAUGHT!` / counters on screen. |

## Controls

- **WASD** — move the player (keep the right mouse button released)
- **RMB + drag** — orbit the fly-camera
- **Esc** — quit

The player (speed 6) is faster than the chaser (speed 3.5), so you can escape
and get caught repeatedly — each catch fires a fresh beep and bumps the counter.

## Not included

Skinned mesh animation is **not** shown here: the runtime render path
(`BasicRenderer`) does not yet do GPU skinning (deferred ECS-skinning
integration, v1.1). The skinning core is verified separately — see
`docs/visual/skinned_rest.png` / `skinned_bent.png` and
`voltex_capture::scene_skinned`.
