# runtime_demo

`RuntimeApp` "hello world" — the entire main loop is four lines. Builds a
solar-system scene programmatically + shows HUD usage (v1.0.32).

## What it demonstrates

- `voltex_runtime::RuntimeApp::new(RuntimeConfig::new()…)?.run()` — the
  full lifecycle in one chained builder.
- `with_scene_builder` callback that populates the World before launch.
- `with_gpu_init` callback that runs once when the GPU comes up (used here
  to upload meshes + materials).
- `add_system` (no input) + `add_input_system` (gets InputState) registration.
- HUD primitive (v1.0.32) — the window title shows
  "voltex_runtime: solar system | N planets, M moons | XX fps".

## Run

```bash
cargo run -p runtime_demo --release
```

## Controls

- **Right-click + drag**: rotate camera.
- **WASD + Space/Shift**: move.
- **Esc**: exit.

## Next step

→ [`tutorial_minigame`](../tutorial_minigame) — the 30-minute tutorial's
final output (player + enemies + Lua scripts on top of `RuntimeApp`).
