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
| **Collision events** | "Caught" is a real `set_collision_fn` contact between the player + chaser sphere `Collider`s (not distance math). |
| **HUD runtime API** | `set_hud_fn` draws distance / `RUN!` vs `CAUGHT!` / counters on screen. |
| **Camera control** | `set_camera_fn` keeps a follow-camera framed on the player (the default runtime camera sits far back, leaving a 1-unit cube a tiny speck). |
| **Lighting** | `with_light` (warm angled sun) + `set_light_fn` (slowly rotating it) + two colored arena point lights + a white center **spotlight** (`PointLight::spot`) via `set_point_lights` + `with_shadows(true)` directional shadows. |
| **Skeletal animation** | A GPU-skinned "totem" strip (off to the left) waves via `SkinnedPose` + `BasicRenderer`'s skinned pass. |
| **Debug draw** | `set_debug_draw_fn` builds gizmos each frame: a yellow chase vector, cyan/magenta crosses on the player/chaser, and the arena bounds as a green `DebugLines::aabb`. |
| **Billboard sprites** | Three floating `Sprite::solid` "pickups" — camera-facing colored quads that stay screen-aligned as you orbit. |
| **Particles** | Each catch pops a 16-particle orange burst (`spawn_particle` + the runtime's `update_particles`) that arcs outward under gravity and fades. |

## Controls

- **WASD** — move the player (keep the right mouse button released)
- **RMB + drag** — orbit the fly-camera
- **Esc** — quit

The player is the **blue** cube; the chaser is the **red** one on the gray
ground (per-object colors via `MaterialUniform.base_color`). The player
(speed 6) is faster than the chaser (speed 3.5), so you can escape and get
caught repeatedly — each catch fires a fresh beep and bumps the counter.

## Notes

The skinned totem uses a single directional light + the Blinn-Phong runtime
path. Full **PBR** materials (textures/normal maps) and **multi-light** still
require the dedicated PBR pipeline (see `survivor_game`) — runtime integration
of those is future work.
